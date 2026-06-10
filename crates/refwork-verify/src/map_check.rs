//! `refwork-verify map-check` — run a script and assert an expectations file.
//!
//! # Exit semantics (CLI)
//!
//! - Exit 0: all assertions passed.
//! - Exit 1: first failing assertion (diagnostics printed to stderr).
//! - Exit 2: expectations file error (bad schema, unknown feature, or an
//!   assertion targets a feature whose `valid_when` is false at that frame).
//!
//! The operation **rejects** `--continue-past-faults` (either as a CLI flag
//! or in a JSON report artifact) and exits non-zero.

use crate::decode::{find_feature, is_valid, read_feature_value, FeatureValue};
use crate::expectations::{Assertion, Expectations};
use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES, WRAM_INIT_BYTE};
use refwork_featuremap::FeatureMap;
use refwork_hash::{chain_update, frame_hash};
use refwork_script::PadLog;

/// Outcome of a map-check run.
#[derive(Debug)]
pub enum MapCheckResult {
    /// All assertions passed.
    Pass,
    /// An expectations-file error (bad syntax, unknown feature, …).
    ExpectationsError(String),
    /// An assertion failed.
    Failure {
        frame: u64,
        feature: String,
        expected_description: String,
        actual: FeatureValue,
        raw_bytes: Vec<u8>,
    },
}

