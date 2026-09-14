//! `refwork-verify host-capture-index` — host-side Phase 4 capture-index writer.
//!
//! Replays a `.padlog` on the host emulator core (no worker) and writes a
//! Phase-4-style capture index (JSONL rows + packed feature-byte blobs) that
//! `refwork-verify trace` ([`crate::phase4_trace`]) can consume. This is the
//! host-side replacement for [`crate::phase4_capture_export`], which requires
//! a live worker connection.
//!
//! Reuses the layout/feature-map validation and byte-packing helpers from
//! `phase4_capture_export` so the two capture paths can never drift on what
//! counts as a valid layout or how packed feature bytes are decoded.

use crate::phase4_capture_export::{
    atomic_write, decode_packed, hash, packed_width, read_json, validate_layout, Layout,
};
use refwork_emu::{Cartridge, Core, RegionBuffers, WRAM_INIT_BYTE};
use refwork_featuremap::parse_feature_map;
use refwork_script::parse as parse_padlog;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

/// Options for [`write_host_capture_index`].
#[derive(Debug, Clone)]
pub struct HostIndexOptions {
    pub rom: PathBuf,
    pub script: PathBuf,
    pub map: PathBuf,
    pub layout: PathBuf,
    pub out_dir: PathBuf,
    pub every: u64,
    /// First frame index eligible for a row (skips boot-era frames whose WRAM
    /// still holds the emulator's init pattern); `--every` cadence is absolute.
    pub start_frame: u64,
    pub frames: Option<u64>,
    pub source_ref: String,
    pub session_name: String,
    pub marks: Vec<(u64, String)>,
    pub report: PathBuf,
}

/// JSON report produced by [`write_host_capture_index`].
#[derive(Serialize, Clone, Debug, Default)]
pub struct HostIndexReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub capture_count: usize,
    pub every: u64,
    pub start_frame: u64,
    pub frames_run: u64,
    pub first_frame: Option<u64>,
    pub last_frame: Option<u64>,
    pub map_hash: Option<String>,
    pub layout_hash: Option<String>,
    pub source_ref: String,
    pub session_name: String,
    pub index_hash: Option<String>,
    pub faults: Vec<String>,
    pub errors: Vec<String>,
}

