# Positive Notes

- `crates/refwork-harness/src/frame.rs:127` keeps the frame-loop ordering easy to reason about: one pad poll, one emulator frame, one framebuffer blit, one meta update, one `frame_mark`, then boundary control handling.
- `crates/refwork-harness/src/frame.rs:157` cleanly distinguishes empty nonblocking polls, bad datagrams, decode errors, I/O errors, valid hash requests, shutdown, and unexpected steady-state messages.
- `crates/refwork-harness/src/frame.rs:172` enforces "HashRequest only for the last completed frame" in a simple equality check against the post-frame counter.
- `crates/refwork-harness/src/ctl.rs:89` centralizes datagram length/decode validation through `decode_datagram`, and the `len.min(buf.len())` slice avoids panic-prone assumptions about transport implementations.
- `crates/refwork-harness/src/regions.rs:386` moves `RegionBuffers` behind `Option` and consumes them with `take_buffers`, which prevents accidental reuse after the emulator core owns the static WRAM/VRAM/SRAM slices.
- `crates/refwork-harness/src/frame.rs:439` through `crates/refwork-harness/src/frame.rs:558` cover the main acceptance cases: shutdown at a frame boundary, one input read and frame mark per completed frame, last-frame hash reporting, future and stale hash rejection, unexpected message faults, and empty poll continuation.
