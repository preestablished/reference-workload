//! Integration tests for `refwork-verify host-capture-index`.
//!
//! Uses the synthetic ROM built by `xtask::build_synth_rom()`. No game
//! content is involved.
//!
//! # Frame-counter ground truth
//!
//! The synthetic ROM's WRAM frame counter (`$0010`/`$0011`, u16le) is
//! initialized to zero by the reset epilogue and incremented once per frame
//! by the NMI handler's main loop — but the reset sequence itself spans
//! (rather than precedes) the *first* `run_one_frame` call, so at
//! `frame_index == 0` the counter has not yet been zeroed and the row reads
//! back a fixed pre-init artifact instead of a small counter value. From
//! `frame_index >= 1` onward the relationship is a clean identity:
//! `frame_ctr == frame_index`. This was verified empirically against this
//! ROM/core pair (see the crate's existing `map_check_positive` test in
//! `tests/integration.rs`, which independently asserts `frame_ctr == 59` at
//! `core.frame_counter() == 60`, i.e. at loop index 59 — the same identity).

use refwork_verify::host_capture_index::{write_host_capture_index, HostIndexOptions};
use refwork_verify::phase4_layout::{write_phase4_layout, LayoutOptions};
use refwork_verify::phase4_trace::{emit_phase4_trace, TraceOptions};
use refwork_verify::play::synth_pad;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const SYNTH_MAP_YAML: &str = r#"
schema_version: 1
kind: feature-map
meta:
  name: synth-test
  workload: refwork-synth
  game_revision: "test"
  version: 1
regions:
  - name: wram
    size: 131072
features:
  - name: frame_ctr
    region: wram
    offset: 0x0010
    type: u16le
    semantics: counter
    stability: stable
"#;

/// The deterministic WRAM value at `frame_index == 0` (see module doc):
/// the reset epilogue has not yet zeroed `$0010/$0011`, so the row reads
/// back a fixed pre-init artifact rather than a small counter value.
const FRAME_ZERO_ARTIFACT: i64 = 21846;

fn write_padlog(path: &Path, frames: usize) {
    let mut text = String::from("padlog v1\n");
    for f in 0..frames {
        text.push_str(&format!("{:04x}\n", synth_pad(f)));
    }
    fs::write(path, text).unwrap();
}

struct Fixture {
    _root: tempfile::TempDir,
    rom: PathBuf,
    script: PathBuf,
    map: PathBuf,
    layout: PathBuf,
}

fn setup(frames: usize) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let rom = root.path().join("synth.rom");
    fs::write(&rom, xtask::build_synth_rom()).unwrap();
    let script = root.path().join("input.padlog");
    write_padlog(&script, frames);
    let map = root.path().join("map.yaml");
    fs::write(&map, SYNTH_MAP_YAML).unwrap();
    let layout = root.path().join("layout.json");
    let layout_report = write_phase4_layout(&LayoutOptions {
        map: map.clone(),
        out: layout.clone(),
        capture_spec_hash: format!("blake3:{}", "a".repeat(64)),
        layout_version: 1,
        compiler_or_exporter_commit: "0123456789012345678901234567890123456789".into(),
    });
    assert!(layout_report.passed(), "{:?}", layout_report.errors);
    Fixture {
        _root: root,
        rom,
        script,
        map,
        layout,
    }
}

