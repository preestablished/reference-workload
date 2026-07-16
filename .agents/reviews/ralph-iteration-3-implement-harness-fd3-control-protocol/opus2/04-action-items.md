# Action Items

## Critical

- None.

## Important

- Add `MSG_NOSIGNAL` or an equivalent closed-peer strategy in `SeqpacketFd::send_datagram`, then cover it with a real `AF_UNIX/SOCK_SEQPACKET` closed-peer test.

- Ensure every post-`Ready` bad-protocol path marks the meta page faulted before returning, including malformed postcard data and oversize datagrams while waiting for `Start`.

- Bound or sanitize `Fault.detail` before encoding so long peer-controlled values cannot turn the intended deterministic fault into `EncodeError::Oversize`.

## Suggestions

- Add socketpair-based tests for exact-size datagrams, oversized datagrams, malformed datagrams, and closed-peer send behavior.

- Extend fd validation to check `SO_DOMAIN == AF_UNIX` in addition to `SO_TYPE == SOCK_SEQPACKET`.

- Define a single SRAM source of truth so `GameLoaded.sram_size` cannot disagree with a configured/registered `sram` region.

- When the frame-loop bead lands, consume `SetupResult` into the loop entrypoint so the production binary cannot drop published regions immediately after `Start`.

- Add happy-path assertions on the returned `SetupResult`, including meta contents and activation readiness.
