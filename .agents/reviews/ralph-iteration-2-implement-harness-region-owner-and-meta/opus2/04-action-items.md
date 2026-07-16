## Action Items

### Critical

- [ ] [crates/refwork-harness/src/regions.rs:119] Block safe owner slice access and repeated bridges after `emu_buffers` publishes `'static mut` buffers.

### Important

- [ ] [crates/refwork-harness/src/regions.rs:158] Replace the droppable/public post-bridge owner shape with a private active guard, process-lifetime leak, or lifetime-parameterized core API.
- [ ] [crates/refwork-harness/src/regions.rs:195] Separate SRAM mapped length from logical emulator length so valid 2048-byte SRAM can be published without changing mirroring semantics.

### Suggestions

- [ ] [crates/refwork-harness/src/lib.rs:11] Refresh the outdated `hello_ack` dependency comment.
- [ ] [crates/refwork-harness/src/lib.rs:1] Re-tighten unsafe-code policy in safe harness modules.
- [ ] [crates/refwork-harness/src/meta.rs:55] Write `meta` payload fields before status transitions.
- [ ] [crates/refwork-emu/src/apu/mod.rs:191] Narrow the new dead-code allows to the build modes that require them.
