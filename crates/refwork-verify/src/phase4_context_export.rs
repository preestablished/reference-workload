//! Derive a Phase 4 context fixture from an already captured corpus.

use crate::phase4_artifact_check::check_phase4_artifacts;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ContextExportOptions {
    pub corpus: PathBuf,
    pub out: PathBuf,
    pub capture_ids: Vec<String>,
    pub context_artifact_id: String,
    pub access_requirement: String,
    pub retention: String,
    pub pad_table_hash: String,
    pub recent_input: Option<PathBuf>,
    pub evidence_type: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ContextExportReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub workload_image_manifest_hash: String,
    pub feature_map_hash: String,
    pub layout_hash: String,
    pub capture_count: usize,
    pub recent_input_available: bool,
    pub errors: Vec<String>,
}

impl ContextExportReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty() && self.capture_count > 0
    }
}

pub fn export_phase4_context(opts: &ContextExportOptions) -> ContextExportReport {
    let mut report = ContextExportReport {
        schema_version: 1,
        command: "refwork-verify phase4-context-export <private arguments redacted>".into(),
        status: "fail".into(),
        recent_input_available: opts.recent_input.is_some(),
        ..Default::default()
    };
    if opts.capture_ids.is_empty() {
        report
            .errors
            .push("at least one capture id is required".into());
        return report;
    }
    if opts.evidence_type != "live" && opts.evidence_type != "synthetic" {
        report
            .errors
            .push("evidence type must be live or synthetic".into());
        return report;
    }
    if !looks_hash(&opts.pad_table_hash) {
        report
            .errors
            .push("pad table hash must be a BLAKE3 hash".into());
        return report;
    }
    let artifact_report = check_phase4_artifacts(&opts.corpus);
    if !artifact_report.passed() {
        report.errors.push(format!(
            "source artifact verification failed with {} issue(s)",
            artifact_report.errors.len()
        ));
        return report;
    }
    let manifest: Value = match read_json(&opts.corpus.join("manifest.json")) {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(e);
            return report;
        }
    };
    let required = |path: &[&str]| -> Option<String> {
        value_at(&manifest, path)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let Some(commit) = required(&["reference_workload_commit"]) else {
        report
            .errors
            .push("source manifest missing reference_workload_commit".into());
        return report;
    };
    let Some(feature_hash) = required(&["feature_map_hash"]) else {
        report
            .errors
            .push("source manifest missing feature_map_hash".into());
        return report;
    };
    let map_text = match fs::read_to_string(opts.corpus.join("feature-map.yaml")) {
        Ok(v) => v,
        Err(e) => {
            report
                .errors
                .push(format!("cannot read source feature map: {e}"));
            return report;
        }
    };
    if hash(&map_text.as_bytes()) != feature_hash {
        report
            .errors
            .push("source feature-map hash does not match manifest".into());
        return report;
    }
    let (feature_map, map_errors) = match refwork_featuremap::parse_feature_map(&map_text) {
        Ok(v) => v,
        Err(e) => {
            report
                .errors
                .push(format!("source feature map parse failed: {e}"));
            return report;
        }
    };
    if !map_errors.is_empty() {
        report
            .errors
            .push("source feature map validation failed".into());
        return report;
    }
    let expected_order = feature_map
        .features
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>();
    let Some(scoring_hash) = required(&["scoring_program_hash"]) else {
        report
            .errors
            .push("source manifest missing scoring_program_hash".into());
        return report;
    };
    let Some(layout_hash) = required(&["layout_hash"]) else {
        report
            .errors
            .push("source manifest missing layout_hash".into());
        return report;
    };
    let image_hash = required(&["workload_image", "manifest_hash"]);
    let image_ref = required(&["workload_image", "private_artifact_id"])
        .or_else(|| required(&["workload_image", "artifact_id"]));
    if image_hash.is_none() && image_ref.is_none() {
        report
            .errors
            .push("source manifest lacks workload image hash/ref".into());
        return report;
    }
    report.workload_image_manifest_hash = image_hash
        .clone()
        .unwrap_or_else(|| image_ref.clone().unwrap());
    report.feature_map_hash = feature_hash.clone();
    report.layout_hash = layout_hash.clone();

    let index = match fs::read_to_string(opts.corpus.join("captures/index.jsonl")) {
        Ok(v) => v,
        Err(e) => {
            report
                .errors
                .push(format!("cannot read source capture index: {e}"));
            return report;
        }
    };
    let wanted = opts.capture_ids.iter().cloned().collect::<BTreeSet<_>>();
    if wanted.len() != opts.capture_ids.len() {
        report
            .errors
            .push("capture id selection contains duplicates".into());
        return report;
    }
    let mut rows = BTreeMap::new();
    for (line_no, line) in index.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                report
                    .errors
                    .push(format!("source capture line {} invalid: {e}", line_no + 1));
                return report;
            }
        };
        if let Some(id) = row.get("capture_id").and_then(Value::as_str) {
            if wanted.contains(id) {
                rows.insert(id.to_owned(), row);
            }
        }
    }
    for id in &opts.capture_ids {
        if !rows.contains_key(id) {
            report
                .errors
                .push(format!("selected capture id not found: {id}"));
        }
    }
    if !report.errors.is_empty() {
        return report;
    }
    if opts.out.exists()
        && fs::read_dir(&opts.out)
            .map(|mut e| e.next().is_some())
            .unwrap_or(false)
    {
        report
            .errors
            .push("context output directory must be absent or empty".into());
        return report;
    }
    if let Err(e) = fs::create_dir_all(opts.out.join("validation"))
        .and_then(|_| fs::create_dir_all(opts.out.join("artifacts/feature-bytes")))
        .and_then(|_| fs::create_dir_all(opts.out.join("artifacts/framebuffer")))
    {
        report
            .errors
            .push(format!("cannot create context output: {e}"));
        return report;
    }
    let mut context_lines = String::new();
    for id in &opts.capture_ids {
        let row = &rows[id];
        let feature = match copy_meta_artifact(&opts.corpus, &opts.out, &row["feature_bytes"]) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(e);
                break;
            }
        };
        let framebuffer = match copy_meta_artifact(&opts.corpus, &opts.out, &row["framebuffer"]) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(e);
                break;
            }
        };
        let order = row.get("decoded_order").cloned().unwrap_or(Value::Null);
        let values = row.get("decoded_values").cloned().unwrap_or(Value::Null);
        let Some(names) = order.as_array() else {
            report
                .errors
                .push("selected capture decoded_order is not an array".into());
            break;
        };
        if names.iter().filter_map(Value::as_str).collect::<Vec<_>>() != expected_order {
            report
                .errors
                .push("selected capture decoded order does not match source feature map".into());
            break;
        }
        let Some(decoded) = values.as_array() else {
            report
                .errors
                .push("selected capture decoded_values is not an array".into());
            break;
        };
        if names.len() != decoded.len() {
            report
                .errors
                .push("selected capture decoded order/value length mismatch".into());
            break;
        }
        let by_name = names
            .iter()
            .zip(decoded)
            .filter_map(|(n, v)| n.as_str().map(|n| (n.to_owned(), v.clone())))
            .collect::<Map<_, _>>();
        let region = serde_json::json!({"name":"packed-feature-bytes","size":feature["len"],"layout_version":1,"ref":feature["ref"],"blake3":feature["blake3"]});
        let recent = if opts.recent_input.is_some() {
            serde_json::json!({"available":true,"padlog_ref":"recent-input.padlog"})
        } else {
            serde_json::json!({"available":false,"reason":"not retained by operator policy"})
        };
        let ctx = serde_json::json!({"schema_version":1,"capture_id":id,"node_ref":row["node_ref"],"frame_index":row["frame_index"],"workload_image_manifest_hash":report.workload_image_manifest_hash,"feature_map_hash":feature_hash,"layout_hash":layout_hash,"decoded_order":order,"decoded_values":values,"decoded_by_name":by_name,"framebuffer":framebuffer,"regions":[region],"recent_input":recent});
        context_lines.push_str(&serde_json::to_string(&ctx).unwrap());
        context_lines.push('\n');
    }
    if !report.errors.is_empty() {
        return report;
    }
    if let Some(input) = &opts.recent_input {
        let text = match fs::read_to_string(input) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push(format!("cannot read recent input: {e}"));
                return report;
            }
        };
        if let Err(e) = refwork_script::parse(&text) {
            report.errors.push(format!("recent input is invalid: {e}"));
            return report;
        }
        if let Err(e) = atomic_write(&opts.out.join("recent-input.padlog"), text.as_bytes()) {
            report
                .errors
                .push(format!("cannot write recent input: {e}"));
            return report;
        }
    }
    report.capture_count = opts.capture_ids.len();
    let workload_image = if let Some(hash) = image_hash {
        serde_json::json!({"manifest_hash":hash,"artifact_id":image_ref})
    } else {
        serde_json::json!({"artifact_id":image_ref})
    };
    let context_manifest = serde_json::json!({"schema_version":1,"kind":"phase4-context-smoke","evidence_type":opts.evidence_type,"reference_workload_commit":commit,"workload_image":workload_image,"pad_layout":{"layout_id":"console16-12btn-v1","layout_version":1,"table_hash":opts.pad_table_hash},"feature_map_hash":feature_hash,"scoring_program_hash":scoring_hash,"layout_hash":layout_hash,"capture_count":report.capture_count,"recent_input_available":report.recent_input_available,"recent_input_unavailable_reason":if report.recent_input_available{Value::Null}else{Value::String("not retained by operator policy".into())},"private_storage":{"artifact_id":opts.context_artifact_id,"access_requirement":opts.access_requirement,"retention":opts.retention},"clean_room_provenance":"derived from verified Phase 4 corpus captures"});
    if let Err(e) = atomic_write(&opts.out.join("contexts.jsonl"), context_lines.as_bytes())
        .and_then(|_| {
            atomic_write(
                &opts.out.join("manifest.json"),
                &serde_json::to_vec_pretty(&context_manifest).unwrap(),
            )
        })
    {
        report
            .errors
            .push(format!("cannot write context contract: {e}"));
        return report;
    }
    report.status = "pass".into();
    let report_bytes = serde_json::to_vec_pretty(&report).unwrap();
    if let Err(e) = atomic_write(
        &opts.out.join("validation/context-export-report.json"),
        &report_bytes,
    ) {
        report
            .errors
            .push(format!("cannot write export report: {e}"));
        report.status = "fail".into()
    }
    report
}

