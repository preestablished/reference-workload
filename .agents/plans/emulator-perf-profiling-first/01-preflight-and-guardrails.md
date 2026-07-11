# Step 01 — Freeze The Baseline And Measurement Contract

## 1. Confirm Repository State

1. Read all five request files and inspect `git status`. Preserve unrelated user
   changes; do not use them in a baseline build.
2. Record the baseline commit, `rustc -Vv`, `cargo -V`, target triple, CPU model,
   kernel, `perf --version`, governor/frequency policy, SMT state, ASLR state,
   and the exact commands in a machine-readable run manifest under a gitignored
   `target/refwork-emu-perf/<run-id>/` directory.
3. Confirm `perf stat -e instructions:u` is permitted. Run a short deterministic
   executable three times as a smoke test. If the event is unavailable or
   multiplexed, stop and document the host prerequisite; do not silently replace
   retired instructions with cycles or wall time. Reject scaled results: record
   enabled/running time and require a scaling factor of 1.
4. Inspect current bead state and existing first-room/M5 evidence for pointers,
   not private contents. Confirm whether a busy-scene scripted log exists. If it
   does not, adopt the request's two-workload fallback and plan a busy-scene
   follow-up bead rather than blocking this request.

## 2. Create An Immutable Comparison Baseline

Use a clean worktree at the recorded pre-request commit and separate target
directories for baseline and HEAD. Do not compare against a stale artifact in
the shared `target/` tree.

Build the shipped binary in both environments with the same toolchain and
environment:

```sh
cargo build -p refwork-harness --locked --release \
  --target x86_64-unknown-linux-musl
```

Record a cryptographic hash, byte size, ELF build ID if present, `Cargo.lock`,
and `cargo metadata --locked --format-version 1` for the baseline. Use the
actual configured musl target if the repository/toolchain specifies a different
one. Do not install a target or mutate shared toolchain state without noting it.

AC0 passes by either byte identity or a reviewed lockfile proof that no changed
package is in the `refwork-harness` dependency closure. Prefer and attempt byte
identity; source changes under a disabled `cfg` should normally preserve it, so
investigate a mismatch before relying on the allowed closure proof. Control
absolute paths/remapping, environment, toolchain, and `git_rev` inputs so build
metadata does not create a false mismatch.

## 3. Pin Workload Definitions Before Measuring

Define a case manifest with, at minimum:

- case name and visibility (`synthetic` or `operator-private`);
- ROM identity as a hash only;
- input-log identity as a hash only;
- boot frame range and steady-state warmup/measurement frame ranges;
- whether framebuffer blitting is included (default: include it, matching the
  production `run_one_frame` + `blit_completed_frame` shape);
- release target/profile, feature set, and executable hash;
- expected final frame count and a clean-room-safe final chain hash.

Recommended initial synthetic windows are a boot-frame case with zero warmup and
measured emulated frames `[0, 600)` and a steady case with 600 warmup frames followed by at
least 2,000 measured frames. Adjust only after timing the lane: each measured
sample should be long enough to avoid process-startup domination while still
supporting at least three independent repetitions. Apply the identical chosen
window at baseline and HEAD. ROM loading and `Core` construction are excluded
from boot-frame timing; report them only as separately labeled startup metrics.

For first-room, derive the steady window from the already verified scripted log
and document why it represents room play. Do not expose the input words or any
game-derived content in the public manifest.

## 4. Decide The Attribution Taxonomy Up Front

Use mutually exclusive top-level buckets so percentages sum correctly:

1. CPU interpreter: dispatch/loop, addressing, ALU/operation body;
2. APU catch-up: SPC700, DSP, timers/ARAM/glue;
3. PPU scanline rendering, including background/sprite/palette sub-buckets;
4. bus and timing, with `mem_speed`/per-access accounting isolated where source
   attribution permits;
5. DMA/HDMA and frame scheduler/blit;
6. benchmark setup/hash/output (outside the measured frame window if possible);
7. everything else/unresolved.

Write the symbol/source-range mapping used by the analysis into versioned config
or the methodology doc. Inclusive call-stack totals must not be added together;
use exclusive samples for the top-level sum and inclusive views only to explain
callers.

## Exit Criteria

- Baseline revision and shipped-binary hash are frozen.
- `perf` can produce non-multiplexed user-only instruction counts.
- Synthetic windows are fixed; private/busy-scene availability is classified.
- Attribution buckets and the raw-data schema are written before looking at
  results, preventing result-driven rebucketing.
