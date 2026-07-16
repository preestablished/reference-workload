# Action Items

## Critical

- None.

## Important

- Update the post-READY receive path so malformed and oversized datagrams both send `Fault { code: BadProto }` and write `meta.status = faulted` with the matching fault code before exit.

- Change the production `run_fd3` path so a successful `run_setup` result is retained by the next runner phase. Do not drop `SetupResult` and exit success immediately after receiving `Start`.

- Reconcile optional SRAM handling. `GameLoaded.sram_size`, registered `sram` publication, and the `Cartridge` SRAM slice must all come from the same decision, or SRAM should be disabled/rejected until the frame-loop bead wires it correctly.

## Suggestions

- Add a real `socketpair(AF_UNIX, SOCK_SEQPACKET)` test for `SeqpacketFd`, including datagram boundaries and oversize behavior.

- Validate the fd domain as `AF_UNIX` and use `MSG_NOSIGNAL` for control sends.

- Extend setup tests to assert `GameLoaded` hash/mapper/SRAM fields and meta page cart hash/status/version contents.

- Reject trailing CLI arguments after `--fd3` or `--help`.
