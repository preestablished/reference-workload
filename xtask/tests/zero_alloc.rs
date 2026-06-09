//! Zero-allocation-per-frame acceptance test (D8).
//!
//! Uses a counting global allocator to verify that after the first frame,
//! running 200 subsequent frames (plus blit) does not trigger any heap
//! allocations.
//!
//! Will fail at runtime with todo!() panics until the integration agent
//! completes the emulator — that is expected. The test must compile.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES};

// ─── counting allocator ───────────────────────────────────────────────────────

static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

// ─── test ─────────────────────────────────────────────────────────────────────

fn pad(frame: usize) -> u16 {
    (frame as u16).wrapping_mul(0x9E37) & 0x0FFF
}

#[test]
fn zero_alloc_per_frame_after_warmup() {
    // Build ROM and Core (allocations expected here).
    let rom = xtask::build_synth_rom();
    let cart = Cartridge::from_rom(rom, None).expect("synth ROM must be valid");
    let wram: &'static mut [u8; 0x20000] =
        Box::leak(Box::new([refwork_emu::WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core = Core::new(cart, regions).expect("core construction must succeed");
    let mut fb = [0u8; FB_BYTES];

    // Run frame 0 (warmup — allocations allowed).
    core.run_one_frame(pad(0));
    core.blit_completed_frame(&mut fb);

    // Snapshot count after warmup.
    let count_before = ALLOC_COUNT.load(Ordering::Relaxed);

    // Run 200 more frames with blit.
    for f in 1..=200 {
        core.run_one_frame(pad(f));
        core.blit_completed_frame(&mut fb);
    }

    let count_after = ALLOC_COUNT.load(Ordering::Relaxed);
    assert_eq!(
        count_after,
        count_before,
        "D8 VIOLATION: {} allocation(s) occurred during frames 1-200 (expected 0)",
        count_after - count_before
    );
}
