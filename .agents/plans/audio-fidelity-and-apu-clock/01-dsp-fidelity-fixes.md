# Track A — DSP Synthesis Fidelity Fixes (D1-D10)

All in `crates/refwork-emu/src/apu/dsp.rs`. Ranked by audible impact; fix
in this order, flipping the corresponding `fidelity_tests` as each lands.
Line numbers reference the current file.

## D1 (Critical): Gaussian interpolator taps a frozen position

- Where: `gaussian_interp` (:748-773, tap selection :755-758), fed by the
  16-at-a-time block decoder (:665-719), call site :1181-1185.
- Now: blocks are decoded 16 samples at once into a 16-entry ring, leaving
  the write cursor at 0; the interpolator taps `buf[12..15]` for **all 16
  output ticks**, ignoring `brr_block_offset`. Output is piecewise-constant
  per block — the proven square-wave/crunch source.
- Hardware: taps are the four samples ending at the *playback* position.
- Fix: widen `BRR_BUF_LEN` to 32 (current + previous block so the 3-sample
  look-behind survives block boundaries); pass `brr_block_offset` into
  `gaussian_interp`; newest tap = `(buf_pos - 16 + block_offset) & 31`,
  taps at `newest-3..newest`.

## D2 (Critical): BRR filter-2/3 coefficients wrong; filter 3 unstable

- Where: `decode_brr_nybble` (:608-618).
- Now: f2 nets `o1·15/16 − o2·15/16` (hardware: `o1·61/32 − o2·15/16`);
  f3 nets `o1·29/8 − o2·7/4` — recursion pole at z≈3.05, rails to ±32767.
- Fix (hardware forms):
  `2 => s + (f1 << 1) + ((-(f1 * 3)) >> 5) - f2 + (f2 >> 4)`
  `3 => s + (f1 << 1) + ((-(f1 * 13)) >> 6) - f2 + ((f2 * 3) >> 4)`

## D3 (High): GAUSS table is not the published S-DSP table

- Where: :84-111. Current table ends at 1799; published ends at 0x519
  (1305), and every 4-tap kernel sums to ~2048 (unity after `>>11`). The
  current table's kernels sum 2270-2840 → 1.1-1.4× gain rippling with the
  pitch fraction.
- Fix: re-transcribe the published table verbatim; re-pin
  `gauss_table_checksum` (the current pin 290589 pins the wrong table). The
  kernel-sum test validates the transcription.

## D4 (High): Two newest Gaussian taps swapped

- Where: :765-768. `g2 = GAUSS[frac]`, `g3 = GAUSS[256+frac]` — hardware is
  the reverse. Swap the two lines. (Currently masked by D1.)

## D5 (Medium): BRR clamp/wrap semantics and invalid shift

- Where: :589-596, :626-628. Hardware computes in a halved domain, clamps
  to 16 bits, doubles with 16-bit wrap; shift 13-15 yields 0 / −4096.
  Code saturates at ±32767 and yields −32768 for negative invalid-shift.
- Fix: `let c = filtered.clamp(-65536, 65534); (c as i16) & !1` (with D2's
  filter forms); invalid shift → `if raw4 < 0 { -4096 } else { 0 }`.

## D6 (Medium): Noise LFSR direction and output level

- Where: `step_noise` (:933-936), injection (:1173-1179). Hardware:
  `new = (lfsr >> 1) | (((lfsr ^ (lfsr >> 1)) & 1) << 14)`; the noise
  sample is the 15-bit LFSR value sign-extended (routing — replacing the
  Gaussian output for NON voices — is already correct). Code shifts left
  with taps 14/13 and emits a full-scale ±0x4000 square.
- Fix: right-shift LFSR form above; sample =
  `((self.noise_lfsr << 1) as i16) as i32` — the 15-bit value shifted left
  into the 16-bit domain WITHOUT shifting back (full-scale ±32766, even
  values, matching how BRR samples are stored; blargg snes_spc and ares
  both do `(int16)(noise * 2)`). Do NOT use a `>> 1` form — that ships
  noise 6 dB quiet. Test: cross-check the first N samples against a
  hand-stepped reference LFSR.

## D7 (Medium): BRR filter history zeroed at loop point

- Where: :1141-1142. Hardware carries `old1/old2` across the loop seam;
  zeroing clicks once per loop iteration. Fix: delete the two lines.

## D8 (Low-medium): ADSR decay rate uses wrong bits

- Where: :838. `((adsr1 >> 3) & 0x07)` mixes the attack bit into the decay
  rate; hardware DR is bits 6:4. Fix: `((adsr1 >> 4) & 0x07)`.

## D9 (Low): Echo input divided by 128

- Where: :1283-1284. `(echo_in + fir*efb) >> 7` — hardware is
  `clamp16(echo_in + ((fir * EFB) >> 7))`. Echo is nearly silent today.

## D10 (Low): Mixing details

