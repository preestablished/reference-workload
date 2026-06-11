//! `hash-chain`: run the synthetic ROM for N frames and print a single
//! chained frame hash — the cross-architecture determinism probe.
//!
//! Two machines (e.g. x86_64 and aarch64) running the same build of the
//! same workload must print the same chain (IMPLEMENTATION-PLAN.md M2:
//! "cross-arch identical hashes — catches latent float/UB issues early").
//! Per-frame hash matches the determinism suite: `blake3(wram ‖ fb)`;
//! the chain is `chain = blake3(chain ‖ frame_hash)` from a zero chain.
//!
//! The hash definitions live in `refwork-hash` (see `crates/refwork-hash/`)
//! so xtask and `refwork-verify` share a single source of truth and can
//! never silently disagree on the algorithm.

use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES, WRAM_INIT_BYTE};
use refwork_hash::{chain_update, frame_hash};

/// Deterministic per-frame pad word (same function as the determinism
/// suite in `xtask/tests/determinism.rs`).
pub fn pad(frame: usize) -> u16 {
    (frame as u16).wrapping_mul(0x9E37) & 0x0FFF
}

/// Run `frames` frames and return the final chained hash.
pub fn run_hash_chain(frames: usize) -> Result<[u8; 32], String> {
    // Single-sourced: `build_synth_rom` is the same function `build-rom`
    // writes to disk, so this probe and the built artifact are
    // byte-identical by construction.
    let rom = crate::build_synth_rom();
    let cart = Cartridge::from_rom(rom, None).map_err(|e| format!("bad synth ROM: {:?}", e))?;
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    let mut fb = [0u8; FB_BYTES];
    let mut chain = [0u8; 32];
    for f in 0..frames {
        core.run_one_frame(pad(f));
        if let Some(fault) = core.fault() {
            return Err(format!("fault at frame {}: {:?}", f, fault));
        }
        core.blit_completed_frame(&mut fb);
        let fh = frame_hash(core.wram(), &fb);
        chain = chain_update(&chain, &fh);
    }
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_is_stable_across_runs() {
        let a = run_hash_chain(30).unwrap();
        let b = run_hash_chain(30).unwrap();
        assert_eq!(a, b);
    }
}
