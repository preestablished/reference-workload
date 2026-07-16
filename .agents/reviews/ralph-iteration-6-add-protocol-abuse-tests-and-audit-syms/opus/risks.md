# Risks

- `xtask/src/audit_syms.rs:62` shells out to `nm -a` and parses the last whitespace-delimited field. This passed on the current Linux/GNU toolchain, including the real release binary, but the command is intentionally tied to `nm` output shape. If the build later targets a different object format or a stripped/static binary with different symbol naming, the banned list/parser may need expansion.
- `xtask/src/audit_syms.rs:7` bans exact public entry points. That avoids false positives from Rust/libc runtime support, but it also means future underscored libc internals or alternate libc names would not be caught unless they are added deliberately.
- `crates/refwork-harness/tests/mock_agent.rs:19` keeps the real fd-3 happy path as a 1000-frame integration test. It passed twice during this review, but it remains a relatively long test target, around 50 to 61 seconds on this machine, and depends on local scheduler/socket behavior.
- `crates/refwork-harness/tests/mock_agent.rs:416` uses 10-second socket timeouts. That bounds hangs and is appropriate for the fd-3 tests, but failures under heavy CI load may still take noticeable time before surfacing.