- :1204-1217, :1250: clamp the main accumulator to 16 bits after **each**
  voice add (code clamps once at the end); :1244-1247: apply MVOL to the
  voice sum only (echo×EVOL is currently double-scaled by MVOL); `& !1` on
  interp/voice outputs; `& !1` on echo-buffer writes (hardware masks the
  LSB); the Gaussian interpolator's intermediate 16-bit wrap after
  accumulating the first three taps, before the fourth (the documented
  interpolation-overflow quirk). Saturation/LSB-level fidelity — below
  audibility for this game, but in scope for D10's completeness claim.

## State-epoch visibility gate (decides whether Track A lands alone)

**SPC-visible channels, precisely** (verified): DSP state reaches the SPC —
and thence potentially the CPU/WRAM — only through the readable
ENVX/OUTX/ENDX registers. The echo buffer is a **core-private array**
(`echo_buf_l/r`, dsp.rs:302-303), NOT ARAM, so D9/D10 are not SPC-visible
in this core. Risk ranking: D8 highest (ENVX decay *timing* — the register
sound drivers classically poll), then D1-D6 via OUTX values. So these
fixes are *plausibly* CPU-invisible for this game but not provably so.

> **Correction (2026-07-16):** the claim above that the echo buffer is
> core-private and NOT ARAM is wrong. `dsp.rs` has a pre-existing echo
> write into ARAM alongside the in-memory `echo_buf_l/r` write (the write
> at the echo region, `aram[aram_off_l]`/`aram[aram_off_r]` and their +1
> byte pairs, immediately after the `echo_buf_l/r` update) — so D9/D10 ARE
> SPC-visible in principle via that ARAM region, not confined to a
> core-private array. This is moot in practice for this landing: the
> determinism epoch cut was operator-approved (see
> `.agents/decisions/2026-07-16-apu-clock-epoch-cut.md`) and both tracks
> (Track A DSP fidelity, Track B APU clock) landed together, so the gate
> question of "does Track A need to land alone" no longer applies. Left
> here as a rationale correction for anyone re-deriving the visibility
> analysis later.

**The gate** — one headless replay of the **canonical 45,230-frame
discovery-01 session** (currently `discovery-01.bak-6` after the
2026-07-16 rotation; identify by frame/dump count, not name) under the
fixed build:

1. Compute the per-frame hash chain (`frame_hash = blake3(wram ‖ fb)`,
   refwork-hash) for old and new builds over the full padlog; **equal
   final chain hash ⇒ bit-identical WRAM+framebuffer at every one of the
   45,230 frames** — this subsumes the 16 checkpoint dumps AND the
   1,005-capture cadence-45 frozen corpus in one shot.
2. Belt-and-braces on the actual artifacts: verify **exactly 16/16**
   checkpoint-dump byte-compares report OK (assert the count — the resume
   machinery silently skips frame-0 sentinels and shadowed files), and
   re-run the draft map-check requiring an identical 23/23 progression.
3. **All identical ⇒ Track A is state-epoch-free**: land on main. Even
   then: (a) **host icount still changes** (profile.md:84-88 — DSP inner
   loops are rewritten), so sibling icount-keyed assets get an epoch note
   per §4 regardless — this part of 03's bill item 4 applies to Track A
   too; (b) the operator should re-run the lab expect files once anyway —
   they cover different scenarios/input logs than this gate.
4. **Any divergence ⇒ do NOT land the full set**: bisect by re-gating with
   D8 reverted, then D2/D5/D6 reverted; land the invisible subset
   immediately (fast working audio) and fold only the visible remainder
   into Track B's epoch cut (03) so the artifact bill is paid once.

## Tests

- The four failing `fidelity_tests` already in the tree (committed with
  `#[ignore = "documents divergence D<n>; fix gated on this plan"]` until
  fixes land, then un-ignored in the fixing commits): `fidelity_brr_filter_coefficients_match_hardware` (with
  its `hw_ref_decode` reference decoder), `fidelity_gauss_tap_coefficient_assignment`,
  `fidelity_gauss_table_matches_published_values`,
  `fidelity_pure_tone_renders_smoothly` (end-to-end smoothness: synthetic
  triangle must render with bounded per-sample delta).
- Add per-fix unit tests where the above don't already pin the behavior
  (noise LFSR sequence vs hand-computed reference; loop-seam continuity;
  ADSR decay bit extraction; echo level).
- Existing 206 tests must stay green (register-level semantics unchanged).

## Acceptance criteria

1. All fidelity tests pass un-ignored; full `-p refwork-emu` suite green
   (all feature combos).
2. Headless raw-tap WAV of the 2026-07-16 hand-play session replay (04's
   tooling): loud sections show mean |sample-to-sample Δ| well under
   0.3×RMS and no 16-sample-periodic staircase; operator listen confirms
   no crunch.
3. State-epoch gate outcome recorded in this file (dated note):
   bit-identical, or the bisected subset landed with the remainder folded
   into Track B.