impl HostIndexReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Run the host-capture-index operation: pre-flight validation, then replay
/// the padlog on the host core, writing one row (and one feature-bytes blob)
/// per `--every`/`--mark` frame.
pub fn write_host_capture_index(opts: &HostIndexOptions) -> HostIndexReport {
    let mut report = HostIndexReport {
        schema_version: 1,
        command: "refwork-verify host-capture-index <private arguments redacted>".into(),
        status: "fail".into(),
        every: opts.every,
        start_frame: opts.start_frame,
        source_ref: opts.source_ref.clone(),
        session_name: opts.session_name.clone(),
        ..Default::default()
    };

    // ── 1. Argument pre-flight ──────────────────────────────────────────
    if opts.every == 0 {
        report
            .errors
            .push("--every must be greater than zero".into());
        return finish(opts, report);
    }
    if !valid_session_name(&opts.session_name) {
        report
            .errors
            .push("--session-name must match ^[a-z0-9-]+$".into());
        return finish(opts, report);
    }
    {
        let mut seen = BTreeSet::new();
        for (frame, _) in &opts.marks {
            if !seen.insert(*frame) {
                report.errors.push(format!("duplicate mark frame {frame}"));
                return finish(opts, report);
            }
        }
    }

    // ── 2. Layout ────────────────────────────────────────────────────────
    let layout: Layout = match read_json(&opts.layout) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e);
            return finish(opts, report);
        }
    };
    if let Err(e) = validate_layout(&layout) {
        report.errors.push(e);
        return finish(opts, report);
    }
    report.layout_hash = Some(layout.blake3.clone());

    // ── 3. Feature map ──────────────────────────────────────────────────
    let map_text = match fs::read_to_string(&opts.map) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("cannot read feature map: {e}"));
            return finish(opts, report);
        }
    };
    let map_hash = hash(map_text.as_bytes());
    report.map_hash = Some(map_hash.clone());
    let (map, map_errors) = match parse_feature_map(&map_text) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("feature-map parse failed: {e}"));
            return finish(opts, report);
        }
    };
    if !map_errors.is_empty() {
        report.errors.extend(
            map_errors
                .into_iter()
                .map(|e| format!("feature-map validation: {e}")),
        );
        return finish(opts, report);
    }
    if layout.compiled_from_feature_map_hash != map_hash {
        report
            .errors
            .push("layout feature-map hash mismatch".into());
        return finish(opts, report);
    }
    if packed_width(&map) != Some(layout.total_len as usize)
        || map.features.len() != layout.ranges.len()
    {
        report
            .errors
            .push("feature-map packing does not agree with layout".into());
        return finish(opts, report);
    }
    for (feature, range) in map.features.iter().zip(&layout.ranges) {
        let width = feature
            .feature_type
            .derived_width()
            .or(feature.width)
            .unwrap() as u64;
        if feature.region != range.region
            || feature.offset.0 < 0
            || feature.offset.0 as u64 != range.offset
            || width != range.len
            || range.layout_version != 1
        {
            report
                .errors
                .push("feature-map range order does not exactly agree with layout".into());
            return finish(opts, report);
        }
    }
    if map.features.iter().any(|f| f.region != "wram") {
        report
            .errors
            .push("only wram region is supported on the host".into());
        return finish(opts, report);
    }

    // ── 4. ROM + padlog + core ──────────────────────────────────────────
    let rom = match fs::read(&opts.rom) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("cannot read rom: {e}"));
            return finish(opts, report);
        }
    };
    let script_text = match fs::read_to_string(&opts.script) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("cannot read script: {e}"));
            return finish(opts, report);
        }
    };
    let script = match parse_padlog(&script_text) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("padlog parse failed: {e}"));
            return finish(opts, report);
        }
    };
    let cart = match Cartridge::from_rom(rom, None) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("bad ROM: {e:?}"));
            return finish(opts, report);
        }
    };
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core = match Core::new(cart, regions) {
        Ok(v) => v,
        Err(e) => {
            report
                .errors
                .push(format!("core construction failed: {e:?}"));
            return finish(opts, report);
        }
    };

    let total_frames = opts.frames.unwrap_or(script.len() as u64);
    let last_pad: u16 = script.frames.last().copied().unwrap_or(0);
    let marks: BTreeMap<u64, String> = opts.marks.iter().cloned().collect();

    // Pre-flight passed: only now do we touch the output directory.
    let artifacts_dir = opts.out_dir.join("artifacts/feature-bytes");
    if let Err(e) = fs::create_dir_all(&artifacts_dir) {
        report
            .errors
            .push(format!("cannot create output directories: {e}"));
        return finish(opts, report);
    }

    // ── 5. Replay loop ───────────────────────────────────────────────────
    let mut rows: Vec<String> = Vec::new();
    'frames: for f in 0..total_frames {
        let pad = if (f as usize) < script.frames.len() {
            script.frames[f as usize]
        } else {
            last_pad
        };
        core.run_one_frame(pad);

        if let Some(fault) = core.fault() {
            let msg = format!("fault at frame {f}: {fault:?}");
            report.faults.push(msg.clone());
            report.errors.push(msg);
            break 'frames;
        }

        if f < opts.start_frame {
            continue;
        }
        let mark_label = marks.get(&f);
        let is_every = f % opts.every == 0;
        if !is_every && mark_label.is_none() {
            continue;
        }

        let capture_id = format!("host-{}-{:07}", opts.session_name, f);
        let mut bytes = Vec::with_capacity(layout.total_len as usize);
        for feature in &map.features {
            let width = feature
                .feature_type
                .derived_width()
                .or(feature.width)
                .unwrap() as usize;
            let off = feature.offset.0 as usize;
            let slice = match core.wram().get(off..off + width) {
                Some(s) => s,
                None => {
                    report.errors.push(format!(
                        "feature {} range exceeds wram bounds",
                        feature.name
                    ));
                    break 'frames;
                }
            };
            bytes.extend_from_slice(slice);
        }
        let decoded = match decode_packed(&map, &bytes) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(e);
                break 'frames;
            }
        };

        let blob_path = artifacts_dir.join(format!("{capture_id}.bin"));
        let tmp_path = blob_path.with_extension("tmp");
        // Deterministic overwrite: `atomic_write` uses `create_new` on the
        // `.tmp` staging file, so a stale tmp or a blob left by a prior run
        // (e.g. a rerun into the same out dir) must be cleared first.
        let _ = fs::remove_file(&tmp_path);
        let _ = fs::remove_file(&blob_path);
        if let Err(e) = atomic_write(&blob_path, &bytes) {
            report
                .errors
                .push(format!("cannot write feature-bytes blob: {e}"));
            break 'frames;
        }
        let blob_hash = hash(&bytes);
        let feature_ref = format!("artifacts/feature-bytes/{capture_id}.bin");
        let decoded_order: Vec<String> = map.features.iter().map(|f| f.name.clone()).collect();
        let mut row = serde_json::json!({
            "schema_version": 1,
            "kind": "host-capture-row",
            "capture_id": capture_id,
            "frame_index": f,
            "source_ref": opts.source_ref,
            "map_hash": map_hash,
            "layout_hash": layout.blake3,
            "packed_width": layout.total_len,
            "feature_bytes": {
                "ref": feature_ref,
                "len": bytes.len(),
                "blake3": blob_hash,
            },
            "decoded_order": decoded_order,
            "decoded_values": decoded,
        });
        if let Some(label) = mark_label {
            row["label"] = serde_json::Value::String(label.clone());
        }
        rows.push(serde_json::to_string(&row).unwrap());
        report.capture_count += 1;
        report.first_frame.get_or_insert(f);
        report.last_frame = Some(f);
    }
    report.frames_run = core.frame_counter();

    // ── 6. Write the index ───────────────────────────────────────────────
    if report.errors.is_empty() {
        let mut out = String::new();
        for row in &rows {
            out.push_str(row);
            out.push('\n');
        }
        let index_path = opts.out_dir.join("index.jsonl");
        let tmp_path = index_path.with_extension("tmp");
        let _ = fs::remove_file(&tmp_path);
        let _ = fs::remove_file(&index_path);
        match atomic_write(&index_path, out.as_bytes()) {
            Ok(()) => report.index_hash = Some(hash(out.as_bytes())),
            Err(e) => report.errors.push(format!("cannot write index.jsonl: {e}")),
        }
    }

    finish(opts, report)
}

fn finish(opts: &HostIndexOptions, mut report: HostIndexReport) -> HostIndexReport {
    report.status = if report.errors.is_empty() {
        "pass"
    } else {
        "fail"
    }
    .into();
    if let Some(parent) = opts.report.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            report
                .errors
                .push(format!("cannot create report directory: {e}"));
            report.status = "fail".into();
        }
    }
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            if let Err(e) = fs::write(&opts.report, json) {
                report.errors.push(format!(
                    "cannot write report {}: {e}",
                    opts.report.display()
                ));
                report.status = "fail".into();
            }
        }
        Err(e) => {
            report
                .errors
                .push(format!("report serialization failed: {e}"));
            report.status = "fail".into();
        }
    }
    report
}

fn valid_session_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
