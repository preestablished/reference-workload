# Interactive Sound Output, Mute Hotkey, And Full Pad Mapping (Plan)

## Outcome

After this plan is implemented, `ramdiff record --interactive` (and its
wrapper `tools/record-ramdiff`):

1. **Plays the game's audio** on the host (macOS lab Mac; Linux lab machine
   where ALSA is available). The emulator core already synthesizes the full
   S-DSP stereo stream every frame and throws it away
   (`crates/refwork-emu/src/apu/dsp.rs:1170-1181`); this plan taps that
   existing stream and plays it. No new synthesis code.
2. **`M` toggles mute/unmute** during an interactive session. Mute is purely
   host-side: it never enters the pad word, the padlog, or emulator state.
3. **All 12 SNES pad buttons work from the Logitech F310**, including L and R,
   which are currently unusable via the controller even though the mapping
   tables nominally cover them. The example game uses L and R, so this is a
   hard requirement, verified on hardware.

## Packages

| File | Package | Depends on |
|------|---------|------------|
| `01-emu-audio-sample-tap.md` | `refwork-emu`: feature-gated audio sample ring + `Core::take_audio_samples` | — |
| `02-ramdiff-audio-output.md` | `ramdiff`: cpal playback sink, M mute hotkey, `--no-audio` | 01 |
| `03-gamepad-lr-and-mapping.md` | `ramdiff`: pad-debug diagnostic, L/R mapping fix, full-mapping audit | — |
| `04-verification-and-docs.md` | Gates, on-hardware checklist, doc updates | 01–03 |

Packages 01 and 03 are independent and can be implemented in parallel;
02 needs 01; 04 closes out.

## Hard Constraints (read before touching anything)

- **Determinism contract** (`crates/refwork-emu/Cargo.toml:7`, ARCHITECTURE.md §1):
  refwork-emu has zero runtime dependencies. D1 no threads/async, D2 no
  clocks, D3 no RNG, D4 no floats (CI token-scan enforced), D8 no per-frame
  allocation (CI counting allocator). Everything added to refwork-emu must
  honor all of these — the audio tap is integer-only (`i16`), allocated once
  at construction, and behind a new cargo feature so the default build is
  **byte- and icount-identical** (icount changes re-baseline absolute-icount
  epoch chains; see `docs/emulator-performance-profile.md:82-88`).
  **(2026-07-16: this "byte- and icount-identical" default-build guarantee
  is superseded for the APU clock epoch — see
  `.agents/decisions/2026-07-16-apu-clock-epoch-cut.md`.)**
- **Frame hashes are unaffected by design**: `frame_hash = blake3(wram ‖ fb)`
  (`crates/refwork-hash/src/lib.rs:26-31`) does not cover APU/DSP/ARAM, and
  the tap only captures values already computed. `refwork-verify` double-run
  and the harness mock-agent fixture must still pass untouched.
- **Clean-room naming**: never write the example game's commercial name or
  ROM filename into this repo (existing GATE-RECORD-ASK1 discipline,
  `tools/record-ramdiff:42-43`). Plan docs, commit messages, code comments:
  "the example game" / "the real ROM" only.
- **Host-side-only UX**: mute state, audio device failures, resampling — none
  of it may alter the pad word stream or emulation. An audio-device failure
  degrades to silent (with a stderr note), exactly like a missing gamepad
  degrades to keyboard-only today (`crates/ramdiff/src/record.rs:546-572`).

## Cross-Repo Requests

**None needed.** Both seams (sample capture in `refwork-emu`, playback/input
in `ramdiff`) live in this repo. `cpal` is a crates.io dependency of the
`interactive` feature only, not a sibling-repo integration. The
`.agents/requests/` flow is therefore not exercised by this plan.

## Verification Gates (details in 04)

- G1: `cargo test --locked --workspace` green, including new unit tests.
- G2: Determinism lanes untouched: `cargo test --locked -p refwork-harness
  --test mock_agent` and the xtask determinism test pass unchanged. With the
  `audio` feature off, no new machine code is compiled in (all additions are
  `#[cfg(feature = "audio")]`), so host icount and the perf-epoch baselines
  are unaffected. Note: raw artifact bytes may still differ (line-number
  shifts move panic-`Location` span metadata); the gate is behavioral +
  icount identity, not a byte-compare of the rlib.
- G3: On-hardware (lab Mac + F310): audio audible in the example game,
  M toggles mute, and **all 12 buttons** — including L/R — register in a
  pad-debug session and in-game.