/// Run the map-check operation.
///
/// Returns `Ok(MapCheckResult)` on success or if assertions fail.
/// Returns `Err(String)` only on unrecoverable errors (ROM parse failure,
/// core construction failure).
pub fn map_check(
    rom: Vec<u8>,
    script: &PadLog,
    map: &FeatureMap,
    expectations: &Expectations,
    frames_override: Option<u64>,
) -> Result<MapCheckResult, String> {
    // Validate all assertions up front (schema errors are exit-2 failures).
    for a in &expectations.assertions {
        if let Err(e) = a.validate() {
            return Ok(MapCheckResult::ExpectationsError(e));
        }
        // Feature must exist in the map.
        if find_feature(map, &a.feature).is_none() {
            return Ok(MapCheckResult::ExpectationsError(format!(
                "assertion references unknown feature {:?} (not in feature map)",
                a.feature
            )));
        }
    }
    for n in &expectations.never {
        if find_feature(map, &n.feature).is_none() {
            return Ok(MapCheckResult::ExpectationsError(format!(
                "never clause references unknown feature {:?} (not in feature map)",
                n.feature
            )));
        }
    }

    // ── Construct core ───────────────────────────────────────────────────────
    let cart = Cartridge::from_rom(rom, None).map_err(|e| format!("bad ROM: {:?}", e))?;
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    let total_frames = frames_override.unwrap_or(script.len() as u64);
    let last_pad: u16 = script.frames.last().copied().unwrap_or(0);
    let mut fb = [0u8; FB_BYTES];
    let mut chain = [0u8; 32];

    // Previous values for delta and changes_to assertions.
    let mut prev_values: std::collections::BTreeMap<String, FeatureValue> =
        std::collections::BTreeMap::new();

    // Track which by_frame assertions still need to fire.
    let mut pending: Vec<(usize, &Assertion)> =
        expectations.assertions.iter().enumerate().collect();

    for f in 0..total_frames {
        let pad = if (f as usize) < script.frames.len() {
            script.frames[f as usize]
        } else {
            last_pad
        };

        core.run_one_frame(pad);

        if core.fault().is_some() {
            // A fault halts the run; any untriggered assertions become
            // failures only if they had a deadline ≤ f.
            return Ok(MapCheckResult::Failure {
                frame: f,
                feature: String::from("<core fault>"),
                expected_description: format!("no fault before frame {}", f),
                actual: 0,
                raw_bytes: Vec::new(),
            });
        }

        core.blit_completed_frame(&mut fb);
        let fh = frame_hash(core.wram(), &fb);
        chain = chain_update(&chain, &fh);

        let frame_no = core.frame_counter();

        // ── Check never-clauses ──────────────────────────────────────────────
        for nc in &expectations.never {
            if let Some(feat) = find_feature(map, &nc.feature) {
                let val = read_feature_value(feat, core.wram());
                if val == nc.equals {
                    let off = feat.offset.0 as usize;
                    let width = feat.feature_type.derived_width().unwrap_or(1) as usize;
                    let raw: Vec<u8> = (0..width)
                        .map(|i| core.wram().get(off + i).copied().unwrap_or(0))
                        .collect();
                    return Ok(MapCheckResult::Failure {
                        frame: frame_no,
                        feature: nc.feature.clone(),
                        expected_description: format!("never equals {}", nc.equals),
                        actual: val,
                        raw_bytes: raw,
                    });
                }
            }
        }

        // ── Check at_frame assertions ────────────────────────────────────────
        let mut remaining: Vec<(usize, &Assertion)> = Vec::new();
        for (idx, a) in pending.drain(..) {
            let deadline = a.deadline();
            let fire_now = match (a.at_frame, a.by_frame) {
                (Some(af), None) => frame_no == af,
                (None, Some(bf)) => frame_no >= bf,
                _ => false,
            };

            if fire_now {
                let feat = find_feature(map, &a.feature).unwrap();

                // valid_when check: false valid_when at assertion time is an
                // expectations-file error (exit 2).
                if !is_valid(map, feat, core.wram()) {
                    return Ok(MapCheckResult::ExpectationsError(format!(
                        "assertion #{} ({:?}) at frame {}: valid_when is false — \
                         asserting on an invalid feature is an error in the \
                         expectations file (exit 2)",
                        idx, a.feature, frame_no
                    )));
                }

                let cur = read_feature_value(feat, core.wram());
                let prev = *prev_values.get(&a.feature).unwrap_or(&cur);

                let off = feat.offset.0 as usize;
                let width = feat.feature_type.derived_width().unwrap_or(1) as usize;
                let raw: Vec<u8> = (0..width)
                    .map(|i| core.wram().get(off + i).copied().unwrap_or(0))
                    .collect();

                let (ok, desc) = if let Some(exp) = a.equals {
                    (cur == exp, format!("equals {}", exp))
                } else if let Some(ct) = a.changes_to {
                    (cur != prev && cur == ct, format!("changes_to {}", ct))
                } else if let Some(d) = a.delta {
                    (cur.wrapping_sub(prev) == d, format!("delta {}", d))
                } else {
                    unreachable!("validate() already enforced exactly one condition")
                };

                if !ok {
                    return Ok(MapCheckResult::Failure {
                        frame: frame_no,
                        feature: a.feature.clone(),
                        expected_description: desc,
                        actual: cur,
                        raw_bytes: raw,
                    });
                }
                // Assertion passed; update prev.
                prev_values.insert(a.feature.clone(), cur);
            } else if frame_no > deadline {
                // by_frame deadline exceeded without firing (shouldn't happen
                // for by_frame, but at_frame missed means the frame has passed).
                let feat = find_feature(map, &a.feature).unwrap();
                let cur = read_feature_value(feat, core.wram());
                return Ok(MapCheckResult::Failure {
                    frame: frame_no,
                    feature: a.feature.clone(),
                    expected_description: format!(
                        "assertion deadline {} exceeded (by_frame/at_frame)",
                        deadline
                    ),
                    actual: cur,
                    raw_bytes: Vec::new(),
                });
            } else {
                remaining.push((idx, a));
            }
        }
        pending = remaining;
    }

    // Any un-fired assertions are failures.
    if let Some((_, a)) = pending.first() {
        let feat = find_feature(map, &a.feature).unwrap();
        let cur = read_feature_value(feat, core.wram());
        return Ok(MapCheckResult::Failure {
            frame: core.frame_counter(),
            feature: a.feature.clone(),
            expected_description: format!(
                "assertion not satisfied by end of run (deadline {})",
                a.deadline()
            ),
            actual: cur,
            raw_bytes: Vec::new(),
        });
    }

    Ok(MapCheckResult::Pass)
}
