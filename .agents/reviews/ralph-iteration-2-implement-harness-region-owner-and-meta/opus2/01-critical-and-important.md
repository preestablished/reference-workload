# Critical And Important Issues

## Critical

### 1. Published regions can still be safely aliased after `emu_buffers`

Severity: Critical

Path: `crates/refwork-harness/src/regions.rs:119`, `crates/refwork-harness/src/regions.rs:138`, `crates/refwork-harness/src/regions.rs:151`, `crates/refwork-harness/src/regions.rs:233`

Description: `HarnessRegions::emu_buffers` returns `RegionBuffers` containing `'static mut` references to WRAM, VRAM, and SRAM, but the owner still exposes safe APIs that can alias those buffers. `PublishedRegion::as_mut_slice` checks `published`, but `as_slice` does not, and the internal `static_array`/`static_slice` helpers do not reject an already-published region. That means the publication flag gives partial protection while tests assert only the mutable-slice case. The safe shared slice is enough to violate the uniqueness contract of an outstanding `&'static mut`, and a second bridge call can mint another mutable reference to the same allocation.

Suggested fix: make the publication guard apply to every API that dereferences the allocation, make the bridge one-shot, and add tests for safe shared reads plus repeated bridge attempts after publication. Keeping `descriptors()` available is fine because it only reads pointer metadata, not region bytes.

```rust
impl PublishedRegion {
    fn ensure_unpublished(&self) -> Result<(), RegionError> {
        if self.published {
            return Err(RegionError::Published { name: self.name });
        }
        Ok(())
    }

    pub fn as_slice(&self) -> Result<&[u8], RegionError> {
        self.ensure_unpublished()?;
        // Safety: `ptr` came from an allocation with `len` bytes and remains
        // live until `Drop`; publication has not handed out static aliases.
        Ok(unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) })
    }

    pub fn as_mut_slice(&mut self) -> Result<&mut [u8], RegionError> {
        self.ensure_unpublished()?;
        // Safety: caller has `&mut self`, and the region is unpublished.
        Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) })
    }

    unsafe fn static_array<const N: usize>(&mut self) -> Result<&'static mut [u8; N], RegionError> {
        self.ensure_unpublished()?;
        if self.len != N {
            return Err(RegionError::WrongSize {
                name: self.name,
                expected: N,
                actual: self.len,
            });
        }
        self.mark_published();
        Ok(unsafe { &mut *(self.ptr.as_ptr() as *mut [u8; N]) })
    }

    unsafe fn static_slice(&mut self) -> Result<&'static mut [u8], RegionError> {
        self.ensure_unpublished()?;
        self.mark_published();
        Ok(unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) })
    }
}
```

Add a regression test along these lines:

```rust
#[test]
fn emu_bridge_is_one_shot_and_blocks_owner_slices() {
    let mut regions = HarnessRegions::with_optional(true, Some(8192)).unwrap();
    let _buffers = unsafe { regions.emu_buffers() }.unwrap();

    assert!(matches!(
        regions.wram.as_slice(),
        Err(RegionError::Published { name: "wram" })
    ));
    assert!(matches!(
        unsafe { regions.emu_buffers() },
        Err(RegionError::Published { name: "wram" })
    ));
}
```

## Important

### 1. The owner remains droppable and publicly replaceable while `'static mut` buffers may exist

Severity: Important

Path: `crates/refwork-harness/src/regions.rs:158`, `crates/refwork-harness/src/regions.rs:176`, `crates/refwork-harness/src/regions.rs:233`

Description: The bridge returns `'static mut` references while leaving the `HarnessRegions` value as an ordinary owner with a normal `Drop` implementation behind it. The safety comment says the caller must not drop or access the owner until the `Core` stops, but the type shape does not help enforce that, and the `HarnessRegions` fields are public. A safe field assignment such as replacing `regions.wram` would drop the old allocation while the emulator still holds it. This is the same lifetime hazard as dropping the whole owner, just made easier by public fields.

Suggested fix: either make the fields private and consume the owner into an active guard that cannot be safely dereferenced, or leak the bridged allocations for the process lifetime until `refwork_emu::RegionBuffers` can carry real lifetimes. A minimal guard shape could be:

```rust
use std::mem::ManuallyDrop;

pub struct ActiveEmuRegions {
    _regions: ManuallyDrop<HarnessRegions>,
    pub buffers: RegionBuffers,
}

impl HarnessRegions {
    pub unsafe fn activate_for_emu(mut self) -> Result<ActiveEmuRegions, RegionError> {
        let buffers = unsafe { self.emu_buffers()? };
        Ok(ActiveEmuRegions {
            _regions: ManuallyDrop::new(self),
            buffers,
        })
    }
}
```

Longer term, the better fix is to give `RegionBuffers` and `Core` a lifetime parameter so the compiler can prove the core cannot outlive the region owner.

### 2. SRAM publication has no way to represent valid sub-page logical sizes

Severity: Important

Path: `crates/refwork-harness/src/regions.rs:78`, `crates/refwork-harness/src/regions.rs:195`

Description: `PublishedRegion::new` requires every allocation length to be a 4096-byte multiple, and `with_optional` passes `sram_len` directly into that allocator. The API requires registered regions to be page multiples, but `refwork_emu::Cartridge::from_rom` accepts logical SRAM lengths down to 2048 bytes and uses the slice length for mirroring. As written, `with_optional(true, Some(2048))` rejects a valid emulator SRAM size; if a caller works around that by rounding to 4096 before passing the slice to the core, the cartridge mirroring semantics change.

Suggested fix: model mapped length and logical SRAM length separately. Publish the page-rounded backing range in the descriptor, but pass only the logical prefix to `RegionBuffers`.

```rust
pub struct SramRegion {
    backing: PublishedRegion,
    logical_len: usize,
}

impl SramRegion {
    pub fn new(logical_len: usize) -> Result<Self, RegionError> {
        if !logical_len.is_power_of_two() || !(2048..=512 * 1024).contains(&logical_len) {
            return Err(RegionError::WrongSize {
                name: "sram",
                expected: 2048,
                actual: logical_len,
            });
        }

        let mapped_len = logical_len.next_multiple_of(PAGE_SIZE);
        Ok(Self {
            backing: PublishedRegion::new("sram", mapped_len)?,
            logical_len,
        })
    }

    unsafe fn emu_slice(&mut self) -> Result<&'static mut [u8], RegionError> {
        self.backing.static_slice_prefix(self.logical_len)
    }
}
```

Add tests for `Some(2048)` and for the descriptor length staying page-rounded while the emulator slice length stays 2048.
