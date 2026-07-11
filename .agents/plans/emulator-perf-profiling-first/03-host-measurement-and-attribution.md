# Step 03 — Measure And Attribute The Host Lane

## Authoritative Runs

Build once per revision/case using `--locked --release`; run that exact binary
for every repetition. Pin the process to one logical CPU with `taskset` where
available, keep the machine otherwise idle, and record rather than silently
change governor/turbo/SMT settings. Randomize or interleave short/long run order
to reduce thermal/time drift.

For each boot and steady case:

1. collect >=3 short-window and >=3 long-window `instructions:u` process totals;
2. derive the paired-delta instructions/frame and preserve every raw total;
3. require identical counts within each exact case/window. If counts jitter,
   inspect runtime paths, event enabled/running time, scaling, and CPU event
   errata, and collect enough additional runs to characterize it. Do not average
   jitter away or claim exact reproducibility;
4. collect >=3 benchmark-emitted elapsed times and report median, min/max, and a
   robust spread such as MAD or relative range;
5. state the observed noise bound rather than promising a universal threshold.

Because the baseline revision predates the benchmark crate, freeze the completed
benchmark source and source hash, then apply that harness-only source as a
temporary patch in clean baseline and HEAD worktrees. Build it against each
revision's `refwork-emu` with identical toolchain, flags, target, and features.
Record both executable hashes and emulator revisions. The rational
measured-window instruction slope and established frame-hash proof must match;
do not imply that one statically linked executable swaps emulator revisions.

## Sampling Pass

Use a symbolized release build with the same optimization level and target.
Prefer release debuginfo without changing code generation; verify executable
`.text` identity against the authoritative build by extracting and hashing that
section rather than comparing whole ELF files. If `.text` differs, label the
sampling binary separately and quantify its instr/frame delta before using it.

Scope `perf record` to exactly the post-warmup measured frame interval using
`perf` control enable/disable around explicit benchmark synchronization, or an
equivalent proven gate. Validate the gated sample window's frames and event total
against `perf stat` for that same window. If gating is unavailable, use a long
steady window, quantify contamination, and do not claim boot attribution from
it.

Use `instructions:u` sampling for the instruction-attribution table, with DWARF
call graphs where needed. Cycle sampling may be a separately labeled latency
profile and must never be multiplied by authoritative instruction totals.
Preserve the command, event period/frequency, enabled/running time, lost samples,
skid/callgraph mode, `perf.data`, `perf report --stdio`, binary, and source
revision. Repeat until bucket shares and their uncertainty are stable.

Build the attribution table from exclusive samples. Use call graphs and source
annotation to split CPU dispatch/addressing/ALU and to recover inlined
`mem_speed` costs. Do not double-count an inlined timing calculation once under
its caller and again as bus timing. Document ambiguous source ranges and put
them in unresolved rather than assigning them by intuition.

If `perf` symbolization cannot resolve an important inline bucket, use one or
more of these cross-checks without changing the authoritative build:

- `perf annotate` against release debuginfo and source-line ranges;
- the optional structural counters from step 02;
- carefully scoped sampling variants, each reported as non-authoritative;
- static disassembly inspection to identify the inlined range.

Do not introduce `inline(never)` or refactor functions merely to create cleaner
profiles; that would profile a different emulator.

## Convert Samples Into Instruction Estimates

For every top-level bucket, report sample share, estimated instructions/frame
(`authoritative instr/frame * share`), confidence/noise statement, and the
symbol/source mapping. Sampling percentages are estimates; keep more precision
in raw data than in prose, and avoid false single-instruction accuracy.

Report two coverage views. The normative AC2 host-lane coverage is:

```text
1 - (unresolved samples / all user-mode samples in the exact measured window)
```

The top-level table must reconcile to the entire authoritative marginal host
instructions/frame, including benchmark/runtime/DSO work in the measured
window; attribute it or leave it unresolved. Also report emulator-frame-loop
resolution as a diagnostic. Give a sampling confidence interval and support the
>=90% claim conservatively rather than by a rounded point estimate.

Reach >=90% for the host lane on each representative workload, not merely after
pooling an easy synthetic case with a poorly resolved private case. Benchmark
startup/output should be outside the gated interval; any that remains is a named
host bucket. Kernel execution is excluded by `instructions:u`, not subtracted.

## Attribution Review Checks

- Top-level exclusive shares sum to 100% after rounding correction.
- CPU, APU/SPC700/DSP, PPU, bus/`mem_speed`, scheduler/blit, and unresolved all
  appear even if a measured share is small.
- Boot and steady state have separate rows or tables.
- Synthetic and first-room results remain separate; no synthetic profile is
  presented as a proxy for the real workload.
- Every table cell traces to a raw artifact and command.

## Exit Criteria

Synthetic host results are repeatable, baseline-versus-HEAD authoritative
instruction counts are identical, and >=90% of synthetic steady-state host
instructions are attributed with a reproducible mapping. Private-case coverage
is completed in step 04.
