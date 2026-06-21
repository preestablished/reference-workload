# 03 - Mock-Agent Fixture And Protocol Abuse Tests

**Upstream package:** RW-1, validation half.

**Purpose:** prove the harness-agent protocol without guest-sdk or hypervisor
dependencies. This package closes `guest-sdk-ext-refwork-m3-mock-agent` by
publishing a fixture guest-sdk can reuse for unit.control tests.

## Dependencies

- Package 02 harness runner and test platform.
- Existing `refwork-protocol` encode/decode helpers.
- Existing synthetic ROM builder in `xtask`.

## Deliverables

1. Mock-agent test harness:
   - Add a reusable fixture module, preferably under
     `crates/refwork-verify/tests/` or `crates/refwork-harness/tests/`.
   - Use `socketpair(AF_UNIX, SOCK_SEQPACKET)` so the fixture exercises the
     real datagram transport.
   - Drive the harness library in-process with a scripted platform
     implementation. A test thread is acceptable in the host test fixture; the
     production harness loop must still be single-threaded.
   - Feed the synthetic ROM through the `LoadGame { dev_path }` path using a
     temp file.
2. Scripted platform:
   - Supplies a deterministic sequence of 1,000 pad words.
   - Records every `poll_input` and `frame_mark`.
   - Fails the test if the harness reads the latch twice in one frame, skips a
     frame mark, or double-marks a frame.
   - Exposes frame-boundary notifications so the mock agent can send
     `HashRequest { frame }` for the last completed frame.
3. Happy-path integration test:
   - `Hello -> HelloAck`.
   - `LoadGame -> GameLoaded`.
   - Receive `RegisterRegion` for at least `wram`, `framebuffer`, and `meta`.
   - Receive `Ready { frame: 0 }`.
   - Send `Start`.
   - Run 1,000 free-running frames.
   - Request and receive `HashReport` at each frame boundary.
   - Compare hash reports with a direct `refwork-emu` run over the same pad
     sequence.
   - Send `Shutdown` and require clean exit.
4. Abuse tests:
   - `Start` before `LoadGame` faults with `ProtocolOrder`.
   - Double `Start` faults with `ProtocolOrder`.
   - `HashRequest` before `Start` faults with `ProtocolOrder`.
   - `HashRequest` for a frame other than the last completed frame faults with
     `ProtocolOrder`.
   - Malformed postcard datagram faults with `BadProto`.
   - Oversize non-`RegisterRegion` datagram faults with `BadProto`.
   - `poll_input` latch re-read in one frame is detected by the test platform
     and reported as a deterministic harness/test failure.
   - `frame_mark` without a completed frame or double `frame_mark` is covered
     either by platform invariants or direct runner tests.
5. Published fixture path:
   - Document the fixture command and expected files in this plan folder or in
     repo docs.
   - Guest-sdk should be able to cite a stable path and command when closing
     its unit.control dependency.
6. Release binary symbol audit:
   - Add the first usable `cargo run --locked -p xtask -- audit-syms --bin <path>`
     implementation here, because RW-1 acceptance includes the release harness
     artifact audit.
   - Package 07 owns CI wiring and evidence publication for this command; this
     package owns making the command available before claiming M3 acceptance.

## Suggested Commands

```sh
cargo test --locked -p refwork-harness
cargo test --locked -p refwork-verify --test mock_agent
cargo run --locked -p xtask -- deny
cargo build --locked --release -p refwork-harness
cargo run --locked -p xtask -- audit-syms --bin target/release/refwork-harness
```

If the mock-agent tests live in a different crate, keep the command names in
the package closeout note.

## Acceptance

- Full mock-agent handshake, 1,000 frames, per-frame hash requests, and clean
  shutdown pass.
- Abuse tests cover all cases listed above.
- Hashes from `HashReport` match a direct `refwork-emu` run for the same pad
  sequence.
- The fixture uses the real `CtlMsg` postcard encoding and real SEQPACKET
  datagrams.
- A release `refwork-harness` binary passes `xtask audit-syms`; package 07
  later wires the same command into CI.

## Stop Conditions

- If the fixture needs behavior that is not in API.md section 3, stop and file
  a doc/API reconciliation note. Do not extend `CtlMsg` only for tests.
- If the test can only pass by adding per-frame socket traffic, the harness
  design is wrong. Pad input must remain in the platform abstraction.
