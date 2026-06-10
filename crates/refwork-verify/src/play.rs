//! `refwork-verify play` — run a `.padlog` script against a ROM.
//!
//! # Pad policy for frames beyond the script
//!
//! When `--script` is exhausted and `--frames N` extends the run further,
//! the last pad word in the script is held for all remaining frames.  This
//! matches the "hold last input" convention used by the hash-chain probe and
//! keeps double-run reproducible without requiring the caller to pad the
//! script manually.  The `--help` text documents this choice.

use crate::decode::{read_feature_value, FeatureValue};
use refwork_emu::{Cartridge, Core, Fault, RegionBuffers, FB_BYTES, WRAM_INIT_BYTE};
use refwork_featuremap::FeatureMap;
use refwork_hash::{chain_update, frame_hash};
use refwork_script::PadLog;
use serde::{Deserialize, Serialize};

/// A recorded fault during a play run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultRecord {
    pub frame: u64,
    pub description: String,
}

/// A feature-change event recorded during a play run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureEvent {
    pub frame: u64,
    pub feature: String,
    pub old_value: FeatureValue,
    pub new_value: FeatureValue,
}

/// JSON report produced by `play`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayReport {
    /// Last completed frame number (`Core::frame_counter()`).
    pub final_frame: u64,
    /// Final chained hash (64 hex chars).
    pub final_chain_hash: String,
    /// Feature trajectory (change events in order).
    pub feature_trajectory: Vec<FeatureEvent>,
    /// Fault records.
    pub faults: Vec<FaultRecord>,
    /// Set to `true` when `--continue-past-faults` was active.
    pub continue_past_faults: bool,
    /// Per-frame chained hashes at `--hash-every N` intervals (frame, hex).
    pub periodic_hashes: Vec<(u64, String)>,
}

/// Options for the `play` operation.
/// Feature-change callback: `(frame, feature_name, old, new)`.
pub type FeatureChangeHook = Box<dyn Fn(u64, &str, FeatureValue, FeatureValue)>;
/// Generic event callback: `(frame, text)` (faults, periodic hashes).
pub type EventHook = Box<dyn Fn(u64, &str)>;

pub struct PlayOptions<'a> {
    pub rom: Vec<u8>,
    pub script: &'a PadLog,
    pub feature_map: Option<&'a FeatureMap>,
    /// Feature names to emit change events for (empty = all in map).
    pub watch: Vec<String>,
    /// Emit a chained hash line every N frames (0 = disabled).
    pub hash_every: u64,
    /// Override the run length (0 = run script to completion).
    pub frames: u64,
    /// Continue past faults instead of halting. LAB-ONLY.
    pub continue_past_faults: bool,
    /// Framebuffer snapshots: (frame_number, output_path).
    pub snaps: Vec<(u64, String)>,
    /// If Some, write the JSON report here; otherwise None (caller receives
    /// the struct).
    pub report_path: Option<String>,
    /// Callback for printing feature-change events to stdout.  Receives
    /// `(frame, feature_name, old, new)`.  `None` = suppress output.
    pub on_feature_change: Option<FeatureChangeHook>,
    /// Callback invoked on fault `(frame, description)`.
    pub on_fault: Option<EventHook>,
    /// Callback invoked on periodic hash `(frame, hex)`.
    pub on_hash: Option<EventHook>,
}

impl<'a> PlayOptions<'a> {
    /// Minimal constructor: just ROM bytes + script.
    pub fn new(rom: Vec<u8>, script: &'a PadLog) -> Self {
        PlayOptions {
            rom,
            script,
            feature_map: None,
            watch: Vec::new(),
            hash_every: 0,
            frames: 0,
            continue_past_faults: false,
            snaps: Vec::new(),
            report_path: None,
            on_feature_change: None,
            on_fault: None,
            on_hash: None,
        }
    }
}

