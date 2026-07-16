## Action Items

### Critical
- None

### Important
- None

### Suggestions
- [ ] [crates/refwork-harness/src/frame.rs:142] Decide whether to drain multiple queued control datagrams at one frame boundary or document the current one-message-per-boundary behavior before the mock-agent fixture relies on shutdown timing.
- [ ] [crates/refwork-harness/src/frame.rs:462] Extend the hash-report unit test to assert the WRAM/framebuffer hash payloads, not just the reported frame number.
- [ ] [crates/refwork-harness/src/ctl.rs:95] Add a real `SeqpacketFd` test for `try_recv_msg` returning `None` on an empty socket and decoding a later datagram.
