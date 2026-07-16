# Positive Notes

- `crates/refwork-harness/src/ctl.rs:79-85`: The receive path intentionally allocates `MAX_DATAGRAM + 1` bytes and classifies `len > MAX_DATAGRAM` before decoding, which is a simple way to detect oversize records without trying to decode partial postcard payloads.

- `crates/refwork-harness/src/runner.rs:91-100`: `run_setup` models the setup sequence directly: `Hello`, `LoadGame`, game/region publication, `Ready`, then `Start`. That makes the protocol ordering easy to audit.

- `crates/refwork-harness/src/runner.rs:211-228`: Meta initialization writes cart hash and emulator version before publishing `Ready`, which matches the requirement that the agent observes populated metadata at `Ready { frame: 0 }`.

- `crates/refwork-harness/src/runner.rs:399-555`: The unit tests cover the key state-machine branches: happy path, protocol version mismatch, malformed datagram, oversize datagram, out-of-order first message, bad game, region failure, and out-of-order start.

- `crates/refwork-harness/src/runner.rs:31-34`: Returning both `LoadedGame` and `HarnessRegions` from setup is a good library boundary for the next frame-loop bead.
