# Acceptance

Acceptance status: satisfied for the reviewed checkpoint.

- Start before `LoadGame`: covered by `crates/refwork-harness/tests/mock_agent.rs:54`, asserting `FaultCode::ProtocolOrder` at frame 0.
- Double `Start`: covered by `crates/refwork-harness/tests/mock_agent.rs:88`, asserting `FaultCode::ProtocolOrder` at frame 1.
- `HashRequest` before `Start`: covered by `crates/refwork-harness/tests/mock_agent.rs:69`, asserting `FaultCode::ProtocolOrder` at frame 0.
- Future `HashRequest`: covered by `crates/refwork-harness/src/frame.rs:506`, asserting `FaultCode::ProtocolOrder` at frame 1.
- Stale `HashRequest`: covered by `crates/refwork-harness/src/frame.rs:527`, asserting `FaultCode::ProtocolOrder` at frame 2.
- Malformed postcard datagram: covered by `crates/refwork-harness/tests/mock_agent.rs:104` for fd-3 setup and by `crates/refwork-harness/src/frame.rs:562` for steady state, both asserting `FaultCode::BadProto`.
- Oversize datagram: covered by `crates/refwork-harness/tests/mock_agent.rs:118` for fd-3 setup and by `crates/refwork-harness/src/frame.rs:579` for steady state, both asserting `FaultCode::BadProto`.
- Latch reread and double/missing `frame_mark`: covered by `crates/refwork-harness/src/frame.rs:636`, which asserts exactly one `poll_input`, one `frame_mark`, and one quiesce check per completed frame across three frames.
- `audit-syms --bin`: implemented at `xtask/src/main.rs:61` and `xtask/src/audit_syms.rs:57`; the release harness passed the audit command during review.
- Release build and deny gates: `cargo build --release --locked -p refwork-harness` and `cargo run --locked -p xtask -- deny` passed during review.