fn opts(fx: &Fixture, out_dir: PathBuf) -> HostIndexOptions {
    HostIndexOptions {
        rom: fx.rom.clone(),
        script: fx.script.clone(),
        map: fx.map.clone(),
        layout: fx.layout.clone(),
        out_dir: out_dir.clone(),
        every: 5,
        start_frame: 0,
        frames: Some(12),
        source_ref: "test:synth".into(),
        session_name: "synth".into(),
        marks: vec![(7, "probe".into())],
        report: out_dir.join("report.json"),
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

#[test]
fn host_capture_index_emits_rows_at_every_and_mark_frames() {
    let fx = setup(12);
    let out_dir = fx._root.path().join("out");
    let report = write_host_capture_index(&opts(&fx, out_dir.clone()));
    assert!(report.passed(), "{:?}", report.errors);
    assert_eq!(report.capture_count, 4);
    assert_eq!(report.frames_run, 12);

    let map_bytes = fs::read(&fx.map).unwrap();
    let expected_map_hash = hash_bytes(&map_bytes);
    assert_eq!(report.map_hash.as_deref(), Some(expected_map_hash.as_str()));

    let layout_json: Value =
        serde_json::from_str(&fs::read_to_string(&fx.layout).unwrap()).unwrap();
    let expected_layout_hash = layout_json["blake3"].as_str().unwrap().to_owned();
    assert_eq!(
        report.layout_hash.as_deref(),
        Some(expected_layout_hash.as_str())
    );

    let index_text = fs::read_to_string(out_dir.join("index.jsonl")).unwrap();
    let rows: Vec<Value> = index_text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 4);

    let expected_frames = [0u64, 5, 7, 10];
    let expected_values = [FRAME_ZERO_ARTIFACT, 5, 7, 10];
    for (i, (row, (&frame, &value))) in rows
        .iter()
        .zip(expected_frames.iter().zip(expected_values.iter()))
        .enumerate()
    {
        assert_eq!(row["frame_index"].as_u64(), Some(frame), "row {i}");
        assert_eq!(
            row["decoded_order"],
            serde_json::json!(["frame_ctr"]),
            "row {i}"
        );
        assert_eq!(row["decoded_values"], serde_json::json!([value]), "row {i}");
        assert_eq!(
            row["map_hash"].as_str(),
            Some(expected_map_hash.as_str()),
            "row {i}"
        );
        assert_eq!(
            row["layout_hash"].as_str(),
            Some(expected_layout_hash.as_str()),
            "row {i}"
        );
        assert_eq!(row["packed_width"].as_u64(), Some(2), "row {i}");

        let fb = &row["feature_bytes"];
        assert_eq!(fb["len"].as_u64(), Some(2), "row {i}");
        let blob_ref = fb["ref"].as_str().unwrap();
        let blob_bytes = fs::read(out_dir.join(blob_ref)).unwrap();
        assert_eq!(
            fb["blake3"].as_str(),
            Some(hash_bytes(&blob_bytes).as_str())
        );

        if frame == 7 {
            assert_eq!(row["label"].as_str(), Some("probe"), "row {i}");
        } else {
            assert!(row.get("label").is_none(), "row {i} unexpected label");
        }
    }
}

#[test]
fn host_capture_index_is_deterministic_across_runs() {
    let fx = setup(12);
    let out_a = fx._root.path().join("out-a");
    let out_b = fx._root.path().join("out-b");
    let report_a = write_host_capture_index(&opts(&fx, out_a.clone()));
    let report_b = write_host_capture_index(&opts(&fx, out_b.clone()));
    assert!(report_a.passed(), "{:?}", report_a.errors);
    assert!(report_b.passed(), "{:?}", report_b.errors);

    let index_a = fs::read(out_a.join("index.jsonl")).unwrap();
    let index_b = fs::read(out_b.join("index.jsonl")).unwrap();
    assert_eq!(index_a, index_b, "index.jsonl must be byte-identical");
    assert_eq!(report_a.index_hash, report_b.index_hash);

    for frame in [0u64, 5, 7, 10] {
        let name = format!("host-synth-{:07}.bin", frame);
        let blob_a = fs::read(out_a.join("artifacts/feature-bytes").join(&name)).unwrap();
        let blob_b = fs::read(out_b.join("artifacts/feature-bytes").join(&name)).unwrap();
        assert_eq!(blob_a, blob_b, "blob {name} must be byte-identical");
    }
}

#[test]
fn host_capture_index_rejects_layout_hash_drift() {
    let fx = setup(12);
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&fx.layout).unwrap()).unwrap();
    v["blake3"] = Value::String(
        "blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into(),
    );
    fs::write(&fx.layout, serde_json::to_vec(&v).unwrap()).unwrap();

    let out_dir = fx._root.path().join("out");
    let report = write_host_capture_index(&opts(&fx, out_dir.clone()));
    assert!(!report.passed());
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.contains("layout hash mismatch")),
        "{:?}",
        report.errors
    );
    assert!(!out_dir.join("index.jsonl").exists());
}

#[test]
fn host_capture_index_rejects_zero_every() {
    let fx = setup(12);
    let out_dir = fx._root.path().join("out");
    let mut o = opts(&fx, out_dir.clone());
    o.every = 0;
    let report = write_host_capture_index(&o);
    assert!(!report.passed());
    assert!(!out_dir.join("index.jsonl").exists());
}

const SCORING_YAML: &str = r#"
schema_version: 1
kind: scoring-program
meta:
  name: synth-test
  feature_map: synth-test
  version: 1
stages:
  monotone: true
  list:
    - name: first_boss
      points: 10
      when:
        all:
          - {feature: frame_ctr, op: ge, value: 6}
          - {feature: frame_ctr, op: le, value: 1000}
goal:
  name: g
  predicate: {feature: frame_ctr, op: eq, value: 10}
"#;

