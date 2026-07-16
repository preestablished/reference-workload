# Tests

Commands run during review:

```text
cargo test --locked -p xtask audit_syms
```

Result: passed. The 3 `audit_syms` unit tests passed.

```text
cargo test --locked -p refwork-harness --test mock_agent
```

Result: passed. All 6 fd-3 mock-agent tests passed, including the long 1000-frame happy path.

```text
cargo build --release --locked -p refwork-harness
```

Result: passed.

```text
cargo run --locked -p xtask -- audit-syms --bin target/release/refwork-harness
```

Result: passed with `audit-syms: OK - no banned symbols in target/release/refwork-harness`.

```text
cargo run --locked -p xtask -- deny
```

Result: passed with `deny: OK`.

```text
cargo test --locked -p refwork-harness
```

Result: passed. The package run covered 41 unit tests, the 6 mock-agent integration tests, and doctests.

Not run during this review: full workspace tests or clippy. The acceptance-specific release build, symbol audit, deny gate, `xtask` audit tests, and harness tests were run.
