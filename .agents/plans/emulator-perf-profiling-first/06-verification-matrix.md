# Step 06 — Prove Zero Behavior Change And Close Acceptance

Run verification first on the synthetic workload while developing, then rerun
the complete matrix at the final commit. Preserve exact commands, revisions,
toolchain, hashes, exit status, and artifact/report locations in the resolution
record.

## A. Build And Dependency Guards (AC0)

1. Build baseline and final `refwork-harness` from clean worktrees with identical
   toolchain/environment:

   ```sh
   cargo build -p refwork-harness --locked --release \
     --target x86_64-unknown-linux-musl
   ```

2. Byte-compare and hash the two binaries. A passing result is exact identity.
3. Diff `Cargo.lock`, then use `cargo metadata --locked` to prove whether any
   changed package enters the harness closure. Record both even if binaries are
   identical.
4. Build `refwork-emu` with and without the profiling feature in separate target
   directories. Confirm the feature is non-default and absent from the harness's
   resolved features.

## B. Benchmark Correctness And Reproducibility (AC1)

- From a clean checkout, build the synthetic ROM and bench binary using only
  documented commands.
- Run boot and steady cases at every fixed window >=3 times.
- Assert identical executable/case/final proof identities and exact user-mode
  instruction totals for identical windows where observed; any jitter is a
  required finding and cannot be averaged into exactness. Validate rational
  short/long slopes and the third-window linearity check. Record wall-time
  spread/noise.
- Repeat the private first-room lane through documented private intake; public
  evidence contains aggregates and hashes only.
- Negative-test malformed args, emulator fault, mismatched case identity, and
  unavailable/multiplexed perf events.

## C. Attribution And Calibration (AC2)

- Recompute exclusive bucket totals from raw sampling output and confirm 100%
  sum plus >=90% resolved coverage separately for synthetic and first-room.
- Audit inline/source-range classification for `mem_speed` and ensure inclusive
  call-stack rows were not added together.
- Recompute guest residual and unexplained-gap percentages from raw totals.
- Confirm host and guest use comparable revision/profile/window/event semantics.
  An unexplained gap above approximately 15–20% is a failure, not a caveat.

## D. Before/After Behavior And Determinism (AC3)

Apply the frozen, source-hashed harness-only patch to clean baseline and final
worktrees, build one statically linked executable per emulator revision with
identical toolchain/flags/features, and compare rational authoritative
instructions/frame, frame counts, fault status, and the established frame-hash
chain for every public case.

At final HEAD run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run --locked -p xtask -- deny
cargo test --workspace --locked
cargo test --release --locked -p xtask --test zero_alloc
cargo test --release --locked -p xtask --test determinism -- --include-ignored
cargo run --locked --release -p xtask -- hash-chain --frames 10000
cargo run --locked -p xtask -- fetch-test-roms
cargo run --locked --release -p xtask -- cpu-tests
cargo run --locked --release -p xtask -- spc-tests
```

The fetch requires network and verifies `xtask/test-roms.lock`; do not download
or vendor unpinned ROM material. Run or cite the final-revision CI 10k
x86_64/aarch64 hash-chain comparison (nightly uses 100k). If neither local nor
CI evidence is available, record the skip and reason exactly as AC3 requires.

`--all-features` clippy/tests exercise instrumentation only as compile/lint
coverage. They are not authoritative behavior evidence; default-feature release
benchmarks and determinism commands provide that evidence.

The `deny` scan may need a narrowly reviewed update so host-only benchmark uses
of `Instant` are not mistaken for emulator/harness D2 violations. Do not weaken
the scan for `refwork-emu` or `refwork-harness`.

## E. Scope And Documentation Audit (AC4)

- `git diff` contains no optimization, emulator semantic/timing change, private
  data, or performance threshold assertion.
- Findings have all five required sections, raw provenance, ranked candidates,
  determinism blast radii, re-baseline ownership, and the `38b6` answer.
- Follow-up bead descriptions cite the stable findings revision/path and carry
  both blast-radius dimensions.
- `38b6` pointer and resolution evidence exist.
- Busy scene is measured or has its fallback bead.

## Final Stop Rule

Any shipped-binary mismatch without the AC0 lockfile/closure proof,
authoritative instruction-count delta, determinism gate failure, <90%
attribution, >20% unexplained host↔guest gap, or missing
external handoff keeps the request open. Record the failure; do not dilute the
criterion or optimize the emulator while trying to make profiling pass.
