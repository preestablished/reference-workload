# Suggestions

### 1. Refresh the stale dependency comment

Path: `crates/refwork-harness/src/lib.rs:11`

What/why: The `hello_ack` docs still say the helper avoids a compile-time dependency on `refwork-emu`, but this branch adds that dependency in `Cargo.toml`. The helper remains parameterized, but the reason is no longer dependency avoidance.

Suggested snippet:

```rust
/// Returns `CtlMsg::HelloAck` populated with the current protocol version and
/// the supplied emulator identity strings. The helper stays parameterized so
/// tests and future harness modes can choose the advertised identity.
```

### 2. Re-tighten unsafe policy outside the region owner

Path: `crates/refwork-harness/src/lib.rs:1`, `crates/refwork-harness/src/meta.rs:1`, `crates/refwork-harness/src/main.rs:1`

What/why: Relaxing the crate root from `forbid(unsafe_code)` is necessary for `regions.rs`, but it also allows unsafe code in unrelated future modules. Add module-level forbids to safe files so the unsafe boundary stays visibly confined.

Suggested snippet:

```rust
// crates/refwork-harness/src/meta.rs
#![forbid(unsafe_code)]
```

```rust
// crates/refwork-harness/src/main.rs
#![forbid(unsafe_code)]
```

### 3. Write `meta.status` last during transitions

Path: `crates/refwork-harness/src/meta.rs:55`, `crates/refwork-harness/src/meta.rs:61`

What/why: Current setters publish the status before the payload fields. If any host-side probe ever observes the page outside the intended frame-boundary pause, it can see `running` or `faulted` with stale frame/pad/fault data. Writing payload first and status last makes the status act as a simple commit marker.

Suggested snippet:

```rust
pub fn set_running_frame(&mut self, frame: u64, last_pad: u16) {
    self.set_frame(frame);
    self.set_last_pad(last_pad);
    self.set_status(MetaStatus::Running);
}

pub fn set_fault(&mut self, frame: u64, code: FaultCode) {
    self.set_frame(frame);
    self.write_u32(FAULT_CODE_OFF, fault_code_value(code));
    self.set_status(MetaStatus::Faulted);
}
```

### 4. Narrow the dead-code allows to the build modes that need them

Path: `crates/refwork-emu/src/apu/mod.rs:191`, `crates/refwork-emu/src/apu/spc700.rs:650`

What/why: The new allows are narrow, which is good, but unconditional `allow(dead_code)` can hide later accidental dead paths in tests and introspection builds. The existing codebase already uses conditional allows elsewhere.

Suggested snippet:

```rust
#[cfg_attr(not(any(test, feature = "introspect")), allow(dead_code))]
pub fn mem_write(&mut self, addr: u16, value: u8) {
    // ...
}
```
