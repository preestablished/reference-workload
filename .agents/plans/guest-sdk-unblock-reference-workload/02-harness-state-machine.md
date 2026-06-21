# 02 - Harness State Machine And Frame Loop

**Upstream package:** RW-1, implementation half.

**Purpose:** turn `crates/refwork-harness` from a handshake helper into the
actual single-threaded workload binary: fd-3 SEQPACKET control, deterministic
startup, published regions, `meta` page, and the free-running frame loop.

## Current State

- `crates/refwork-harness/src/lib.rs` only exposes `hello_ack()`.
- The crate has no binary target.
- `refwork-protocol` already defines the `CtlMsg` wire surface and should be
  reused, not reshaped.
- There is no guest-sdk crate dependency in this workspace yet; M3 must remain
  host-testable without platform dependencies.

## Deliverables

1. Crate shape:
   - Add `src/main.rs` for the production `refwork-harness` binary.
   - Split the library into small modules such as `ctl`, `runner`, `regions`,
     `meta`, `platform`, and `game`.
   - Keep most modules `#![forbid(unsafe_code)]`, but define the production
     region allocator explicitly as a small unsafe boundary if mmap-backed
     static slices cannot be expressed safely. The preferred shape is an owned
     `MmapRegion` type in `regions` that stores pointer, length, page
     alignment, and mapping flags; unmaps on `Drop`; and exposes unique mutable
     slices only while the owner is alive and not aliased.
   - If the crate-level `#![forbid(unsafe_code)]` must be relaxed, replace it
     with `#![deny(unsafe_op_in_unsafe_fn)]`, keep unsafe code confined to the
     region owner module, and document the invariants in that module: one owner
     per mapping, page-aligned length, no reallocation after registration, no
     aliasing mutable slices, and drop only after the emulator stops.
2. Control transport:
   - Use inherited fd 3 as `socketpair(AF_UNIX, SOCK_SEQPACKET)`.
   - One datagram equals one postcard-encoded `CtlMsg`.
   - Enforce `MAX_DATAGRAM` for received non-`RegisterRegion` messages as well
     as encoded messages.
   - Strict request/response ordering: `Hello`, `LoadGame`, harness emits
     `GameLoaded`, `RegisterRegion` messages, `Ready`, agent sends `Start`,
     then steady state.
   - No per-frame pad traffic over the control socket.
3. Runner state machine:
   - Reject out-of-order messages with `Fault { code: ProtocolOrder }`.
   - Reject malformed or oversize messages with `Fault { code: BadProto }`.
   - On protocol version mismatch, send `Fault { code: BadProto }` and exit
     non-zero.
   - On bad ROM/cart construction, send `Fault { code: BadGame }`.
   - On region allocation or registration preparation failure, send
     `Fault { code: RegionRegFailed }`.
4. Game loading:
   - Production path loads the block-device path supplied by
     `LoadGame { dev_path }` read-only. The owner docs prefer an mmap of
     `/dev/vdb`; host tests may use a regular temp file with the synthetic ROM.
   - Compute the cart BLAKE3 and emit `GameLoaded { cart_hash, mapper,
     sram_size }`.
5. Region buffers:
   - Allocate page-aligned mappings whose lengths are exact page multiples and
     match the API/manifest sizes before `Core::new`: `wram` = 131072 bytes,
     `framebuffer` = 229376 bytes, `meta` = 4096 bytes; optional `vram` and
     `sram` use their manifest or harness-configured sizes.
   - Bridge the current `RegionBuffers` API, which takes `&'static mut` slices,
     through the owned mapping type above. The code must make the lifetime
     extension explicit and justified by process lifetime/owner invariants, not
     hidden behind helper casts.
   - Always publish `wram`, `framebuffer`, and `meta`.
   - Support optional `vram` and `sram` behind a config flag or harness config,
     but do not make them required for READY.
   - Use `MAP_LOCKED | MAP_POPULATE` on the production path when available.
   - Never reallocate published buffers after `RegisterRegion`.
6. `meta` region:
   - Implement a typed writer for API.md section 3.6:

     | Offset | Field |
     |---|---|
     | `0x00` | `meta_version = 1` |
     | `0x04` | status: init, ready, running, faulted |
     | `0x08` | last completed frame |
     | `0x10` | last pad |
     | `0x14` | fault code |
     | `0x18` | cart BLAKE3 |
     | `0x38` | NUL-padded emulator version string |

   - Add byte-offset tests. These tests are the guardrail for guest-sdk,
     hypervisor, and observatory consumers.
7. Platform abstraction:
   - Add a small internal trait for the guest-sdk calls the harness needs:
     `poll_input(port)`, `frame_mark()`, and `quiesce_check()`.
   - M3 uses a deterministic host/test implementation.
   - The real guest implementation can be added under a feature or separate
     module when guest-sdk exposes the crate/API. Do not invent a guest-sdk
     contract in this repo.
8. Frame loop:
   - Each iteration performs exactly:
     `poll_input(0) -> run_one_frame(pad) -> blit_completed_frame -> meta update -> frame_mark -> quiesce_check -> nonblocking control poll`.
   - `poll_input` is called exactly once per frame.
   - `frame_mark` is called exactly once per completed frame.
   - `HashRequest { frame }` is serviced only at the frame-boundary control
     poll and only if `frame` equals the last completed frame.
   - `Shutdown` exits 0 from a frame boundary.
   - Any unexpected steady-state control message faults deterministically.
   - `meta.status` is `ready` before sending harness `Ready { frame: 0 }` and
     remains `ready` after receiving `Start` until the first completed frame
     updates `meta` to `running`. This keeps the guest-sdk READY beacon/root
     snapshot observable as `status=ready, frame=0` while still making
     subsequent frame-boundary state `running`.

## Tests

Add package-local unit tests for:

- `meta` layout bytes and status transitions.
- State-machine ordering without running the emulator.
- Protocol version mismatch.
- Oversize receive path.
- `HashRequest` for the last completed frame versus a future/stale frame.
- One pad read and one frame mark per successful frame using the test platform.
- `Fault` status written to `meta` before exit.

The end-to-end mock-agent tests live in package 03.

## Acceptance

- `cargo run --locked -p refwork-harness -- --help` or equivalent exits
  deterministically and documents fd-3 production mode plus test mode, if a
  test mode is added.
- `cargo test --locked -p refwork-harness` passes.
- `cargo run --locked -p xtask -- deny` still scans `refwork-harness` and
  passes.
- The harness binary can be built in release mode without adding thread,
  clock, RNG, async, or float tokens to deny-scoped crates.
- Any unsafe code is confined to the documented region owner module and is
  covered by tests for alignment, length, drop, and no post-registration
  reallocation.
- The code path is ready for package 03 to drive through a mock agent.

## Notes For The Implementing Agent

- Do not use `std::os::unix::net::UnixDatagram` as a quiet substitute for
  SEQPACKET. The transport contract is SEQPACKET because message boundaries
  and ordering are part of the protocol.
- Do not put pad words on the control socket for convenience. The host test
  platform may supply scripted pad words, but the protocol must remain control
  only.
- Keep host-test helpers out of the production frame loop. The production loop
  must remain free-running and single-threaded.
