# Positive Notes

- `crates/refwork-harness/src/ctl.rs:78` uses a `MAX_DATAGRAM + 1` receive buffer and rejects `len > MAX_DATAGRAM` before decode. That is a clean way to detect oversized setup messages without allocating.

- `crates/refwork-harness/src/runner.rs:91` keeps the setup ordering explicit: Hello, LoadGame, load/prepare, GameLoaded, RegisterRegion, Ready, then Start. The top-level flow is easy to audit against the protocol.

- `crates/refwork-harness/src/runner.rs:148` centralizes malformed and oversized receive handling so the BadProto behavior is not duplicated across the initial handshake states.

- `crates/refwork-harness/src/meta.rs:58` and `crates/refwork-harness/src/meta.rs:110` publish status after payload fields, with a release compiler fence immediately before the status write. That matches the status-last publication pattern needed by external readers.

- `crates/refwork-harness/src/runner.rs:211` initializes the meta page through the typed `MetaPage` writer, then writes cart hash, emulator version, and ready status before `Ready` is sent.

- `crates/refwork-harness/src/runner.rs:399` through `crates/refwork-harness/src/runner.rs:554` cover the major setup state-machine branches: happy ordering, protocol version mismatch, malformed and oversized input, early out-of-order messages, bad game load, region preparation failure, and out-of-order Start.
