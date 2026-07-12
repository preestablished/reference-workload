//! Frame-coherent Phase 4 capture exporter.

use refwork_dh_client::{decompress_fb_lz4, proto, WorkerEndpoint, WorkerSession};
use refwork_featuremap::{parse_feature_map, FeatureMap, FeatureType};
use refwork_script::parse as parse_padlog;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const FB_LEN: usize = 229_376;

#[derive(Debug, Clone)]
pub struct CaptureExportOptions {
    pub endpoint: String,
    pub snapshot_hash: String,
    pub padlog: PathBuf,
    pub map: PathBuf,
    pub layout: PathBuf,
    pub bundle: PathBuf,
    pub count: u32,
    pub cadence: u32,
    pub hard_icount_cap: u64,
    pub production: bool,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CaptureExportReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub requested: u32,
    pub completed: u32,
    pub resumed: u32,
    pub layout_hash: Option<String>,
    pub feature_map_hash: Option<String>,
    pub first_frame: Option<u32>,
    pub last_frame: Option<u32>,
    pub cadence: u32,
    pub hard_icount_cap: u64,
    pub source_ref: String,
    pub compiler_or_exporter_commit: Option<String>,
    pub worker_version: Option<String>,
    pub worker_build_profile: Option<String>,
    pub errors: Vec<String>,
}
impl CaptureExportReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty() && self.completed == self.requested
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Layout {
    ranges: Vec<LayoutRange>,
    total_len: u64,
    blake3: String,
    compiled_from_feature_map_hash: String,
    capture_spec_hash: String,
    compiler_or_exporter_commit: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LayoutRange {
    region: String,
    layout_version: u64,
    offset: u64,
    len: u64,
}

pub fn export_phase4_captures(opts: &CaptureExportOptions) -> CaptureExportReport {
    let mut report = CaptureExportReport {
        schema_version: 1,
        command: "refwork-verify phase4-capture-export <private arguments redacted>".into(),
        status: "fail".into(),
        requested: opts.count,
        cadence: opts.cadence,
        hard_icount_cap: opts.hard_icount_cap,
        source_ref: opts.source_ref.clone(),
        ..Default::default()
    };
    if opts.count == 0 || opts.cadence == 0 || opts.hard_icount_cap == 0 {
        report
            .errors
            .push("count, cadence, and hard instruction cap must be positive".into());
        return report;
    }
    let layout: Layout = match read_json(&opts.layout) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    if let Err(e) = validate_layout(&layout) {
        report.errors.push(e);
        return report;
    }
    report.layout_hash = Some(layout.blake3.clone());
    report.compiler_or_exporter_commit = Some(layout.compiler_or_exporter_commit.clone());
    let map_text = match fs::read_to_string(&opts.map) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("cannot read feature map: {e}"));
            return report;
        }
    };
    let map_hash = hash(map_text.as_bytes());
    report.feature_map_hash = Some(map_hash.clone());
    let (map, map_errors) = match parse_feature_map(&map_text) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("feature-map parse failed: {e}"));
            return report;
        }
    };
    if !map_errors.is_empty() {
        report.errors.extend(
            map_errors
                .into_iter()
                .map(|e| format!("feature-map validation: {e}")),
        );
        return report;
    }
    if layout.compiled_from_feature_map_hash != map_hash {
        report
            .errors
            .push("layout feature-map hash mismatch".into());
        return report;
    }
    if opts.production
        && (map.meta.workload.to_ascii_lowercase().contains("demo")
            || map
                .meta
                .game_revision
                .to_ascii_lowercase()
                .contains("synthetic"))
    {
        report
            .errors
            .push("production export rejects demo/synthetic feature-map provenance".into());
        return report;
    }
    if packed_width(&map) != Some(layout.total_len as usize)
        || map.features.len() != layout.ranges.len()
    {
        report
            .errors
            .push("feature-map packing does not agree with layout".into());
        return report;
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
            return report;
        }
    }
    let pad_text = match fs::read_to_string(&opts.padlog) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("cannot read padlog: {e}"));
            return report;
        }
    };
    let padlog = match parse_padlog(&pad_text) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("padlog parse failed: {e}"));
            return report;
        }
    };
    let snapshot = match parse_hash(&opts.snapshot_hash) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    if let Err(e) = fs::create_dir_all(opts.bundle.join("artifacts/feature-bytes"))
        .and_then(|_| fs::create_dir_all(opts.bundle.join("artifacts/framebuffer")))
        .and_then(|_| fs::create_dir_all(opts.bundle.join("captures")))
    {
        report
            .errors
            .push(format!("cannot create bundle directories: {e}"));
        return report;
    }
    let index_path = opts.bundle.join("captures/index.jsonl");
    let existing = match validate_resume(&index_path, opts, &layout) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    report.resumed = existing as u32;
    if existing > 0 {
        let checked = crate::phase4_artifact_check::check_phase4_artifacts(&opts.bundle);
        if !checked.passed() || checked.capture_count != existing {
            report
                .errors
                .push("existing capture artifacts failed resume verification".into());
            return report;
        }
    }
    if existing >= opts.count as usize {
        report.completed = opts.count;
        report.status = "pass".into();
        return report;
    }
    let mut session = match WorkerSession::connect(&WorkerEndpoint::parse(&opts.endpoint)) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("worker connection failed: {e}"));
            return report;
        }
    };
    match session.worker_info() {
        Ok(info) => {
            report.worker_version = Some(info.version);
            report.worker_build_profile = Some(info.build_profile)
        }
        Err(e) => {
            report
                .errors
                .push(format!("worker provenance query failed: {e}"));
            return report;
        }
    }
    let restore = match session.restore_snapshot(snapshot, vec![]) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("snapshot restore failed: {e}"));
            return report;
        }
    };
    let lease = restore.lease.unwrap();
    if existing > 0 {
        if let Err(e) = validate_resume_frames(&index_path, restore.frame_counter, opts.cadence) {
            report.errors.push(e);
            let _ = session.destroy_vm(lease);
            return report;
        }
    }
    let events = padlog
        .frames
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < (opts.count * opts.cadence) as usize)
        .map(|(i, word)| proto::ScheduledEvent {
            at: Some(proto::scheduled_event::At::AtFrame(
                restore.frame_counter + 1 + i as u32,
            )),
            event: Some(proto::scheduled_event::Event::PadSet(proto::PadSet {
                port: 0,
                buttons: *word as u32,
            })),
        })
        .collect();
    if let Err(e) = session.inject_inputs(lease.clone(), events) {
        report.errors.push(format!("input injection failed: {e}"));
        let _ = session.destroy_vm(lease);
        return report;
    }
    let spec = proto::CaptureSpec {
        framebuffer: true,
        ranges: layout
            .ranges
            .iter()
            .map(|r| proto::ExtractRange {
                region: r.region.clone(),
                layout_version: r.layout_version as u32,
                offset: r.offset,
                len: r.len as u32,
            })
            .collect(),
    };
    if existing > 0 {
        if let Err(e) = session.run_frames(
            lease.clone(),
            existing as u32 * opts.cadence,
            None,
            opts.hard_icount_cap,
        ) {
            report
                .errors
                .push(format!("resume fast-forward failed: {e}"));
            let _ = session.destroy_vm(lease);
            return report;
        }
    }
    let mut index = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&index_path)
    {
        Ok(v) => v,
        Err(e) => {
            report
                .errors
                .push(format!("cannot open capture index: {e}"));
            return report;
        }
    };
    for i in existing..opts.count as usize {
        let run = match session.run_frames(
            lease.clone(),
            opts.cadence,
            Some(spec.clone()),
            opts.hard_icount_cap,
        ) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(format!("capture run failed: {e}"));
                break;
            }
        };
        let (info, pixels) = match validate_capture_response(&run, layout.total_len as usize) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(e);
                break;
            }
        };
        let decoded = match decode_packed(&map, &run.feature_bytes) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(e);
                break;
            }
        };
        let id =
            &hash(format!("{}:{}:{}", opts.source_ref, info.frame_counter, i).as_bytes())[7..31];
        let feature_ref = format!("artifacts/feature-bytes/{id}.bin");
        let fb_ref = format!("artifacts/framebuffer/{id}.lz4");
        if let Err(e) = atomic_write(&opts.bundle.join(&feature_ref), &run.feature_bytes)
            .and_then(|_| atomic_write(&opts.bundle.join(&fb_ref), &run.fb_lz4))
        {
            report.errors.push(format!("artifact write failed: {e}"));
            break;
        }
        let row = serde_json::json!({"schema_version":1,"capture_id":format!("cap-{id}"),"node_ref":opts.source_ref,"capture_source":"phase4-capture-export","frame_index":info.frame_counter,"icount":run.icount,"layout_hash":layout.blake3,
            "feature_bytes":{"ref":feature_ref,"len":run.feature_bytes.len(),"blake3":hash(&run.feature_bytes)},"decoded_order":map.features.iter().map(|f| &f.name).collect::<Vec<_>>(),"decoded_values":decoded,
            "framebuffer":{"ref":fb_ref,"len":run.fb_lz4.len(),"encoding":"fb_lz4","width":256,"height":224,"stride":1024,"pixel_format":"xrgb8888","uncompressed_len":FB_LEN,"blake3":hash(&run.fb_lz4),"uncompressed_blake3":hash(&pixels)}});
        if writeln!(index, "{}", serde_json::to_string(&row).unwrap())
            .and_then(|_| index.sync_data())
            .is_err()
        {
            report
                .errors
                .push("cannot durably append capture index".into());
            break;
        }
        report.completed = i as u32 + 1;
        report.first_frame.get_or_insert(info.frame_counter);
        report.last_frame = Some(info.frame_counter);
    }
    let _ = session.destroy_vm(lease);
    report.status = if report.passed() { "pass" } else { "fail" }.into();
    report
}

