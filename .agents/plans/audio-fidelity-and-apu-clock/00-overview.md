# Audio Fidelity And APU Clock Correctness (Plan)

## Outcome

Interactive audio (`ramdiff record --interactive`, added by
`.agents/plans/interactive-sound-and-pad-lr/`) currently sounds terrible.
This plan fixes the two proven root causes so the example game's audio is
clean, at correct pitch and tempo, with stable A/V sync — while handling the
determinism consequences per program governance.

## Diagnosis (completed — three independent consultants, all confirmed)

Method: replayed the operator's 2026-07-16 hand-play session (1,181 frames
— the session in which the terrible audio was reported; note this is NOT
the canonical m6 `discovery-01`, see below) headless, drained the raw
32 kHz DSP tap per frame, and analyzed the WAV; then two code-level
investigations proved mechanisms with in-tree tests.

**Session naming (important):** the canonical m6 `discovery-01` is the
45,230-frame session with 16 WRAM dumps. The 2026-07-16 hand-play rotated
it aside (currently `discovery-01.bak-6` under the private root — identify
it by frame/dump count, not directory name, as future rotations renumber).
The 1,181-frame session is the listen/diagnosis artifact only; every epoch
gate below anchors on the canonical 45,230-frame session.

**Root cause 1 — APU clock overshoot (rate/tempo).**
`Apu::advance_master_cycles` (`crates/refwork-emu/src/apu/mod.rs:795-828`)
steps whole SPC700 instructions past its integer-accumulator budget and
never carries the excess back. Every `apu_catch_up` call — 262×/frame per
scanline plus one per CPU access to $2140-$217F — leaks up to
(instruction_length − 1) cycles into the SPC clock, timers, and DSP sample
accumulator. Proven numbers: a single 1-second advance produces exactly
32,000 samples (the arithmetic is correct); 30-cycle chunked advances of
the same total produce 156,603 (4.89×). The real session measured
mean 560.75 stereo pairs/frame (nominal 532.5, +5.3%), min 543, max 1,238
(2.32×, a busy-wait handshake frame). Consequences: audio contains ~5.3%
more audio-seconds than wall-seconds (playback must trim ~150 ms every few
seconds — the dominant "terrible"), and SPC timers run traffic-dependently
fast, so music tempo is wrong and unstable. **No host-side mitigation
exists** — the surplus varies with port traffic, so any playback strategy
just chooses a different artifact.

**Root cause 2 — DSP synthesis defects (crunch).** Confirmed by rendering a
synthetic filter-0 BRR triangle through the production DSP: it comes out a
square wave (constant for each 16-sample block, then a ~3,500-count jump),
matching the field capture's per-sample jaggedness (mean |Δ| ≈ 0.65×RMS).
Ten divergences from S-DSP hardware behavior, four of them primary:
D1 Gaussian interpolator taps a frozen buffer position (never advances
through the block); D2 BRR filter-2/3 coefficients wrong (filter 3
unstable, rails); D3 GAUSS table is not the published table (kernel gain
1.1-1.4× with ripple); D4 the two newest Gaussian taps are swapped.
Details and D5-D10 in `01-dsp-fidelity-fixes.md`.

Evidence tests are already in the working tree (uncommitted), documenting
both mechanisms; see 01/02 for their disposition.

## Two tracks, two gates

| Track | File | What | Gate |
|-------|------|------|------|
| A | `01-dsp-fidelity-fixes.md` | D1-D10 synthesis fixes | Likely **state-epoch-free** — verified by a full hash-chain compare over the canonical 45,230-frame replay (subsumes the 16 dumps AND the 1,005-capture frozen corpus). If not bit-identical, the divergent subset folds into Track B's epoch cut. Host-icount still changes regardless (see 01). |
| B | `02-apu-clock-debt-fix.md` | `spc_debt` carry in `advance_master_cycles` | **Epoch-breaking by design** (timers shift → CPU-visible → WRAM/hashes). Per `docs/emulator-performance-profile.md:97-98` ("Do not implement any candidate until those owners accept the bill"), **implementation is blocked on an explicit operator decision.** |
| — | `03-epoch-cut-protocol.md` | The re-baseline bill and 10-step rollout | Operator-owned decision, recorded in `.agents/decisions/`. |
| — | `04-verification.md` | Gates, WAV-based listen tests, tooling | Closes both tracks. |

Order: Track A first (it fixes the crunch the operator hears most, possibly
with zero epoch cost). Track B lands at the corpus boundary — before
discovery-02 is recorded — if and when the operator accepts the bill.

## Hard constraints

- Determinism contract (refwork-emu): no floats/threads/clocks/RNG, no
  per-frame allocation; all fixes here are integer-only changes to existing
  code paths — no new features, no gating (a behavior toggle would
  bifurcate hash universes; assessed and rejected, see 03).
- All in-repo determinism gates are run-vs-run and survive both tracks;
  what breaks (Track B, and Track A only if the byte-compare fails) is the
  recorded-artifact layer — enumerated in 03.
- Clean-room naming: never the commercial game name; "the example game".
- Suite hygiene: main must stay green. Evidence tests that assert
  currently-buggy behavior are committed passing-as-documentation (clock —
  these double as a governance tripwire: any ungated debt-carry
  implementation turns a green test red, see 02) or ignored with the
  reason-string form `#[ignore = "documents divergence Dn; fix gated on
  this plan"]` (fidelity), each flipped/un-ignored in the same commit as
  its fix. Caution: CI's `--include-ignored` is scoped to the xtask
  determinism test only — do not extend it to refwork-emu invocations
  while ignored fidelity tests are in the tree.
