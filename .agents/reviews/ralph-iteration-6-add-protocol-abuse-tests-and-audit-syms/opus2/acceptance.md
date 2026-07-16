# Acceptance

Acceptance appears satisfied for this checkpoint.

- Start before LoadGame: covered by `start_before_load_game_faults_protocol_order` at `crates/refwork-harness/tests/mock_agent.rs:54`, asserting `FaultCode::ProtocolOrder`.
- HashRequest before Start: covered by `hash_request_before_start_faults_protocol_order` at `crates/refwork-harness/tests/mock_agent.rs:69`, asserting `FaultCode::ProtocolOrder`.
- Double Start: covered by `double_start_faults_protocol_order_at_first_frame_boundary` at `crates/refwork-harness/tests/mock_agent.rs:88`, asserting `FaultCode::ProtocolOrder` at frame 1.
- Malformed postcard datagram: covered by `malformed_postcard_datagram_faults_bad_proto` at `crates/refwork-harness/tests/mock_agent.rs:104`, asserting `FaultCode::BadProto`.
- Oversize datagram: covered by `oversize_datagram_faults_bad_proto` at `crates/refwork-harness/tests/mock_agent.rs:118`, asserting `FaultCode::BadProto`.
- Future HashRequest: covered by `hash_request_for_future_frame_faults` at `crates/refwork-harness/src/frame.rs:501`, asserting `FaultCode::ProtocolOrder`.
- Stale HashRequest: covered by `hash_request_for_stale_frame_faults` at `crates/refwork-harness/src/frame.rs:527`, asserting `FaultCode::ProtocolOrder`.
- Latch reread and double/missing `frame_mark`: covered by `empty_boundaries_poll_latch_and_mark_once_per_frame` at `crates/refwork-harness/src/frame.rs:636`, asserting one poll and one mark per completed frame.
- `cargo build --release -p refwork-harness` passed.
- `cargo run --locked -p xtask -- audit-syms --bin target/release/refwork-harness` passed.
- `cargo run --locked -p xtask -- deny` passed.
