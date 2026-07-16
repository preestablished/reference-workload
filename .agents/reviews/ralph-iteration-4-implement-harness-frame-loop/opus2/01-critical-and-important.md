# Critical And Important Issues

## Important

### `crates/refwork-harness/src/frame.rs:130`

Severity: Important

The `EmuHalt` path collapses every emulator fault into the generic detail string `"core returned FAULTED"`. The core exposes the concrete halt reason via `Core::fault()` (`crates/refwork-emu/src/core_impl.rs:204`), and the control protocol has a `Fault.detail` field specifically for this diagnostic payload. Dropping that detail makes runtime protocol failures much harder to triage and weakens the "fail loudly" behavior expected from emulator faults.

Suggested fix:

```rust
let flags = self.core.run_one_frame(pad);
if flags.contains(FrameFlags::FAULTED) {
    let frame = self.core.frame_counter();
    let detail = self
        .core
        .fault()
        .map(|fault| format!("{fault:?}"))
        .unwrap_or_else(|| "core returned FAULTED without fault detail".to_owned());
    return self.fault_emu(channel, frame, &detail);
}
```

Add a focused unit test with a ROM or injected core path that produces a deterministic emulator fault, then assert the outbound `CtlMsg::Fault { code: FaultCode::EmuHalt, detail, .. }` includes the concrete fault text rather than only the generic wrapper.

## Critical

None.
