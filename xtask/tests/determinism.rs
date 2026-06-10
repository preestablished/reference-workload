//! Determinism acceptance tests.
//!
//! These tests compile against the frozen public API of refwork-emu.
//! They will fail at runtime with todo!() panics until the integration
//! agent completes the emulator implementation — that is expected.
//! The tests are structurally correct and must compile.

use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES};

/// A fixed deterministic pad function: deterministic per-frame input.
fn pad(frame: usize) -> u16 {
    // Multiply by a Knuth constant and mask to valid joypad bits (12 bits used).
    let raw = (frame as u16).wrapping_mul(0x9E37);
    raw & 0x0FFF
}

/// Build a `Core` from the synth ROM. Leaks WRAM as required by the API.
fn make_core() -> Core {
    let rom = xtask::build_synth_rom();
    let cart = Cartridge::from_rom(rom, None).expect("synth ROM must be valid");
    let wram: &'static mut [u8; 0x20000] =
        Box::leak(Box::new([refwork_emu::WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    Core::new(cart, regions).expect("core construction must succeed")
}

/// Run `frames` frames, hashing `wram ‖ framebuffer` after each frame.
/// Returns the per-frame hash vector.
fn run_hashes(frames: usize) -> Vec<[u8; 32]> {
    let mut core = make_core();
    let mut fb = [0u8; FB_BYTES];
    let mut hashes = Vec::with_capacity(frames);

    for f in 0..frames {
        let _flags = core.run_one_frame(pad(f));
        assert!(
            core.fault().is_none(),
            "unexpected fault at frame {}: {:?}",
            f,
            core.fault()
        );
        core.blit_completed_frame(&mut fb);

        // Frame hash (README glossary): BLAKE3 over wram ‖ framebuffer.
        let mut hasher = blake3::Hasher::new();
        hasher.update(core.wram());
        hasher.update(&fb);
        let hash: [u8; 32] = hasher.finalize().into();
        hashes.push(hash);
    }

    hashes
}

#[test]
fn determinism_600_frames() {
    let hashes1 = run_hashes(600);
    let hashes2 = run_hashes(600);

    assert_eq!(hashes1.len(), hashes2.len(), "hash vector lengths differ");
    for (i, (h1, h2)) in hashes1.iter().zip(hashes2.iter()).enumerate() {
        assert_eq!(
            h1,
            h2,
            "frame {} hash mismatch: run1={} run2={}",
            i,
            hex(h1),
            hex(h2)
        );
    }

    // Frame-600 framebuffer must not be all one color, and the ROM's CPU
    // work loop must be live: the rolling checksum cell at $7E:10FE and the
    // frame counter at $7E:0010 change between frames 100 and 600.
    let mut core = make_core();
    let mut fb = [0u8; FB_BYTES];
    for f in 0..100 {
        core.run_one_frame(pad(f));
    }
    let chksum_100 = [core.wram()[0x10FE], core.wram()[0x10FF]];
    let frame_ctr_100 = [core.wram()[0x0010], core.wram()[0x0011]];
    for f in 100..600 {
        core.run_one_frame(pad(f));
    }
    let chksum_600 = [core.wram()[0x10FE], core.wram()[0x10FF]];
    let frame_ctr_600 = [core.wram()[0x0010], core.wram()[0x0011]];
    assert_ne!(
        frame_ctr_100, frame_ctr_600,
        "WRAM frame counter is not advancing — NMI handler not running"
    );
    assert_ne!(
        chksum_100, chksum_600,
        "WRAM rolling checksum unchanged — CPU exercise loop not running"
    );
    core.blit_completed_frame(&mut fb);
    let first_pixel = [fb[0], fb[1], fb[2], fb[3]];
    let all_same = fb.chunks(4).all(|p| p == first_pixel);
    assert!(
        !all_same,
        "frame 600 framebuffer is all one color — emulator not drawing"
    );
}

#[test]
#[ignore]
fn determinism_10000_frames() {
    // CI acceptance gate (run in release): same double-run over 10_000 frames.
    let hashes1 = run_hashes(10_000);
    let hashes2 = run_hashes(10_000);

    for (i, (h1, h2)) in hashes1.iter().zip(hashes2.iter()).enumerate() {
        assert_eq!(h1, h2, "frame {} hash mismatch at 10k run", i);
    }
}

fn hex(b: &[u8; 32]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}
