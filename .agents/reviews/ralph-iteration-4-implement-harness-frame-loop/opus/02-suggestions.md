# Suggestions

## Drain Same-Boundary Control Messages Deliberately

- File: `crates/refwork-harness/src/frame.rs:142`
- What to change and why: The current loop processes one control datagram per completed-frame boundary. That matches the package-02 wording of a single nonblocking control poll, so this is not blocking. The next mock-agent fixture, however, will send per-frame `HashRequest`s and then a clean `Shutdown`; if both are queued for the same boundary, the current loop handles the hash and then runs one more frame before observing shutdown. Consider either documenting that behavior or draining boundary messages until the socket is empty or shutdown/fault occurs.

Suggested shape if same-boundary drain is desired:

```rust
loop {
    match self.recv_boundary_msg(channel, frame)? {
        Some(BoundaryAction::Continue) => continue,
        Some(BoundaryAction::Shutdown) => return Ok(FrameLoopExit::Shutdown { frame }),
        None => break,
    }
}
```

## Assert Hash Payloads Against Direct Core State In Tests

- File: `crates/refwork-harness/src/frame.rs:462`
- What to change and why: `hash_request_reports_only_last_completed_frame` verifies that a `HashReport` is emitted for frame 1, but it does not assert that `wram` and `fb` are the hashes of the completed frame. Package 03 will cover end-to-end hash parity, but a local unit assertion would protect this new `send_hash_report` behavior from regressions.

Suggested test helper/assertion:

```rust
let CtlMsg::HashReport { frame, wram, fb } = &channel.transport().sent[0] else {
    panic!("expected HashReport, got {:?}", channel.transport().sent[0]);
};
assert_eq!(*frame, 1);
assert_ne!(*wram, [0u8; 32]);
assert_eq!(*fb, blake3::hash(frame_loop.active.framebuffer().unwrap()).into());
```

If direct access to `active` remains private in tests, compare the report against an independently run `Core` over the same ROM and pad for one frame.

## Add A Nonblocking SEQPACKET Transport Test

- File: `crates/refwork-harness/src/ctl.rs:95`
- What to change and why: The new `try_recv_msg` path is covered by scripted transports through the frame-loop tests, but not by the real `SeqpacketFd` implementation. A small socketpair test would pin the `WouldBlock -> Ok(None)` behavior and the later successful decode on the production transport.

Suggested test:

```rust
#[test]
fn seqpacket_try_recv_reports_empty_then_message() {
    let (transport, mut peer) = seqpacket_pair();
    let mut channel = ControlChannel::new(transport);

    assert!(channel.try_recv_msg().unwrap().is_none());

    let msg = refwork_protocol::encode(&CtlMsg::Shutdown {}).unwrap();
    send_raw(&mut peer, &msg);

    assert_eq!(channel.try_recv_msg().unwrap(), Some(CtlMsg::Shutdown {}));
}
```