fn fmt_hash(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Run the play operation.  Returns the report and any hard error.
pub fn play(opts: PlayOptions<'_>) -> Result<PlayReport, String> {
    // ── Construct core ───────────────────────────────────────────────────────
    let cart = Cartridge::from_rom(opts.rom, None).map_err(|e| format!("bad ROM: {:?}", e))?;
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    // ── Run parameters ───────────────────────────────────────────────────────
    let total_frames = if opts.frames > 0 {
        opts.frames
    } else if opts.script.is_empty() {
        0
    } else {
        opts.script.len() as u64
    };

    let last_pad: u16 = opts.script.frames.last().copied().unwrap_or(0);

    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb: Box<[u8; FB_BYTES]> = Box::new([0u8; FB_BYTES]);
    let mut chain = [0u8; 32];
    let mut faults: Vec<FaultRecord> = Vec::new();
    let mut feature_trajectory: Vec<FeatureEvent> = Vec::new();
    let mut periodic_hashes: Vec<(u64, String)> = Vec::new();

    // Track previous values for watched features.
    let mut prev_values: std::collections::BTreeMap<String, FeatureValue> =
        std::collections::BTreeMap::new();

    if opts.continue_past_faults {
        eprintln!();
        eprintln!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        eprintln!("!  WARNING: --continue-past-faults is active.                       !");
        eprintln!("!  Post-fault emulator state is GARBAGE.  This run is               !");
        eprintln!("!  NON-AUTHORITATIVE and MUST NOT be used for acceptance testing.   !");
        eprintln!("!  Use only for fault-inventory reconnaissance in the lab.          !");
        eprintln!("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!");
        eprintln!();
    }

    let fmap = opts.feature_map;
    let watch_set: std::collections::BTreeSet<String> = opts.watch.iter().cloned().collect();

    // Pre-populate previous values from initial WRAM state.
    if let Some(map) = fmap {
        for feat in &map.features {
            if watch_set.is_empty() || watch_set.contains(&feat.name) {
                let v = read_feature_value(feat, core.wram());
                prev_values.insert(feat.name.clone(), v);
            }
        }
    }

    for f in 0..total_frames {
        let pad = if (f as usize) < opts.script.frames.len() {
            opts.script.frames[f as usize]
        } else {
            last_pad
        };

        core.run_one_frame(pad);

        // Check fault.
        let fault: Option<Fault> = core.fault();
        if let Some(fl) = fault {
            let desc = format!("{:?}", fl);
            if let Some(cb) = &opts.on_fault {
                cb(f, &desc);
            }
            faults.push(FaultRecord {
                frame: f,
                description: desc,
            });
            if !opts.continue_past_faults {
                // Emit the chain for the last successful frame if any.
                break;
            }
        }

        // Update chain even when faulted in continue mode (post-fault state
        // is garbage; the report carries continue_past_faults=true).
        core.blit_completed_frame(&mut fb);
        let fh = frame_hash(core.wram(), &fb[..]);
        chain = chain_update(&chain, &fh);

        let frame_no = core.frame_counter();

        // Snapshots.
        for (snap_frame, snap_path) in &opts.snaps {
            if *snap_frame == f {
                if let Err(e) = std::fs::write(snap_path, &fb[..]) {
                    eprintln!("play: snap write error for {}: {}", snap_path, e);
                }
            }
        }

        // Periodic hashes.
        if opts.hash_every > 0 && (f + 1) % opts.hash_every == 0 {
            let hex = fmt_hash(&chain);
            if let Some(cb) = &opts.on_hash {
                cb(frame_no, &hex);
            }
            periodic_hashes.push((frame_no, hex));
        }

        // Feature watch.
        if let Some(map) = fmap {
            for feat in &map.features {
                if !watch_set.is_empty() && !watch_set.contains(&feat.name) {
                    continue;
                }
                let cur = read_feature_value(feat, core.wram());
                let prev = *prev_values.get(&feat.name).unwrap_or(&cur);
                if cur != prev {
                    if let Some(cb) = &opts.on_feature_change {
                        cb(frame_no, &feat.name, prev, cur);
                    }
                    feature_trajectory.push(FeatureEvent {
                        frame: frame_no,
                        feature: feat.name.clone(),
                        old_value: prev,
                        new_value: cur,
                    });
                    *prev_values.entry(feat.name.clone()).or_insert(cur) = cur;
                }
            }
        }
    }

    let report = PlayReport {
        final_frame: core.frame_counter(),
        final_chain_hash: fmt_hash(&chain),
        feature_trajectory,
        faults,
        continue_past_faults: opts.continue_past_faults,
        periodic_hashes,
    };

    // Write report if path given.
    if let Some(path) = &opts.report_path {
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| format!("report serialization failed: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("cannot write report to {}: {}", path, e))?;
    }

    Ok(report)
}

/// Return the pad word for frame `f` using the synthetic-ROM determinism
/// policy: `(f as u16).wrapping_mul(0x9E37) & 0x0FFF`.
///
/// Exposed so integration tests can build a matching `.padlog` without
/// importing xtask.
pub fn synth_pad(frame: usize) -> u16 {
    (frame as u16).wrapping_mul(0x9E37) & 0x0FFF
}

/// Build a [`PadLog`] for `frames` frames using the synthetic-ROM pad policy.
pub fn build_synth_padlog(frames: usize) -> Result<PadLog, refwork_script::PadLogError> {
    let words: Vec<u16> = (0..frames).map(synth_pad).collect();
    PadLog::from_frames(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_pad_matches_xtask_definition() {
        // Spot-check a few values against the known formula.
        assert_eq!(synth_pad(0), 0x0000);
        assert_eq!(synth_pad(1), (1u16.wrapping_mul(0x9E37)) & 0x0FFF);
        assert_eq!(synth_pad(100), (100u16.wrapping_mul(0x9E37)) & 0x0FFF);
    }
}
