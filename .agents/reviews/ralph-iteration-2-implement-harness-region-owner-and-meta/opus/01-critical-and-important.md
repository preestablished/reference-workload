# Critical And Important

## Critical

None.

## Important

### Important: Published regions are heap allocations, not locked/populated mappings

Path: `crates/refwork-harness/src/regions.rs:81`

Description: `PublishedRegion::new` builds the advertised regions with `Layout::from_size_align` and `alloc_zeroed`. This gives page alignment and zeroing, but it does not satisfy API.md 3.5's requirement that registered region memory is already `MAP_LOCKED|MAP_POPULATE`. `descriptors()` exposes these GVAs as publishable regions, so the later harness can appear to register compliant regions while they are ordinary allocator pages that can be lazily faulted or swapped. That is the wrong unsafe boundary for D7 publication.

Suggested fix:

```rust
#[cfg(target_os = "linux")]
fn map_region(name: &'static str, len: usize) -> Result<NonNull<u8>, RegionError> {
    let raw = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE
                | libc::MAP_ANONYMOUS
                | libc::MAP_LOCKED
                | libc::MAP_POPULATE,
            -1,
            0,
        )
    };
    if raw == libc::MAP_FAILED {
        return Err(RegionError::Map { name, len });
    }
    Ok(NonNull::new(raw.cast::<u8>()).unwrap())
}
```

Store the allocation kind in `PublishedRegion` and use `munmap` on drop for mapped regions. If a non-Linux/test fallback remains, keep it explicitly non-production and do not emit `RegisterRegion` descriptors from an unlocked allocation path.

### Important: Meta status is written before the fields it publishes

Path: `crates/refwork-harness/src/meta.rs:50`, `crates/refwork-harness/src/meta.rs:55`, `crates/refwork-harness/src/meta.rs:61`

Description: `set_ready`, `set_running_frame`, and `set_fault` all write `status` first, then update the frame, pad, or fault code. The meta page is a published host-readable region, and `status` is the reader's state discriminator. A host-side read can therefore observe `ready` with the old frame, `running` with the previous frame/pad, or `faulted` before the fault code is stored. This makes the acceptance signal misleading at exactly the boundary API.md 3.6 is meant to sanity-check.

Suggested fix:

```rust
use std::sync::atomic::{compiler_fence, Ordering};

impl<'a> MetaPage<'a> {
    fn publish_status(&mut self, status: MetaStatus) {
        compiler_fence(Ordering::Release);
        self.write_u32(STATUS_OFF, status as u32);
    }

    pub fn set_ready(&mut self) {
        self.set_frame(0);
        self.set_last_pad(0);
        self.write_u32(FAULT_CODE_OFF, 0);
        self.publish_status(MetaStatus::Ready);
    }

    pub fn set_running_frame(&mut self, frame: u64, last_pad: u16) {
        self.set_frame(frame);
        self.set_last_pad(last_pad);
        self.publish_status(MetaStatus::Running);
    }

    pub fn set_fault(&mut self, frame: u64, code: FaultCode) {
        self.set_frame(frame);
        self.write_u32(FAULT_CODE_OFF, fault_code_value(code));
        self.publish_status(MetaStatus::Faulted);
    }
}
```

If the final frame loop establishes a stronger quiescent publication point, keep status-last anyway; it is the simplest local invariant for external readers.
