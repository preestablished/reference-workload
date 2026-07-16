# Suggestions

## Non-Blocking Improvements

### Make `emu_buffers` transactional before marking regions published

Path: `crates/refwork-harness/src/regions.rs:138`, `crates/refwork-harness/src/regions.rs:233`

What/why: `emu_buffers` marks `wram` as published before later `vram` size validation can fail. That leaves the owner partially published even though no `RegionBuffers` was returned. It is not currently reachable through public constructors except by manual struct construction, but the unsafe boundary is clearer if validation happens first.

Suggested snippet:

```rust
fn validate_emu_buffer_sizes(&self) -> Result<(), RegionError> {
    self.wram.expect_len::<WRAM_SIZE>()?;
    if let Some(vram) = &self.vram {
        vram.expect_len::<VRAM_SIZE>()?;
    }
    Ok(())
}

pub unsafe fn emu_buffers(&mut self) -> Result<RegionBuffers, RegionError> {
    self.validate_emu_buffer_sizes()?;
    Ok(RegionBuffers {
        wram: unsafe { self.wram.static_array_unchecked::<WRAM_SIZE>() },
        vram: self.vram.as_mut().map(|r| unsafe { r.static_array_unchecked::<VRAM_SIZE>() }),
        sram: self.sram.as_mut().map(|r| unsafe { r.static_slice() }),
    })
}
```

### Update stale `hello_ack` dependency documentation

Path: `crates/refwork-harness/src/lib.rs:11`

What/why: The doc comment still says the harness avoids a compile-time dependency on `refwork-emu`, but this branch adds that dependency in `Cargo.toml` and uses it in `regions.rs`. This is small, but stale safety/scope comments tend to mislead future reviewers.

Suggested snippet:

```rust
/// Returns `CtlMsg::HelloAck` populated with the current protocol version and
/// the supplied emulator identity strings.
```
