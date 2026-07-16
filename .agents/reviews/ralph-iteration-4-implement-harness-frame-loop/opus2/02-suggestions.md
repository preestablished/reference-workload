# Suggestions

### `crates/refwork-harness/src/frame.rs:439`

Add steady-state tests for malformed and oversize control datagrams. The implementation handles `ControlError::Decode` and `ControlError::Oversize` as `BadProto` at lines 160-167, but the frame-loop test matrix currently covers only well-formed unexpected messages and wrong hash frames. This is worth pinning because setup has similar post-Ready coverage and this is a protocol boundary.

Suggested test shape:

```rust
#[test]
fn malformed_steady_state_datagram_faults_bad_proto() {
    let setup = setup_result();
    let mut malformed = wire(CtlMsg::Shutdown {});
    malformed.push(0xff);
    let mut channel = ControlChannel::new(ScriptTransport::new(vec![
        Inbound::Bytes(malformed),
    ]));
    let mut platform = TestPlatform::with_pads(&[0]);

    let err = run_frame_loop(&mut channel, setup, &mut platform).unwrap_err();

    assert!(matches!(err, FrameLoopError::BadProto { frame: 1, .. }));
    assert_fault(&channel.transport().sent, FaultCode::BadProto, 1);
}
```

### `crates/refwork-harness/src/frame.rs:298` and `crates/refwork-harness/src/runner.rs:333`

`bounded_fault_detail` is now duplicated between setup and frame-loop code. The two copies are currently identical, but fault truncation is protocol-visible behavior; extracting a shared helper would keep setup and steady-state faults from drifting.

Suggested helper:

```rust
pub(crate) fn bounded_fault_detail(detail: &str, max_bytes: usize) -> String {
    let mut end = detail.len().min(max_bytes);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end < detail.len() {
        format!("{}...", &detail[..end])
    } else {
        detail.into()
    }
}
```
