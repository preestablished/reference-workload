# 02 — DSP (fixed-point), CPU↔APU scheduling, ApuStub retirement

**Depends on:** 01 (SPC700 core, ARAM, timers, ports).

The DSP is computed even though no audio is ever heard (headless platform):
games gate progress on audio-engine state, and the engine runs on the SPC700
reading DSP status. Skipping it is the "audio-unit shortcuts break game
logic" risk in IMPLEMENTATION-PLAN.md — full APU is *required* for M2
acceptance; `ApuStub` is banned from it.

## Deliverables

1. `crates/refwork-emu/src/apu/dsp.rs` — the 8-voice DSP, **integer/
   fixed-point only (D4)**:
   - BRR sample decode (4-bit ADPCM, all four filter modes, shift, loop
     flags) — pure integer.
   - Pitch/playback: 4-tap gaussian interpolation using the documented
     512-entry integer coefficient table (public hardware references publish
     the exact table — transcribe it, it is integer by nature), Q-format
     accumulation in `i32`.
   - Envelopes: ADSR and GAIN (direct, linear/bent-line/exponential
     inc/dec), integer rate table per public docs.
   - Echo: ring buffer in ARAM (the documented region-overlap hazards apply
     — emulate writes faithfully, games rely on them), 8-tap FIR in
     `i16`/`i32` with the documented clamp/overflow semantics.
   - Noise: the documented 15-bit LFSR clocked by the rate table —
     deterministic machine state, not RNG (D3-compliant by construction).
   - Pitch modulation, key-on/key-off, ENDX/source directory, main/echo
     volume mix with hardware clamp behavior.
   - Output sink: samples are computed and **discarded** after updating
     observable state (ENDX, OUTX, ENVX registers — games poll these). Keep
     a `cfg(feature = "introspect")` tap that exposes the last output frame
     for tests.
2. Scheduling — `apu/mod.rs` orchestration:
   - Master-clock ratio: SPC700 runs at its nominal clock against the
     21.477 MHz master clock; DSP emits one stereo sample per 32 SPC clocks
     (32 kHz). Use an **integer accumulator** (add master cycles, drain in
     fixed ratios) — same pattern as `timing.rs`; no floats, no drift.
   - Catch-up model: the APU advances (a) whenever the CPU touches
     $2140–$2143, and (b) at end of scanline, to the current master-cycle
     timestamp. This bounds divergence windows deterministically — the
     *schedule* is a pure function of emulated state, satisfying the
     contract regardless of accuracy.
3. **Retire `ApuStub`**: bus routes $2140–$2143 to the real APU; delete
   `ApuStub`, the `Apu::Stub` plumbing, and the
   `FrameFlags::APU_STUB_ACCESS`/`APU_STUB_HANDSHAKE` flags plus their
   harvest in `core_impl.rs` (~lines 154–162). Grep tests/docs for the flag
   names — M1's synthetic-ROM tests may assert them.
4. Synthetic-ROM extension (`xtask/asm` + `xtask/src/synth_rom.rs`): add an
   APU exercise segment — upload a small SPC program via the IPL protocol,
   key on a BRR voice, poll ENDX/port echo, fold results into the drawn
   pattern so APU state lands in the frame hash. This keeps CI's
   game-content-free workload covering the new unit forever.

## Acceptance (package-local)

- DSP unit vectors: hand-built BRR blocks decode to expected PCM; envelope
  rate/step vectors match public-doc tables; gaussian table checksum test;
  echo FIR clamp cases. All integer-asserted, no goldens from other
  emulators.
- Extended synthetic ROM: 10k-frame double-run hash gate still green
  (now covering APU state), zero-alloc gate still green (DSP buffers
  allocated in `Core::new` path), deny gate green (**this is the package
  most at risk of a stray `f32`** — run `cargo xtask deny` early and often).
- IPL + engine smoke: the synthetic SPC program from deliverable 4 runs
  identically twice for 10k frames.
- `grep -r "ApuStub\|APU_STUB" crates/` returns nothing.
