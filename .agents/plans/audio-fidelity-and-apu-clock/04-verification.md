# Verification And Tooling

## Tooling: commit the raw-tap dumper

The diagnosis relied on a scratchpad tool that replays a padlog headless
and writes the raw 32 kHz tap to WAV plus per-frame/signal statistics.
Commit it as a permanent diagnostic — either `ramdiff record --dump-audio
<out.wav>` (non-interactive replay path, requires `refwork-emu/audio`;
plumb like `--frames`) or an `#[ignore]`d-by-default xtask. It must print:
frames, pairs/frame (min/max/mean), dropped pairs, clipped-sample count,
per-second RMS/peak/mean|Δ| table, and write PCM s16le stereo 32000 Hz.
No private paths or ROM names in code, docs, or output.

## Track A verification (DSP fidelity)

1. `cargo test --locked -p refwork-emu --features audio` — fidelity tests
   un-ignored and green; 206 pre-existing tests still green.
2. Synthetic: `fidelity_pure_tone_renders_smoothly` bounds per-sample Δ.
3. Real session: regenerate the discovery-01 raw-tap WAV; loud sections
   (previously mean|Δ|≈800 at RMS≈1230) must drop to mean|Δ| < 0.3×RMS
   with no 16-sample staircase pattern; zero clipping.
4. Epoch-visibility gate: byte-compare the 16 replayed WRAM dumps (01).
5. Operator listen: raw-tap WAV, then live interactive session — crunch
   gone. (Rate/tempo issues remain until Track B: expect trims/tempo drift
   still present; judge timbre only.)

## Track B verification (clock)

1. Flipped overshoot tests green; extended chunking tests green; debt-bound
   and halt-freeze tests green.
2. Real-session replay: pairs/frame 532-533 for EVERY frame; the WAV
   duration matches emulated time (the 1,181-frame hand-play session →
   19.65 s ± 0.1%).
3. `EMU_VERSION` propagation verified: the bumped string appears in the
   harness meta region / determinism report output (not just in lib.rs).
4. Determinism gates per 03 step 4.
5. Operator listen, live interactive: no periodic trims (watermark trim
   counter 0 or ~0 over a 5-minute session), stable tempo, correct pitch,
   A/V sync stable. The interactive-sound plan's closed-loop rate control
   (±0.5%) is now sufficient authority — verify queue depth settles.

## Combined acceptance

- Full workspace-minus-harness suite, clippy both feature configs, xtask
  deny/determinism — green.
- On-hardware (operator, F310 + lab Mac): example game audio is clean,
  correct pitch, stable tempo, no skips, mute works — closes the original
  "sounds terrible" report and supersedes the audio lines of the
  refwork-279 checklist. Note: this combined state is unreachable until
  the operator accepts Track B's epoch cut — Track A alone fixes timbre
  but leaves the rate/tempo artifacts. If the decision stalls, that is the
  standing audio quality.
- Beads: track beads per package; close with gate evidence; epoch decision
  document referenced from the Track B bead.
