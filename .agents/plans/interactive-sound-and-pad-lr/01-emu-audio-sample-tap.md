# Package 01 — refwork-emu: Feature-Gated Audio Sample Tap

## Why

The S-DSP pipeline is complete (BRR decode, Gaussian interpolation,
ADSR/GAIN, noise, echo, PMON, full mixing) and runs during every frame; the
final stereo sample is computed and discarded in `Dsp::step_sample`
(`crates/refwork-emu/src/apu/dsp.rs:1170-1181` — `_final_l`/`_final_r`,
retained only under `introspect` as `last_out_l`/`last_out_r`). This package
adds a capture ring so a frontend can drain the stream. Capture only — no
synthesis changes.

## Changes

### 1. New cargo feature `audio` (crates/refwork-emu/Cargo.toml)

```toml
[features]
introspect = []
# Host-frontend audio sample capture (ring buffer + drain API). Compiled out
# of the guest binary and every hashed/perf lane: with the feature off, the
# build is byte- and icount-identical to before.
audio = []
```

No dependencies. Do NOT piggyback on `introspect` (that feature is "test-only
direct state access" and is enabled by lanes that must not grow an audio
ring, e.g. ramdiff's non-interactive replay uses `introspect` already via
`crates/ramdiff/Cargo.toml`). Nothing here changes when `audio` is off — all
new code is `#[cfg(feature = "audio")]`.

### 2. Sample ring in `Dsp` (crates/refwork-emu/src/apu/dsp.rs)

- Fixed-capacity ring of interleaved stereo `i16` pairs, stored as an
  **inline fixed array** (`[i16; 8192]` = 4096 stereo pairs, 16 KiB) plus
  head/len indices, matching the existing by-value buffer style
  (`echo_buf_l`/`echo_buf_r: [i16; ECHO_BUF_MAX]`, `dsp.rs:294-295`) — no
  heap `Vec`, no per-frame allocation (D8), no floats (D4). 4096 pairs
  (~128 ms at 32 kHz) is comfortably above the ~532 pairs/frame a 60 fps
  drain sees (357,368 master cycles/frame × 1024/21477 ÷ 32 ≈ 532.5).
- Overflow policy: **overwrite oldest** and increment a `dropped_pairs: u64`
  counter. Bounded memory even when nobody drains (non-interactive lanes
  built with the feature on, and the fast resume-replay loop in
  `crates/ramdiff/src/record.rs:487-513`).
- Push site: exactly where `_final_l`/`_final_r` are discarded today
  (`dsp.rs:1176` area). The existing computation must not move or change —
  add a `#[cfg(feature = "audio")]` push of the already-computed values.
  Rename `_final_l`/`_final_r` to `final_l`/`final_r` only if the
  underscore-prefix would otherwise warn when the feature is on; keep the
  no-feature build warning-free too (`cfg_attr`/scoped `let _ =` as needed).
- Drain API on `Dsp`:

```rust
#[cfg(feature = "audio")]
/// Move up to out.len()/2 pending stereo pairs into `out` (interleaved
/// L,R). Returns the number of i16 values written (always even).
pub fn drain_audio(&mut self, out: &mut [i16]) -> usize
```

### 3. Passthroughs

- `Apu` (crates/refwork-emu/src/apu/mod.rs — `dsp` field is already `pub`,
  `mod.rs:109`): add `#[cfg(feature = "audio")] pub fn drain_audio(...)`
  delegating to the DSP. Keep the passthrough anyway (don't reach through
  fields from Core) to match existing layering.
- `Core` (crates/refwork-emu/src/core_impl.rs — `bus.apu` is `pub`,
  `bus.rs:52`):

```rust
#[cfg(feature = "audio")]
/// Drain stereo i16 samples (interleaved L,R) synthesized since the last
/// call. Native rate: 32000 Hz nominal (1 sample / 32 SPC cycles,
/// apu/mod.rs DSP_CLOCKS_PER_SAMPLE); ~532 pairs per 60 fps frame.
pub fn take_audio_samples(&mut self, out: &mut [i16]) -> usize
```

Also expose `#[cfg(feature = "audio")] pub fn audio_dropped_pairs(&self) -> u64`
for the frontend's shutdown diagnostics.

### 4. Sample-rate constant

Export `#[cfg(feature = "audio")] pub const AUDIO_SAMPLE_RATE_HZ: u32 = 32_000;`
from the crate root (`lib.rs`), derived from the existing clock constants
(`apu/mod.rs:66-72`: SPC 1.024 MHz nominal / `DSP_CLOCKS_PER_SAMPLE = 32`).
The frontend must not hardcode 32000 independently. (The model's true rate
is ≈32,000.4 Hz — 0.0013% off the constant, inaudible.) Beware a
pre-existing comment drift while in this file: `apu/mod.rs:33-35` and `:65`
say ≈17,028–17,029 SPC cycles/frame, but the constants actually yield
≈17,038.9; do not "correct" new code to match the stale comments.

## Tests (crates/refwork-emu, `#[cfg(all(test, feature = "audio"))]`)

- Ring unit tests: fill/drain round-trip, interleaving order (L then R),
  partial drain, overflow overwrites oldest and counts drops, drain into an
  odd-length slice writes an even count.
- Wiring test at the **`Apu` level** (refwork-emu has no `Core`-constructing
  tests and no synthetic-ROM fixture — the ROM builder lives in xtask, which
  depends on this crate, so a Core-level test is not feasible here; existing
  precedent is `advance_master_cycles_no_panic`, `apu/mod.rs:942`, which
  constructs an `Apu` and advances it directly): construct an `Apu`, advance
  N master cycles, assert the drained pair count matches the **computed**
  expectation `N × SPC_NUM / SPC_DEN / DSP_CLOCKS_PER_SAMPLE` within an
  explicit small tolerance (accumulator remainder), not a hardcoded ~532.
  This pins that the tap is wired into APU stepping; `Core::take_audio_samples`
  is a trivial delegation chain exercised by ramdiff (package 02).
- Determinism guard test (also `Apu`-level): advance two independently
  constructed `Apu`s through the identical cycle sequence, assert the drained
  sample streams are identical (samples are part of deterministic execution
  today; this pins it).

## Acceptance Criteria

0. Default-features build: no new machine code compiled in (everything added
   is `#[cfg(feature = "audio")]`), so behavior and host icount are
   unchanged; `cargo test --locked -p refwork-emu` still green and the
   determinism lanes (criterion 3) pass untouched. Do NOT gate on raw
   artifact byte-identity — inserting cfg'd code shifts line numbers, which
   moves panic-`Location` span metadata in the rlib even though the compiled
   behavior and instruction stream are unchanged.
1. `cargo test --locked -p refwork-emu --features audio` green, including the
   new tests above.
2. No floats, no threads, no clocks, no RNG, no per-frame allocation in any
   new code (ring allocated in constructor only).
3. `cargo test --locked -p refwork-harness --test mock_agent` and the xtask
   determinism test pass unchanged (they build without `audio`).
4. Rustdoc on the three new public items states the interleaving, the rate
   constant's origin, and the overflow policy.

## Out Of Scope

- Any change to DSP synthesis, mixing, timing, or ARAM behavior.
- Resampling (host-side; package 02).
- Exposing audio over the harness/protocol surface (fd3, regions) — the tap
  is a host-frontend affordance only.
