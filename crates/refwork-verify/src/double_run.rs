//! `refwork-verify double-run` — host-side determinism gate.
//!
//! Runs a script twice from fresh `Core::new`, compares chained hashes.
//! On mismatch, binary-searches to find the first divergent frame and
//! reports which region (wram vs fb) differs.
//!
//! # Test-only divergence hook
//!
//! Setting the environment variable `REFWORK_NONDET_TEST=1` (or the
//! `--nondet-test` CLI flag) perturbs the **hash input** on run 2: the first
//! byte of every per-frame `frame_hash` output is XOR-ed with `0x01` before
//! it is fed into `chain_update`.  This guarantees divergence from frame 0
//! regardless of whether the ROM reflects the pad value into WRAM (the spec
//! explicitly allows perturbing "the pad stream **or hash input**").
//!
//! **This hook lives here, never in `refwork-emu`**: the deny gate scans
//! `refwork-emu` source for non-determinism tokens regardless of cfg flags;
//! perturbing the hash from the host side does not touch the core source and
//! cannot weaken the deny gate.
//!
//! The `--nondet-test` CLI flag is an alias for the environment variable.
//! Both are prominently labelled TEST-ONLY in `--help` and must not appear
//! in acceptance or CI scripted runs.
//!
//! **Rejects `--continue-past-faults`**: a double-run with a faulted core
//! on either leg is undefined; the command exits non-zero when the flag is
//! passed or when a report artifact carries `continue_past_faults: true`.

use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES, WRAM_INIT_BYTE};
use refwork_hash::{chain_update, frame_hash};
use refwork_script::PadLog;
use serde::{Deserialize, Serialize};

/// Result of a double-run comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoubleRunReport {
    pub frames_run: u64,
    pub chain_a: String,
    pub chain_b: String,
    pub deterministic: bool,
    /// Present when `deterministic` is false: the first frame where the two
    /// runs diverged.
    pub first_divergent_frame: Option<u64>,
    /// Which region diverged first ("wram", "fb", or "both").
    pub divergent_region: Option<String>,
}

fn fmt_hash(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Run one leg: returns per-frame chains, the final fb, and the final wram.
///
/// `hash_perturb`: when non-zero, XORs the first byte of every per-frame hash
/// with this value before chaining.  This provides a guaranteed divergence
/// signal for the `nondet_test` path that is independent of whether the ROM
/// actually reflects the pad stream into WRAM (e.g. a stale-read-only joypad
/// protocol never writes the raw pad value to WRAM, so pad-stream XOR alone
/// would produce no observable WRAM diff).  The spec explicitly allows
/// perturbing "the pad stream **or hash input**" on run 2.
/// One leg's evidence: per-frame hashes, final framebuffer, final WRAM.
type LegState = (Vec<[u8; 32]>, Box<[u8; FB_BYTES]>, Box<[u8; 0x20000]>);

fn run_one_leg(
    rom_bytes: &[u8],
    script: &PadLog,
    total: u64,
    hash_perturb: u8,
) -> Result<LegState, String> {
    let rom = rom_bytes.to_vec();
    let cart = Cartridge::from_rom(rom, None).map_err(|e| format!("bad ROM: {:?}", e))?;
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    let last_pad: u16 = script.frames.last().copied().unwrap_or(0);
    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb: Box<[u8; FB_BYTES]> = Box::new([0u8; FB_BYTES]);
    let mut chain = [0u8; 32];
    let mut per_frame: Vec<[u8; 32]> = Vec::with_capacity(total as usize);

    for f in 0..total {
        let pad = if (f as usize) < script.frames.len() {
            script.frames[f as usize]
        } else {
            last_pad
        };

        core.run_one_frame(pad);

        if let Some(fault) = core.fault() {
            return Err(format!("fault at frame {} leg: {:?}", f, fault));
        }

        core.blit_completed_frame(&mut fb);
        let mut fh = frame_hash(core.wram(), &fb[..]);
        // TEST-ONLY: perturb the hash directly so divergence is guaranteed
        // regardless of whether the ROM reflects the pad value into WRAM.
        if hash_perturb != 0 {
            fh[0] ^= hash_perturb;
        }
        chain = chain_update(&chain, &fh);
        per_frame.push(chain);
    }

    // Copy WRAM into an owned box so we can compare below the borrow.
    let wram_snap: Box<[u8; 0x20000]> = Box::new(*core.wram());
    Ok((per_frame, fb, wram_snap))
}

/// Run the double-run determinism gate.
///
/// `nondet_test`: when true, perturbs run 2's pad stream so the run
/// deliberately diverges (TEST-ONLY).
pub fn double_run(
    rom: Vec<u8>,
    script: &PadLog,
    frames: u64,
    nondet_test: bool,
) -> Result<DoubleRunReport, String> {
    let total = if frames > 0 {
        frames
    } else {
        script.len() as u64
    };

    // TEST-ONLY: perturb the hash input on run 2 so CI can assert divergence.
    // We perturb the *hash* rather than the pad stream because the synth ROM
    // reads joypad via a stale-read protocol (waits for auto-joy or uses
    // joy_prev), so a pad-stream XOR alone may not produce any WRAM diff.
    // Perturbing the hash input byte[0] guarantees divergence from frame 0.
    let hash_perturb_b: u8 = if nondet_test { 0x01 } else { 0x00 };

    let (chains_a, fb_a, wram_a) = run_one_leg(&rom, script, total, 0x00)?;
    let (chains_b, fb_b, wram_b) = run_one_leg(&rom, script, total, hash_perturb_b)?;

    let chain_a = fmt_hash(chains_a.last().unwrap_or(&[0u8; 32]));
    let chain_b = fmt_hash(chains_b.last().unwrap_or(&[0u8; 32]));

    if chain_a == chain_b {
        return Ok(DoubleRunReport {
            frames_run: total,
            chain_a,
            chain_b,
            deterministic: true,
            first_divergent_frame: None,
            divergent_region: None,
        });
    }

    // Binary search for first divergent frame.
    // Invariant: chains_a[lo] == chains_b[lo] (or lo is uninitialized / frame 0 diverges).
    // We search for the smallest index i where chains_a[i] != chains_b[i].
    let first_div = if chains_a.is_empty() || chains_a[0] != chains_b[0] {
        // Diverges at the very first frame (or no frames ran).
        0u64
    } else {
        // lo is known-matching; hi is first potentially-diverging.
        let mut lo = 0usize;
        let mut hi = total as usize;
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if chains_a[mid] == chains_b[mid] {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi as u64
    };

    // Determine which region diverges by comparing the final state.
    // (For a more precise region diagnosis we would need per-frame wram/fb
    // snapshots, but that is M5 scope; here we compare the final states.)
    let wram_diff = *wram_a != *wram_b;
    let fb_diff = fb_a != fb_b;
    let region = match (wram_diff, fb_diff) {
        (true, true) => "both",
        (true, false) => "wram",
        (false, true) => "fb",
        (false, false) => "unknown (chain differed but final state matches)",
    };

    Ok(DoubleRunReport {
        frames_run: total,
        chain_a,
        chain_b,
        deterministic: false,
        first_divergent_frame: Some(first_div),
        divergent_region: Some(region.to_string()),
    })
}
