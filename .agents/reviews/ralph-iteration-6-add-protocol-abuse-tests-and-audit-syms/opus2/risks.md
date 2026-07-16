# Risks

- The banned-symbol scope is intentionally exact at `xtask/src/audit_syms.rs:7`. That avoids false positives from Rust/glibc runtime support, but future policy changes for broader scheduler or timer APIs will need explicit additions to this list.
- `run_audit_syms` uses `nm -a` at `xtask/src/audit_syms.rs:62`. This works for the current Cargo release binary and fails closed if `nm` cannot read symbols, but a future stripped-release profile would turn the audit into an `nm` failure rather than a precise banned-symbol report.
- The mock-agent integration test target still has one long-running happy path. In this run, `mock_agent_happy_path_1000_frames` completed successfully in about 69 seconds, but it remains the main wall-clock cost in the target.
- The integration lifecycle helpers poll child exit with a bounded loop at `crates/refwork-harness/tests/mock_agent.rs:495` and `crates/refwork-harness/tests/mock_agent.rs:520`. I did not see a hang or orphaned child locally; the drop guard at `crates/refwork-harness/tests/mock_agent.rs:480` is the right fallback if an assertion exits early.