fn copy_meta_artifact(source: &Path, out: &Path, meta: &Value) -> Result<Value, String> {
    let reference = meta
        .get("ref")
        .and_then(Value::as_str)
        .ok_or("artifact ref missing")?;
    let bytes = fs::read(source.join(reference))
        .map_err(|e| format!("cannot read selected artifact: {e}"))?;
    atomic_write(&out.join(reference), &bytes)
        .map_err(|e| format!("cannot copy selected artifact: {e}"))?;
    Ok(meta.clone())
}
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p)?
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?
    }
    fs::rename(tmp, path)
}
fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|e| {
        format!(
            "cannot read {}: {e}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON: {e}"))
}
fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, k| v.get(k))
}
fn looks_hash(v: &str) -> bool {
    v.strip_prefix("blake3:")
        .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phase4_context_check::check_phase4_context_bundle;

    fn hash(bytes: &[u8]) -> String {
        format!("blake3:{}", blake3::hash(bytes).to_hex())
    }
    fn source() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("captures")).unwrap();
        fs::create_dir_all(root.path().join("artifacts/feature-bytes")).unwrap();
        fs::create_dir_all(root.path().join("artifacts/framebuffer")).unwrap();
        let map="schema_version: 1\nkind: feature-map\nmeta: {name: context-fixture, workload: fixture, game_revision: private, version: 1}\nregions: [{name: wram, size: 131072}]\nfeatures:\n  - {name: room, region: wram, offset: 0, type: u8, semantics: room_id, stability: stable}\n";
        fs::write(root.path().join("feature-map.yaml"), map).unwrap();
        let h = hash(map.as_bytes());
        fs::write(root.path().join("manifest.json"),serde_json::to_vec(&serde_json::json!({"reference_workload_commit":"0123456789012345678901234567890123456789","workload_image":{"manifest_hash":h,"private_artifact_id":"artifact:image"},"feature_map_hash":h,"scoring_program_hash":h,"layout_hash":h})).unwrap()).unwrap();
        let feature = [9u8];
        let pixels = vec![3u8; 229376];
        let fb = lz4_flex::compress_prepend_size(&pixels);
        fs::write(root.path().join("artifacts/feature-bytes/a.bin"), feature).unwrap();
        fs::write(root.path().join("artifacts/framebuffer/a.lz4"), &fb).unwrap();
        let row = serde_json::json!({"capture_id":"cap-a","node_ref":"node:a","frame_index":7,"layout_hash":h,"feature_bytes":{"ref":"artifacts/feature-bytes/a.bin","len":1,"blake3":hash(&feature)},"decoded_order":["room"],"decoded_values":[9],"framebuffer":{"ref":"artifacts/framebuffer/a.lz4","len":fb.len(),"encoding":"fb_lz4","width":256,"height":224,"stride":1024,"pixel_format":"xrgb8888","uncompressed_len":229376,"blake3":hash(&fb),"uncompressed_blake3":hash(&pixels)}});
        fs::write(
            root.path().join("captures/index.jsonl"),
            format!("{}\n", serde_json::to_string(&row).unwrap()),
        )
        .unwrap();
        root
    }
    #[test]
    fn phase4_context_export_derives_checker_accepted_fixture() {
        let source = source();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("context");
        let report = export_phase4_context(&ContextExportOptions {
            corpus: source.path().into(),
            out: path.clone(),
            capture_ids: vec!["cap-a".into()],
            context_artifact_id: "artifact:context".into(),
            access_requirement: "role:lab".into(),
            retention: "test only".into(),
            pad_table_hash: hash(b"pad"),
            recent_input: None,
            evidence_type: "synthetic".into(),
        });
        assert!(report.passed(), "{:?}", report.errors);
        let checked = check_phase4_context_bundle(&path);
        assert!(checked.passed(), "{:?}", checked.errors)
    }
    #[test]
    fn phase4_context_export_rejects_unknown_selection() {
        let source = source();
        let out = tempfile::tempdir().unwrap();
        let report = export_phase4_context(&ContextExportOptions {
            corpus: source.path().into(),
            out: out.path().join("x"),
            capture_ids: vec!["missing".into()],
            context_artifact_id: "artifact:context".into(),
            access_requirement: "role:lab".into(),
            retention: "test".into(),
            pad_table_hash: hash(b"pad"),
            recent_input: None,
            evidence_type: "synthetic".into(),
        });
        assert!(!report.passed())
    }
}
