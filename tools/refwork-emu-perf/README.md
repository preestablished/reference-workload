# `refwork-emu` Performance Lane

This host-only lane measures the current emulator without optimizing it. The
release benchmark's timed loop contains only `run_one_frame` and
`blit_completed_frame`; ROM loading, `Core` construction, proof hashing, and JSON
output are outside the timed interval.

Build the synthetic ROM and smoke-test the benchmark:

```sh
cargo run --locked -p xtask -- build-rom --out target/synth-rom.rom
cargo run --locked --release -p refwork-emu-bench -- \
  --rom target/synth-rom.rom --case synth-boot \
  --warmup-frames 0 --measure-frames 600 --synthetic-input
```

Collect three exact-process repetitions at three frame lengths:

```sh
tools/refwork-emu-perf/run-authoritative.sh \
  --rom target/synth-rom.rom --case synth-steady --warmup 600 \
  --lengths 1000,2000,3000 --repetitions 3
```

The perf-controlled JSON has `elapsed_ns: null`: its counter interval is gated
to the frame loop, and wall-clock calls are deliberately excluded. Each adjacent
`*.wall.json` is an uncounted run of the same case and supplies wall time. The
authoritative marginal instructions/frame slope is calculated from raw integer
totals as `(I(Nl)-I(Ns))/(Nl-Ns)`. The third window checks linearity. Never
average instruction jitter into an exactness claim; retain and report it as a
finding. Summarize wall time with median and range/MAD.

Sampling uses `instructions:u` and perf's control FIFO so samples cover only the
measured frame interval:

```sh
tools/refwork-emu-perf/run-sampling.sh \
  target/synth-rom.rom synth-steady 600 3000 \
  target/refwork-emu-perf/synth-steady-sampling
```

Private first-room runs use the same commands with an operator-local ROM and
padlog path. Outputs contain only hashes, aggregate counts/times, frame counts,
and a final-state proof. Review raw stderr before copying it into a public
artifact; never commit private paths, ROM/input contents, memory, or pixels.

Raw run directories under `target/refwork-emu-perf/` are not committed. Promote
only reviewed aggregate tables and methodology to the findings document.
