# Synthetic-ROM Raw Aggregate — 2026-07-11

This is the privacy-safe aggregate derived from raw files under the gitignored
`target/refwork-emu-perf/` run root. Raw `perf.data` remains local because it is
large and embeds host paths. Commands are documented in
`tools/refwork-emu-perf/README.md`.

## Environment

- source revision before the profiling changes: `6cdeb3e5527070cc5c0e8b7bcfefe4893a594050`
- compiler: rustc 1.97.0 (`2d8144b78`, LLVM 22.1.6)
- kernel/perf: Linux 6.8.0-124-generic, perf 6.8.12
- CPU: Intel Core i5-8400 at 2.80 GHz; process pinned to one allowed logical CPU
- profile: native x86_64 GNU `--locked --release`, default emulator features
- synthetic ROM BLAKE3:
  `08715bee08aeee5d67a614507cc343a37996686a374de8fc060ff35108e6cd33`
- event: `instructions:u`, enabled only for the measured frame loop through
  acknowledged perf control FIFOs; all results unscaled (`100.00%` running)

## User Instructions

Counts below are raw integers. The small non-repeatability is retained rather
than averaged away.

| Case | Frames | Run 1 | Run 2 | Run 3 | Range |
|---|---:|---:|---:|---:|---:|
| boot (`warmup=0`) | 100 | 2,418,658,391 | 2,418,659,173 | 2,418,659,324 | 933 |
| boot | 200 | 4,854,284,033 | 4,854,284,322 | 4,854,283,039 | 1,283 |
| boot | 300 | 7,289,916,744 | 7,289,917,332 | 7,289,916,945 | 588 |
| steady (`warmup=600`) | 100 | 2,435,629,941 | 2,435,627,646 | 2,435,628,350 | 2,295 |
| steady | 200 | 4,871,255,711 | 4,871,256,204 | 4,871,255,660 | 544 |
| steady | 300 | 7,306,888,101 | 7,306,887,817 | 7,306,887,692 | 409 |

The 100→300 rational slopes are 24,356,288.11–24,356,300.86
instructions/frame across the six paired run indices. The first 100 boot frames
average about 24.187M/frame; later boot frames converge on the same ~24.356M
steady slope. Exact-window totals jitter by 26–2,295 instructions (roughly
0.00001–0.00009%); this violates the request's hoped-for exact repeatability and
is a finding. Counts were unscaled with no lost counter time, so the likely
source is the acknowledged userspace control path at the interval edges rather
than emulator nondeterminism. Frame-state proofs were identical per exact case.

## Wall Time

Wall runs are separate from perf-controlled runs so clock calls are not charged
to instruction counts. This shared host was noisy. For the 300-frame window:

| Case | Three totals | Median/frame | Observed range/frame |
|---|---|---:|---:|
| boot | 1.574 s, 2.495 s, 2.604 s | 8.32 ms | 5.25–8.68 ms |
| steady | 1.084 s, 1.415 s, 1.606 s | 4.72 ms | 3.61–5.35 ms |

These wall numbers characterize this host only. They are not comparable to the
90–115 ms KVM whole-guest result without matched hardware and execution paths.

## Sampling

The steady synthetic sample used 600 warmup frames and 300 measured frames,
`instructions:u`, DWARF call graphs, and the same acknowledged interval gate.
Perf captured 4,047 samples representing approximately 7,309,485,970
instructions, with zero lost samples.

| Exclusive top-level owner (inclusive call-tree share used for ownership) | Samples/share |
|---|---:|
| PPU `render_scanline` | 83.41% |
| CPU `step` | 8.24% |
| APU `advance_master_cycles` | 6.35% |
| frame scheduler/direct `run_one_frame` work | 1.99% |

These mutually exclusive call-tree children plus scheduler rounding cover
99.99% of the measured host lane. Useful visible sub-symbols include PPU
background rendering (19.37%), PPU pixel composition (17.23%), SPC700 execute
(1.05%), bus read (1.05%), bus write (0.10%), cartridge read (0.39%), DSP
register read (0.38%), sampled CPU addressing helpers (~0.5% visible), and an
ALU helper (0.03%). The latter are not additive to their CPU/APU parents.

`mem_speed` and much of CPU addressing/ALU dispatch are inlined, so this profile
does not honestly split every CPU instruction among those requested sub-buckets.
The whole CPU parent is only 8.24%; the unresolved *sub-bucket* split is recorded
as a methodology limitation, not assigned by intuition. Host-lane subsystem
coverage remains above 90% because the complete CPU parent is identified.
