# Step 02 — Build The Dependency-Free Benchmark Lane

## Placement And Dependency Shape

Add a small binary crate such as `crates/refwork-emu-bench` to the workspace.
Use path dependencies on `refwork-emu`, `refwork-hash` for the established frame
hash/chain contract, and `refwork-script` for padlog parsing. Do not hand-copy the
hash algorithm or parser. These packages and blake3 are already locked; use
`cargo metadata` to audit closure effects. Reuse or move the synthetic-ROM
builder only if doing so does not pull `xtask`'s serde dependency tree into the
benchmark. A practical default is:

- keep `cargo xtask build-rom --out target/synth-rom.rom` as an explicit setup
  command;
- make the benchmark accept `--rom`, `--case`, `--warmup-frames`,
  `--measure-frames`, and an optional private `--script` path;
- implement the deterministic synthetic pad schedule locally in the benchmark,
  matching `xtask::hash_chain::pad`, with a test that prevents drift; use
  `refwork-hash` for the actual proof chain.

Do not add timing or filesystem access to `refwork-emu`; those belong in the
host-only benchmark executable.

## CLI And Output Contract

The executable must support one workload per process and emit one concise JSON
record after completing the run. The record should contain case/window IDs,
executable/ROM/script hashes where available,
warmup and measured frame counts, elapsed nanoseconds for the measured window,
final frame counter, fault status, and a deterministic checksum/chain sufficient
to prove the workload was not optimized away or accidentally changed.

Keep stdout machine-readable and send diagnostics to stderr. Never emit ROM
bytes, input words, framebuffer pixels, WRAM, private filenames, or exact private
paths. A private script should be parsed through `refwork-script` only if adding
that path dependency is proven outside the shipped harness closure; otherwise
use a small private-lab adapter executable and keep the authoritative core loop
identical.

Time only the requested frame loop:

1. load ROM and construct `Core` before timing;
2. execute warmup frames and validate no fault;
3. start `Instant` in the benchmark crate;
4. for each measured frame, run `Core::run_one_frame(pad)` and
   `blit_completed_frame` into a preallocated framebuffer;
5. stop timing, validate frame count/fault, and compute the final proof outside
   the timed window where practical.

Wall-clock timing is deliberately in this host-only tool and is not evidence of
a D2 violation. The benchmark must perform no allocation inside the measured
frame loop beyond behavior already exercised by the existing zero-allocation
gate.

## Instruction-Count Driver

Add a shell script or documented command under a host tooling directory that
wraps the built executable with:

```sh
perf stat --no-big-num -x, -r 1 -e instructions:u -- \
  <bench-binary> <fixed case arguments>
```

Run independent processes at least three times rather than relying only on
`perf stat -r`, and retain each raw stderr/stdout pair under the run directory.
Reject samples if perf reports multiplexing, unsupported events, faults,
unexpected frame/hash output, or a different executable/case identity.

Whole-process `perf stat` includes startup and warmup while the JSON wall time
covers only the measurement window. Therefore report both:

- raw process instructions and raw total executed frames; and
- a measured-window estimate obtained from paired runs with identical setup and
  warmup but two measurement lengths, using the instruction delta divided by
  the frame-count delta.

Use at least two sufficiently separated lengths `Ns` and `Nl`, each with the
same warmup/input prefix, and repeat each exact length >=3 times. First require
or report whether raw totals for each length repeat exactly. Only then compute
the rational slope `(I(Nl)-I(Ns))/(Nl-Ns)`; do not arbitrarily pair individual
replicates or require integer divisibility. A third length should confirm
linearity and expose phase-boundary bias. If raw totals jitter, preserve that as
a finding and do not average it into a claim of exactness. This subtraction avoids
pretending loader/setup instructions belong to a frame without adding in-process
perf APIs, unsafe code, or dependencies.

## Optional Profiling Feature

If sampling cannot distinguish required sub-buckets, add a `profiling` feature
to `refwork-emu` with fixed-size integer counters only. It may count emulator
events or scoped entries (CPU opcodes, addressing calls, bus accesses, SPC/DSP
steps, rendered scanlines), but it does **not** directly measure retired host
instructions. Label it structural corroboration and use it to distribute or
validate sampled costs, never as the authoritative instr/frame result.

Prove with tests or symbol/disassembly comparison that the feature-disabled
crate contains none of the counter storage or update sites. Keep `unsafe_code`
forbidden.

## Tests And Documentation

- CLI parsing rejects zero/overflowing frames, unknown cases, missing inputs,
  malformed scripts, and private-output hazards.
- Synthetic schedule and final proof match the existing hash-chain/determinism
  lane for an agreed short window.
- A fault returns nonzero and cannot produce a successful sample record.
- Document clean-checkout build-ROM, build-bench, run, and raw-result locations.
- Do not put benchmark results in CI assertions. CI may compile and smoke-test
  the lane; performance thresholds are machine-specific evidence, not gates.

## Exit Criteria

The clean-checkout synthetic boot and steady cases run from documented commands,
produce parseable and privacy-safe results, and yield three valid instruction
samples plus three wall-time samples per selected window.

The executable and synthetic acceptance remain host-only and operator-free. The
same executable may accept operator-local ROM/script paths later; private and KVM
runs are evidence steps, not prerequisites to build or review the lane. Without
them the overall request remains incomplete even if a staged host-only harness
lands.