#[test]
fn host_capture_index_output_satisfies_trace_contract() {
    let fx = setup(12);
    let out_dir = fx._root.path().join("out");
    let report = write_host_capture_index(&opts(&fx, out_dir.clone()));
    assert!(report.passed(), "{:?}", report.errors);

    let scoring_path = fx._root.path().join("scoring.yaml");
    fs::write(&scoring_path, SCORING_YAML).unwrap();

    // Computed against SCORING_YAML's thresholds and the real per-frame
    // frame_ctr values (see module doc for the frame_index==0 artifact):
    // frame 0 -> 21846 (root, goal false), frame 5 -> 5 (root, goal false),
    // frame 7 -> 7 (first_boss, goal false), frame 10 -> 10 (first_boss,
    // goal true).
    let labels_yaml = r#"
kind: phase4-trace-labels
schema_version: 1
labels:
  - capture_id: host-synth-0000000
    expected_highest_stage: root
    prune: false
    goal: false
    first_boss_coverage: false
  - capture_id: host-synth-0000005
    expected_highest_stage: root
    prune: false
    goal: false
    first_boss_coverage: false
  - capture_id: host-synth-0000007
    expected_highest_stage: first_boss
    prune: false
    goal: false
    first_boss_coverage: true
  - capture_id: host-synth-0000010
    expected_highest_stage: first_boss
    prune: false
    goal: true
    first_boss_coverage: true
"#;
    let labels_path = fx._root.path().join("labels.yaml");
    fs::write(&labels_path, labels_yaml).unwrap();

    let trace_out = out_dir.join("trace.jsonl");
    let trace_report_path = out_dir.join("trace-report.json");
    let trace_report = emit_phase4_trace(&TraceOptions {
        captures: out_dir.join("index.jsonl"),
        map: fx.map.clone(),
        scoring: scoring_path,
        labels: labels_path,
        out: trace_out,
        report: trace_report_path,
    });
    assert!(trace_report.passed(), "{:?}", trace_report.errors);
    assert_eq!(trace_report.capture_count, 4);
}

#[test]
fn host_capture_index_start_frame_skips_boot_era_rows() {
    let fx = setup(12);
    let out_dir = fx._root.path().join("out-start");
    let mut o = opts(&fx, out_dir.clone());
    o.start_frame = 6;
    let report = write_host_capture_index(&o);
    assert!(report.passed(), "{:?}", report.errors);
    assert_eq!(report.start_frame, 6);
    let text = std::fs::read_to_string(out_dir.join("index.jsonl")).unwrap();
    let frames: Vec<u64> = text
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["frame_index"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(
        frames,
        vec![7, 10],
        "rows before start_frame must be skipped; cadence stays absolute"
    );
    assert_eq!(report.capture_count, 2);
}

#[test]
fn host_capture_index_rejects_marks_outside_the_replay_window() {
    let fx = setup(12);
    for (marks, start) in [
        (vec![(3u64, "early".to_string())], 6u64),
        (vec![(12, "late".to_string())], 0),
    ] {
        let out_dir = fx._root.path().join(format!("out-mark-{start}"));
        let mut o = opts(&fx, out_dir.clone());
        o.marks = marks;
        o.start_frame = start;
        let report = write_host_capture_index(&o);
        assert!(!report.passed());
        assert!(
            report
                .errors
                .iter()
                .any(|e| e.contains("outside the replay window")),
            "{:?}",
            report.errors
        );
        assert!(!out_dir.join("index.jsonl").exists());
    }
    // a valid mark is counted
    let out_dir = fx._root.path().join("out-mark-ok");
    let report = write_host_capture_index(&opts(&fx, out_dir));
    assert!(report.passed());
    assert_eq!((report.marks_requested, report.marks_emitted), (1, 1));
    assert_eq!(report.frames_requested, 12);
}

#[test]
fn host_capture_index_failed_rerun_removes_stale_outputs() {
    let fx = setup(12);
    let out_dir = fx._root.path().join("out-stale");
    let good = write_host_capture_index(&opts(&fx, out_dir.clone()));
    assert!(good.passed());
    assert!(out_dir.join("index.jsonl").exists());
    let mut bad = opts(&fx, out_dir.clone());
    bad.every = 0; // pre-flight failure
    let report = write_host_capture_index(&bad);
    assert!(!report.passed());
    assert!(
        !out_dir.join("index.jsonl").exists(),
        "stale index must not survive a failed rerun"
    );
    assert!(
        !out_dir.join("artifacts/feature-bytes").exists(),
        "stale blobs must not survive a failed rerun"
    );
    assert!(out_dir.join("report.json").exists());
}
