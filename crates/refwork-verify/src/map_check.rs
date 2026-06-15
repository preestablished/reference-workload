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
    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb: Box<[u8; FB_BYTES]> = Box::new([0u8; FB_BYTES]);
    let mut chain = [0u8; 32];

    // Previous values for delta and changes_to assertions.
    let mut prev_values: std::collections::BTreeMap<String, FeatureValue> =
        std::collections::BTreeMap::new();

    // Track which assertions still need to fire. The bool records whether
    // the feature's `valid_when` has been true at any evaluated frame (used
    // to distinguish an expectations error from a plain by_frame failure).
    let mut pending: Vec<(usize, &Assertion, bool)> = expectations
        .assertions
        .iter()
        .enumerate()
        .map(|(i, a)| (i, a, false))
        .collect();

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
        let fh = frame_hash(core.wram(), &fb[..]);
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

        // ── Check assertions ─────────────────────────────────────────────────
        //
        // at_frame: evaluated exactly once, at that frame; `valid_when`
        //   false there is an error in the expectations file (exit 2).
        // by_frame: evaluated at EVERY frame up to and including the
        //   deadline — it passes as soon as the condition holds once.
        //   Frames where `valid_when` is false are skipped; if the deadline
        //   passes without the feature ever being valid, that is an
        //   expectations error, otherwise a plain assertion failure.
        let mut remaining: Vec<(usize, &Assertion, bool)> = Vec::new();
        for (idx, a, mut ever_valid) in pending.drain(..) {
            let deadline = a.deadline();
            let is_at = a.at_frame.is_some();
            let evaluate_now = if is_at {
                frame_no == a.at_frame.unwrap()
            } else {
                frame_no <= deadline
            };

            if evaluate_now {
                let feat = find_feature(map, &a.feature).unwrap();

                if !is_valid(map, feat, core.wram()) {
                    if is_at {
                        // valid_when false at the asserted frame is an
                        // expectations-file error (exit 2).
                        return Ok(MapCheckResult::ExpectationsError(format!(
                            "assertion #{} ({:?}) at frame {}: valid_when is false — \
                             asserting on an invalid feature is an error in the \
                             expectations file (exit 2)",
                            idx, a.feature, frame_no
                        )));
                    }
                    // by_frame: skip invalid frames; the deadline check below
                    // decides the outcome if it never becomes valid.
                    if frame_no < deadline {
                        remaining.push((idx, a, ever_valid));
                        continue;
                    }
                    if !ever_valid {
                        return Ok(MapCheckResult::ExpectationsError(format!(
                            "assertion #{} ({:?}): valid_when never true through \
                             by_frame deadline {} — error in the expectations file \
                             (exit 2)",
                            idx, a.feature, deadline
                        )));
                    }
                }
                ever_valid = true;

                let cur = read_feature_value(feat, core.wram());
                let prev = *prev_values.get(&a.feature).unwrap_or(&cur);

                let (ok, desc) = if let Some(exp) = a.equals {
                    (cur == exp, format!("equals {}", exp))
                } else if let Some(ct) = a.changes_to {
                    (cur != prev && cur == ct, format!("changes_to {}", ct))
                } else if let Some(d) = a.delta {
                    (cur.wrapping_sub(prev) == d, format!("delta {}", d))
                } else {
                    unreachable!("validate() already enforced exactly one condition")
                };

                if ok {
                    // Satisfied — drop from pending.
                    continue;
                }
                if !is_at && frame_no < deadline {
                    // by_frame: not met yet; keep waiting.
                    remaining.push((idx, a, ever_valid));
                    continue;
                }
                // at_frame miss, or by_frame deadline reached unmet.
                let off = feat.offset.0 as usize;
                let width = feat.feature_type.derived_width().unwrap_or(1) as usize;
                let raw: Vec<u8> = (0..width)
                    .map(|i| core.wram().get(off + i).copied().unwrap_or(0))
                    .collect();
                return Ok(MapCheckResult::Failure {
                    frame: frame_no,
                    feature: a.feature.clone(),
                    expected_description: if is_at {
                        desc
                    } else {
                        format!("{} by frame {}", desc, deadline)
                    },
                    actual: cur,
                    raw_bytes: raw,
                });
            } else if frame_no > deadline {
                // at_frame already passed without firing (frames_override can
                // start past it) — report the miss.
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
                remaining.push((idx, a, ever_valid));
            }
        }
        pending = remaining;

        // Record this frame's value for every asserted feature so next
        // frame's `changes_to`/`delta` compare against the true previous
        // value rather than a stale pass-time snapshot.
        for a in &expectations.assertions {
            if let Some(feat) = find_feature(map, &a.feature) {
                let v = read_feature_value(feat, core.wram());
                prev_values.insert(a.feature.clone(), v);
            }
        }
    }

    // Any un-fired assertions are failures.
    if let Some((_, a, _)) = pending.first() {
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
