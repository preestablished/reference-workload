# Emulator Performance Profile

Status: synthetic host lane measured 2026-07-11; operator-private first-room and
matched KVM calibration are still required before this request can close.

## 1. Method And Reproducibility

`refwork-emu-bench` is a host-only workspace binary. It loads a ROM and optional
padlog before measurement, performs a fixed warmup/input prefix, then measures
only `Core::run_one_frame` plus `blit_completed_frame`. It emits one privacy-safe
JSON record with hashes, frame counts, elapsed time, fault state, and a final
state proof. It adds no dependency or default feature to `refwork-emu`.

The authoritative instruction lane uses `perf stat instructions:u`, pinned to a
single logical CPU. Perf begins disabled; the benchmark sends `enable` immediately
before the frame loop and waits for `ack`, then sends `disable` immediately after
the loop and waits again. Wall time is collected in a separate uncounted run so
clock and perf-control instructions do not contaminate one another. Three
repetitions at 100/200/300 frames test repeatability and linearity.

Sampling uses `instructions:u` with DWARF call graphs and the identical interval
gate. Inclusive call trees establish mutually exclusive top-level ownership;
exclusive symbol samples explain hot functions without double-counting inlined
callees. Structural guesses are not converted into retired instructions.

Clean-checkout commands and private-intake boundaries are in
`tools/refwork-emu-perf/README.md`. The complete public aggregate and environment
are in `docs/emulator-performance-profile-data/synthetic-20260711.md`.

## 2. Results And Calibration

### Synthetic host lane

- steady marginal cost: 24,356,288–24,356,301 user instructions/frame;
- first 100 boot frames: approximately 24.187M instructions/frame;
- 300-frame steady wall median on this noisy host: 4.72 ms/frame
  (3.61–5.35 ms observed range);
- exact instruction totals jittered by 26–2,295 instructions per window despite
  unscaled counting, a required reproducibility finding;
- 4,047-sample steady profile, zero lost samples, 99.99% top-level attribution.

| Subsystem | Share | Estimated instructions/frame | Notes |
|---|---:|---:|---|
| PPU scanline rendering | 83.41% | ~20.32M | background line 19.37%; pixel composite 17.23% |
| CPU interpreter | 8.24% | ~2.01M | includes inlined addressing/ALU and most bus timing |
| APU catch-up | 6.35% | ~1.55M | SPC700 visible at 1.05%; DSP register read 0.38% |
| scheduler/blit/other core | 1.99% | ~0.48M | direct `run_one_frame` ownership; rounding overlaps 0.01% |

Bus read/write symbols account for at least 1.15% of the host lane, but
`mem_speed` is inlined and cannot be separated honestly from the 8.24% CPU
parent with this sampling build. CPU dispatch/addressing/ALU and APU DSP details
therefore have lower-bound visible samples, not fabricated exhaustive splits.

### Host↔guest calibration

The historical hypervisor result is 27.8M KVM guest-mode instructions/frame and
90–115 ms/frame for the whole guest. Subtracting the synthetic host result gives
a purely illustrative 3.44M instructions/frame (12.4%) gap, but this is **not an
accepted residual**: the ROM/window, target artifact, harness work, guest image,
and event interval were not proven matched. No private first-room ROM or padlog
is present in this checkout, and no matching KVM run was available. The request's
calibration criterion remains open; an in-guest perf count is not a substitute
for dh-detclock's host-side KVM event.

## 3. Ranked Optimization Candidates

No optimization is implemented by this work. “Maximum” assumes complete removal
of a non-overlapping measured share and is only an Amdahl bound.

| Rank | Candidate hypothesis | Evidence and plausible bound | Blast radius |
|---:|---|---|---|
| 1 | Reduce PPU per-pixel composition/background work while preserving exact pixels | PPU owns 83.41%. A 2x PPU improvement yields at most ~1.72x overall; 4x yields ~2.57x; complete removal bounds at ~6.0x. | frame-content-preserving only if proven bit-exact; icount-changing, so all absolute-icount/vns assets re-baseline |
| 2 | Reduce CPU interpreter dispatch/addressing/bus overhead | CPU parent owns 8.24%; complete removal bounds at ~1.09x. `mem_speed`/addressing are inlined, so profile a chosen design before pricing below this parent bound. | intended frame-content-preserving; icount-changing |
| 3 | Reduce APU SPC700/DSP catch-up work | APU owns 6.35%; complete removal bounds at ~1.07x. Real-game audio may differ materially from synthetic. | audio/state behavior must remain exact; icount-changing |

These wins overlap only at the top-level rows shown; sub-symbol shares must not
be added to their parent. A candidate is viable for a follow-up bead only after
the private workload confirms a measurable share and the hypothesis has a
bit-exact acceptance benchmark.

## 4. Determinism And Re-baseline Price

Every credible speedup above changes host retired instructions/frame even when
WRAM/framebuffer/audio state stays bit-identical. Frame-quantized input and
frame-content hashes should survive a proven semantics-preserving change.
Absolute-icount epoch chains, icount-addressed snapshots, vns-budget runs, and
~25M calibrated cap fixtures do not.

The optimizing `refwork-emu` owner should bump emulator build/revision identity
and publish old/new frame-content equivalence. The reference-workload image owner
should rebuild and identify the image. Determinism-hypervisor should regenerate
epoch/cap fixtures and record the new frame→icount distribution. Snapshot-store
and consumers should version or reject incompatible absolute-icount snapshots.
The bridge should remeasure pacing but should not own hypervisor baselines.

Do not implement any candidate until those owners accept the bill and the A1
frames-versus-vns seam is decided or explicitly shown irrelevant.

## 5. Recommendation And `38b6` Answer

The synthetic profile does not make a robust 6–7x improvement plausible. PPU is
large enough that its impossible “remove all PPU work” bound approaches 6x, but
even a very aggressive 4x PPU improvement yields only ~2.57x overall. Reaching
16.7 ms from 90–115 ms would require near-elimination of rendering plus major
non-PPU and/or guest-overhead changes. The host wall result also shows that the
historical guest wall cost is not explained by this host emulator build alone.

Recommendation: do not optimize yet. First complete the operator-private
first-room profile and a matched KVM calibration. Then file only candidates that
remain material on the real workload, with explicit re-baseline owners and the
A1 dependency.

## Tracking And Handback

- busy-scene profiling fallback: `refwork-4nv`;
- PPU candidate: `refwork-rbz`;
- CPU/bus candidate: `refwork-0um`;
- APU candidate: `refwork-hbh`;
- determinism-hypervisor pointer: `38b6` comment
  `019f4f53-cab3-7365-a5cd-07ae83ae3b1d`.

The candidate beads inherit the blast-radius and re-baseline requirements above;
they do not authorize optimization as part of this profiling request.