fn validate_layout(layout: &Layout) -> Result<(), String> {
    if layout.ranges.is_empty() || layout.total_len == 0 {
        return Err("layout ranges and total_len must be nonempty".into());
    }
    let mut total = 0u64;
    for r in &layout.ranges {
        if r.region.is_empty()
            || r.layout_version != 1
            || r.len == 0
            || r.offset.checked_add(r.len).is_none()
        {
            return Err("layout contains invalid range".into());
        }
        total = total.checked_add(r.len).ok_or("layout total overflow")?;
    }
    if total != layout.total_len {
        return Err("layout total_len mismatch".into());
    }
    let preimage = serde_json::json!({"ranges":layout.ranges,"total_len":layout.total_len,"compiled_from_feature_map_hash":layout.compiled_from_feature_map_hash,"capture_spec_hash":layout.capture_spec_hash,"compiler_or_exporter_commit":layout.compiler_or_exporter_commit});
    if hash(&serde_json::to_vec(&preimage).unwrap()) != layout.blake3 {
        return Err("layout hash mismatch".into());
    }
    Ok(())
}
fn validate_capture_response(
    run: &proto::RunResponse,
    total_len: usize,
) -> Result<(proto::FbInfo, Vec<u8>), String> {
    if run.reason != proto::StopReason::BudgetReached as i32 {
        return Err("capture response did not stop at requested frame boundary".into());
    }
    if run.feature_bytes.len() != total_len {
        return Err("capture response feature length mismatch".into());
    }
    let info = run
        .fb_info
        .clone()
        .filter(|v| {
            v.width == 256
                && v.height == 224
                && v.stride == 1024
                && v.format == proto::PixelFormat::Xrgb8888 as i32
        })
        .ok_or("missing or malformed framebuffer metadata")?;
    let pixels = decompress_fb_lz4(&run.fb_lz4)
        .map_err(|e| format!("framebuffer decompression failed: {e}"))?;
    if pixels.len() != FB_LEN {
        return Err(format!(
            "framebuffer decoded length {} is not {FB_LEN}",
            pixels.len()
        ));
    }
    Ok((info, pixels))
}
fn packed_width(map: &FeatureMap) -> Option<usize> {
    map.features.iter().try_fold(0usize, |n, f| {
        n.checked_add(f.feature_type.derived_width().or(f.width)? as usize)
    })
}
fn decode_packed(map: &FeatureMap, bytes: &[u8]) -> Result<Vec<Value>, String> {
    let mut at = 0;
    let mut out = Vec::new();
    for f in &map.features {
        let width = f
            .feature_type
            .derived_width()
            .or(f.width)
            .ok_or("unresolved feature width")? as usize;
        let b = bytes
            .get(at..at + width)
            .ok_or("feature decode width mismatch")?;
        out.push(decode(&f.feature_type, b));
        at += width;
    }
    if at != bytes.len() {
        return Err("feature bytes contain trailing data".into());
    }
    Ok(out)
}
fn decode(t: &FeatureType, b: &[u8]) -> Value {
    match t {
        FeatureType::U8 | FeatureType::Bitflags8 | FeatureType::Bcd8 => b[0] as u64,
        FeatureType::I8 => return Value::from(b[0] as i8 as i64),
        FeatureType::U16le | FeatureType::Bitflags16le | FeatureType::Bcd16le => {
            u16::from_le_bytes([b[0], b[1]]) as u64
        }
        FeatureType::U16be => u16::from_be_bytes([b[0], b[1]]) as u64,
        FeatureType::I16le => return Value::from(i16::from_le_bytes([b[0], b[1]]) as i64),
        FeatureType::I16be => return Value::from(i16::from_be_bytes([b[0], b[1]]) as i64),
        FeatureType::U32le | FeatureType::Bitflags32le => {
            u32::from_le_bytes(b.try_into().unwrap()) as u64
        }
        FeatureType::U32be => u32::from_be_bytes(b.try_into().unwrap()) as u64,
        FeatureType::I32le => return Value::from(i32::from_le_bytes(b.try_into().unwrap()) as i64),
        FeatureType::I32be => return Value::from(i32::from_be_bytes(b.try_into().unwrap()) as i64),
        FeatureType::Bytes => return Value::String(format!("blake3:{}", blake3::hash(b).to_hex())),
    }
    .into()
}
fn validate_resume(
    path: &Path,
    opts: &CaptureExportOptions,
    layout: &Layout,
) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read existing index: {e}"))?;
    let mut ids = BTreeSet::new();
    for (i, line) in text.lines().enumerate() {
        let v: Value = serde_json::from_str(line)
            .map_err(|e| format!("existing index line {} invalid: {e}", i + 1))?;
        if v["layout_hash"] != layout.blake3 || v["node_ref"] != opts.source_ref {
            return Err("existing capture provenance conflicts with requested export".into());
        }
        let id = v["capture_id"]
            .as_str()
            .ok_or("existing capture id missing")?;
        if !ids.insert(id.to_owned()) {
            return Err("duplicate capture id in existing index".into());
        }
    }
    Ok(ids.len())
}
fn validate_resume_frames(path: &Path, base: u32, cadence: u32) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read existing index: {e}"))?;
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let row: Value =
            serde_json::from_str(line).map_err(|e| format!("existing index invalid: {e}"))?;
        let expected = base
            .checked_add(
                (i as u32 + 1)
                    .checked_mul(cadence)
                    .ok_or("resume frame overflow")?,
            )
            .ok_or("resume frame overflow")?;
        if row.get("frame_index").and_then(Value::as_u64) != Some(expected as u64) {
            return Err("existing capture frame cadence conflicts with restored snapshot".into());
        }
    }
    Ok(())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)
}
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let b = fs::read(path).map_err(|e| format!("cannot read layout: {e}"))?;
    serde_json::from_slice(&b).map_err(|e| format!("cannot parse layout: {e}"))
}
fn hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}
fn parse_hash(value: &str) -> Result<Vec<u8>, String> {
    let s = value.strip_prefix("blake3:").unwrap_or(value);
    if s.len() != 64 {
        return Err("snapshot hash must contain 64 hex digits".into());
    }
    (0..32)
        .map(|i| {
            u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| "snapshot hash is invalid hex".into())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase4_artifact_check::check_phase4_artifacts;
    use crate::phase4_layout::{write_phase4_layout, LayoutOptions};
    use refwork_dh_client::mock::{spawn_uds, MockFixture};
    const MAP: &str = r#"schema_version: 1
kind: feature-map
meta: {name: exporter-fixture, workload: worker-fixture, game_revision: private-revision, version: 1}
regions: [{name: wram, size: 131072}]
features:
  - {name: room_id, region: wram, offset: 64, type: u8, semantics: room_id, stability: stable}
"#;
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let map = root.path().join("map.yaml");
        let layout = root.path().join("layout.json");
        fs::write(&map, MAP).unwrap();
        let report = write_phase4_layout(&LayoutOptions {
            map: map.clone(),
            out: layout.clone(),
            capture_spec_hash: hash(b"spec"),
            layout_version: 1,
            compiler_or_exporter_commit: "0123456789012345678901234567890123456789".into(),
        });
        assert!(report.passed(), "{:?}", report.errors);
        fs::write(root.path().join("input.padlog"), "padlog v1\n4x0001\n").unwrap();
        (root, map, layout)
    }
    fn opts(
        root: &Path,
        map: PathBuf,
        layout: PathBuf,
        uds: &Path,
        cap: u64,
    ) -> CaptureExportOptions {
        CaptureExportOptions {
            endpoint: uds.to_string_lossy().into(),
            snapshot_hash: "ab".repeat(32),
            padlog: root.join("input.padlog"),
            map,
            layout,
            bundle: root.join("bundle"),
            count: 2,
            cadence: 1,
            hard_icount_cap: cap,
            production: false,
            source_ref: "snapshot:fixture".into(),
        }
    }
    #[test]
    fn phase4_capture_export_emits_frame_coherent_artifacts() {
        let (root, map, layout) = setup();
        let uds = root.path().join("worker.sock");
        let _worker = spawn_uds(MockFixture::default(), &uds).unwrap();
        let report = export_phase4_captures(&opts(
            root.path(),
            map.clone(),
            layout.clone(),
            &uds,
            10_000_000,
        ));
        assert!(report.passed(), "{:?}", report.errors);
        let checked = check_phase4_artifacts(&root.path().join("bundle"));
        assert!(checked.passed(), "{:?}", checked.errors);
        let text = fs::read_to_string(root.path().join("bundle/captures/index.jsonl")).unwrap();
        assert!(!text.contains("feature_bytes_inline"));
        assert!(text.contains("uncompressed_blake3"));
        let mut resumed_opts = opts(root.path(), map, layout, &uds, 10_000_000);
        resumed_opts.count = 3;
        let resumed = export_phase4_captures(&resumed_opts);
        assert!(resumed.passed(), "{:?}", resumed.errors);
        assert_eq!(resumed.resumed, 2);
        assert_eq!(
            check_phase4_artifacts(&root.path().join("bundle")).capture_count,
            3
        );
    }
    #[test]
    fn phase4_capture_export_rejects_layout_hash_drift() {
        let (root, map, layout) = setup();
        let mut v: Value = read_json(&layout).unwrap();
        v["blake3"] =
            "blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into();
        fs::write(&layout, serde_json::to_vec(&v).unwrap()).unwrap();
        let report = export_phase4_captures(&opts(
            root.path(),
            map.clone(),
            layout.clone(),
            Path::new("unused.sock"),
            100,
        ));
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("layout hash mismatch")))
    }

    #[test]
    fn phase4_capture_export_surfaces_worker_layout_precondition() {
        let (root, map, layout) = setup();
        let map_text = fs::read_to_string(&map)
            .unwrap()
            .replace("name: wram", "name: unregistered")
            .replace("region: wram", "region: unregistered");
        fs::write(&map, &map_text).unwrap();
        let report = write_phase4_layout(&LayoutOptions {
            map: map.clone(),
            out: layout.clone(),
            capture_spec_hash: hash(b"spec"),
            layout_version: 1,
            compiler_or_exporter_commit: "0123456789012345678901234567890123456789".into(),
        });
        assert!(report.passed());
        let uds = root.path().join("worker.sock");
        let _worker = spawn_uds(MockFixture::default(), &uds).unwrap();
        let result = export_phase4_captures(&opts(root.path(), map, layout, &uds, 10_000_000));
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("FailedPrecondition")),
            "{:?}",
            result.errors
        )
    }
    #[test]
    fn phase4_capture_export_rejects_malformed_capture_responses() {
        let mut run = proto::RunResponse::default();
        run.reason = proto::StopReason::BudgetReached as i32;
        run.feature_bytes = vec![0];
        assert!(validate_capture_response(&run, 1)
            .unwrap_err()
            .contains("framebuffer metadata"));
        run.fb_info = Some(proto::FbInfo {
            width: 256,
            height: 224,
            stride: 1024,
            format: proto::PixelFormat::Xrgb8888 as i32,
            frame_counter: 1,
        });
        run.fb_lz4 = vec![1, 2, 3];
        assert!(validate_capture_response(&run, 1)
            .unwrap_err()
            .contains("decompression"));
        run.feature_bytes.clear();
        assert!(validate_capture_response(&run, 1)
            .unwrap_err()
            .contains("feature length"))
    }
}
