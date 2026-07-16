# Positive Notes

- `crates/refwork-harness/src/frame.rs:127` keeps the production loop single-threaded and deterministic, with the exact ordered sequence from the plan: input poll, frame execution, framebuffer blit, meta update, frame mark, quiesce check, then control poll.
- `crates/refwork-harness/src/frame.rs:128` masks the platform pad value to `0x0fff` before passing it into the core and before publishing it through meta, preserving the documented pad bit contract.
- `crates/refwork-harness/src/frame.rs:157` handles oversize and decode failures separately from I/O errors, translating protocol-boundary corruption into deterministic `BadProto` faults while leaving real transport failures as control I/O errors.
- `crates/refwork-harness/src/frame.rs:172` enforces that `HashRequest` is served only for the last completed frame and faults both future and stale frame requests with `ProtocolOrder`.
- `crates/refwork-harness/src/frame.rs:197` hashes WRAM and framebuffer separately in the `HashReport`, matching the protocol shape and avoiding any extra per-frame control traffic for pad state.
- `crates/refwork-harness/src/frame.rs:298` truncates fault detail at a byte limit while respecting UTF-8 character boundaries.
- `crates/refwork-harness/src/regions.rs:391` makes ownership transfer into `Core::new` explicit with `take_buffers`, while keeping the active region owner alive for framebuffer and meta access.
- `crates/refwork-harness/src/ctl.rs:95` adds a nonblocking control receive abstraction without changing the blocking setup state machine.
- `crates/refwork-harness/src/frame.rs:439` through `crates/refwork-harness/src/frame.rs:558` cover the high-risk steady-state behaviors requested by the bead, including shutdown, future and stale hash requests, unexpected messages, empty polls, and meta fault state.
