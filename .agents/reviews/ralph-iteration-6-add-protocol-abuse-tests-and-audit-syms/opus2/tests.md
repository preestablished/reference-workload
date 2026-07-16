# Tests

Commands run:

- `cargo test --locked -p xtask audit_syms -- --nocapture` passed.
- `cargo test --locked -p refwork-harness --test mock_agent -- --nocapture` passed: 6 tests, including the long 1000-frame happy path.
- `cargo test --locked -p refwork-harness empty_boundaries_poll_latch_and_mark_once_per_frame -- --nocapture` passed.
- `cargo build --release -p refwork-harness && cargo run --locked -p xtask -- audit-syms --bin target/release/refwork-harness` passed.
- `cargo run --locked -p xtask -- deny` passed.
- `cargo fmt --all -- --check` passed.
- `git diff --check main...HEAD` passed.

Additional manual probes:

- A small dynamic C binary importing `sleep` was reported by `audit-syms` as `FAILED` with symbol `sleep`, confirming the positive detection path.
- `nm -D target/release/refwork-harness` showed only allowed pthread runtime-support imports among the searched clock/sleep/thread/clone patterns.

One command attempt used unsupported multiple Cargo test-name filters and failed before running tests. I reran the mock-agent test target in the supported form above.
