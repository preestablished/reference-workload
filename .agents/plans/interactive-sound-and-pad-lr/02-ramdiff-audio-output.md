# Package 02 — ramdiff: Audio Playback Sink And M Mute Hotkey

Depends on package 01 (`refwork-emu` `audio` feature + `Core::take_audio_samples`).

## Why

`ramdiff record --interactive` (`crates/ramdiff/src/record.rs:413-650`) blits
video via minifb but has no audio path. minifb has no audio support, so we
add **cpal** (the de-facto Rust audio-output crate: CoreAudio on macOS, ALSA
on Linux) behind the existing `interactive` feature, drain the core's sample
ring once per frame, and feed a shared playback buffer consumed by the cpal
output stream. `M` toggles mute.

## Changes

### 1. Feature and dependency wiring (crates/ramdiff/Cargo.toml)

```toml
[features]
interactive = ["dep:minifb", "dep:gilrs", "dep:cpal", "refwork-emu/audio"]

[dependencies]
cpal = { version = "0.15", optional = true }
```

- `refwork-emu/audio` rides on `interactive` so non-interactive CI lanes are
  untouched (CI never builds `interactive`, per the existing comment in
  ramdiff's Cargo.toml). Within an interactive build, non-interactive
  subcommands also get the core ring; that is bounded by design
  (overwrite-oldest, package 01).
- Linux note for the lab machine: cpal/ALSA needs `libasound2-dev` at build
  time. Document in `04`; build failure of the interactive feature on a
  machine without it is acceptable (matches existing "interactive is never
  built in CI" stance).
- Adding cpal changes `Cargo.lock`; regenerate and **commit the lock file**
  in the same change, or every `--locked` gate in package 04 fails before
  any code runs.
- cpal 0.15 is not in the local registry cache, so its API names are
  unverified until `cargo fetch` at implementation time. Verify then, before
  writing code: `cpal::traits::{HostTrait, DeviceTrait, StreamTrait}`,
  `default_host()`, `Device::default_output_config()`,
  `build_output_stream`, `SampleFormat::{F32, I16}`, and that `Stream` is
  `!Send` (keep the stream owned by `AudioSink` on the main thread).

### 2. New module `crates/ramdiff/src/audio.rs` (cfg `feature = "interactive"`)

An `AudioSink` owning the cpal stream and a shared state:

- **Shared buffer**: `Arc<Mutex<VecDeque<i16>>>` of interleaved stereo
  samples plus `Arc<AtomicBool>` mute flag. A `Mutex` is acceptable here —
  no new lock-free dep — but harden it: pre-allocate the `VecDeque` to
  high-watermark capacity (`with_capacity`, no regrowth under the lock),
  resample **outside** the lock (lock only to extend/trim the queue), and
  the audio callback uses **`try_lock` with silence fallback** on contention
  so the real-time thread never blocks on a long producer hold.
- **Device/config selection**: default output device, default output config.
  Do **not** demand 32 kHz from the device; resample from
  `refwork_emu::AUDIO_SAMPLE_RATE_HZ` to the device rate with **linear
  interpolation** (f32 math is fine in ramdiff; the no-float rule is
  refwork-emu-only). The resampler must be **stateful across `push` calls**
  — it carries the fractional phase and the last stereo input pair from the
  previous chunk. A per-chunk phase reset would inject a discontinuity at
  every ~16 ms push boundary (a frame-rate buzz, not an occasional artifact).
  Support `f32` and `i16` output sample formats; other formats → fall back
  to disabled-with-note.
- **Callback behavior**: pop samples; on underrun fill with silence (no
  blocking, no panics); when muted, output silence but still pop at the same
  rate so the buffer level stays governed by real time.
- **Pacing correction (required)**: the loop currently limits updates to
  16 ms (`record.rs:531`) — that is a 62.5 fps ceiling, which on a fast Mac
  produces ~33,280 pairs/s against 32,000 consumed: a systematic +4% surplus
  that would force a watermark trim (an audible ~150 ms skip) every few
  seconds. Change the limit to **16,667 µs**
  (`Duration::from_micros(16_667)`, ≈60 fps). Residual mismatch vs the
  emulator's 60.0988 fps NTSC rate is ~0.16% — one trim per ~90 s worst
  case, acceptable for a hand-play tool. If that still proves audible on
  hardware, implementer's discretion to slew the resample ratio by buffer
  level (±0.5%), but do not build that speculatively.
- **Producer-side watermark (backstop, not steady-state)**: after pushing
  each frame's drained samples, if the queue exceeds a high watermark
  (~250 ms of device-rate samples), drop oldest down to a target (~100 ms)
  and count it. With the pacing correction this fires rarely (window
  occlusion, debugger pauses), not periodically.
- **Failure = degrade, never abort**: any error building the device/stream
  prints one stderr note (`interactive: audio unavailable (<cause>) —
  continuing silent`) and yields a no-op sink. Mirrors the gamepad fallback
  style (`record.rs:546-572`).
- On drop/shutdown, if `dropped` counters are nonzero, print one summary
  line. For `core.audio_dropped_pairs()`, report the **delta since the
  post-replay baseline** (see §3): a resumed session legitimately overflows
  the ring during replay, and reporting the raw total would print scary
  numbers that reflect intended behavior.

`AudioSink` public surface (keep it minimal):

```rust
pub fn open() -> AudioSink;               // never fails; may be a no-op sink
pub fn push(&mut self, samples: &[i16]);  // resamples + enqueues
pub fn set_muted(&mut self, muted: bool);
pub fn muted(&self) -> bool;
```

### 3. Interactive loop wiring (crates/ramdiff/src/record.rs)

- Construct the sink after the resume replay completes and, before the live
  loop starts, **loop `take_audio_samples` until it returns 0, discarding**
  (the replay may have filled the ring; a single drain call moves at most
  one scratch-buffer's worth and would leave a stale burst that plays at
  session start). Then record `audio_dropped_pairs()` as the baseline for
  the shutdown summary.
- Per live frame, right after `core.run_one_frame(pad)`
  (`record.rs:583`): drain into a reusable `[i16; N]` scratch buffer
  (N ≥ 2×ring capacity is overkill; 4096 i16 values per call, looped until
  drained) and `sink.push(...)`.
- **M mute toggle**: `window.is_key_pressed(Key::M, KeyRepeat::No)` flips the
  sink's mute flag. Update the window title to reflect state — extend the
  existing title (`record.rs:521`) to
  `"ramdiff record [interactive] — F5=dump, M=mute, Esc=quit"` and use
  `window.set_title(...)` on toggle to append/remove a `[muted]` suffix.
  Mute must not touch the pad word, the padlog, or the core.
- **New CLI flag `--no-audio`** (record subcommand, interactive-only like
  `--gamepad`, `crates/ramdiff/src/main.rs:177-192`): skip sink construction
  entirely. Update the usage text (`main.rs:103-107`) and the module doc
  header keyboard table (`record.rs:14-33`, `main.rs:30-54`) to document
  audio, `M`, and `--no-audio`.

## Tests

- Unit-test the resampler (stateful struct: in-rate, out-rate, carried
  phase + last pair; i16 in/out via f32 lerp): identity at equal rates,
  length ratio at 32k→48k, channel interleaving preserved, empty input, and
  a **continuity test** — resampling one long input split at arbitrary chunk
  boundaries must produce exactly the same output as resampling it unsplit.
- Unit-test the watermark/drop policy on the queue type (pure logic extracted
  from the sink so it tests without a device).
- Mute-flag logic (toggle semantics) if extracted; the cpal callback itself
  is not unit-testable without a device — keep it thin (pop + optional
  zeroing) so inspection suffices.
- No test may require an audio device: everything device-facing stays in the
  thin `open()` path that tests don't call.

## Acceptance Criteria

0. `cargo test --locked -p ramdiff` (no features) green and byte-identical
   behavior (audio code fully feature-gated).
1. `cargo test --locked -p ramdiff --features interactive` green on the lab
   Mac, including resampler and watermark tests.
2. `cargo build --locked -p ramdiff --features interactive` succeeds on
   macOS; Linux build documented (needs ALSA headers) but not gating.
3. Manual (package 04 checklist): audio audible, M toggles, `--no-audio`
   silent, missing/failing device degrades with a single stderr note, no
   frame-pacing regression (window still responsive at ~60 fps).
4. A resume session does not replay stale audio from the replay phase.

## Out Of Scope

- Audio-clock-driven frame pacing (the loop stays minifb-limited; the
  16 ms → 16,667 µs correction in §2 is a constant fix, not clock-driven
  pacing).
- Recording audio to disk; volume control beyond mute.
- Non-interactive subcommands emitting audio.
