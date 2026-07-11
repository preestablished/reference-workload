# Profiling-Lane Verification — 2026-07-11

## Green Local Evidence

- `cargo test -p refwork-emu-bench --locked`: 5 passed.
- `cargo clippy -p refwork-emu-bench --all-targets -- -D warnings`: passed.
- `cargo run --locked -p xtask -- deny`: passed; emulator/harness D1–D4 scope
  unchanged.
- `cargo test --workspace --locked`: the long workspace run completed its
  existing suites without an observed failure, including 181 emulator tests,
  the 1,000-frame mock-agent test, 600-frame hash equality, and the 10k
  double-run.
- `cargo test --release --locked -p xtask --test zero_alloc`: 1 passed.
- `cargo test --release --locked -p xtask --test determinism --
  --include-ignored`: 600-frame and 10,000-frame double-runs passed.
- pinned corpus fetch verified `xtask/test-roms.lock` BLAKE3 values.
- `cargo run --locked --release -p xtask -- cpu-tests`: 5,120,000 passed,
  zero failed.
- `cargo run --locked --release -p xtask -- spc-tests`: 256,000 passed,
  zero failed.
- authoritative perf runs were unscaled; sampling captured 4,047 samples with
  zero lost samples.
- `bash -n tools/refwork-emu-perf/*.sh`: passed.

## Dependency And Shipped-Binary Guard

`Cargo.lock` adds only the new workspace package `refwork-emu-bench` and points
it at already locked packages. `cargo tree --locked -p refwork-harness` contains
no `refwork-emu-bench`; no source, dependency, or feature of `refwork-harness`
or its `refwork-emu` build changed. This is the AC0 closure-proof alternative.

A stale pre-change musl artifact did not byte-match a newly rebuilt artifact,
but the compiler/toolchain/build inputs of the stale artifact were not recorded,
so it is not valid byte-identity evidence in either direction. The closure proof
is the authoritative AC0 evidence for this pass.

## Pre-existing/Environmental Limits

- Workspace-wide strict clippy reaches a pre-existing Rust 1.97 lint in
  `crates/ramdiff/src/filter.rs:413` (`useless_borrows_in_formatting`). The new
  crate passes strict clippy; this profiling request does not alter unrelated
  ramdiff code.
- Workspace-wide `cargo fmt --all -- --check` traverses sibling path workspaces
  and reports pre-existing formatting changes in control-plane and
  determinism-hypervisor. Package-scoped formatting for the new crate passes.
- No local aarch64 execution was available. A final-revision cross-architecture
  CI result requires a pushed commit.
- The prior first-room report proves the private run existed, but the operator
  ROM and padlog are absent now. No matched KVM calibration can be run from the
  retained hashes/report alone.
