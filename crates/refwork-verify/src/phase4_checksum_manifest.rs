//! Canonical payload-root and external recursive freeze-manifest tooling.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const CHECKSUM_SENTINEL: &str = "blake3:phase4-payload-root-normalized-v1";

#[derive(Debug, Clone)]
pub struct ChecksumManifestOptions {
    pub bundle: PathBuf,
    pub out: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChecksumManifestReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub payload_root: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
    pub files: Vec<ChecksumFileEntry>,
    pub errors: Vec<String>,
}

impl ChecksumManifestReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChecksumFileEntry {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

pub fn write_phase4_checksum_manifest(opts: &ChecksumManifestOptions) -> ChecksumManifestReport {
    let mut report = build_manifest(&opts.bundle);
    report.command =
        "refwork-verify phase4-checksum-manifest --bundle <redacted> --out <redacted>".into();
    if opts.out.starts_with(&opts.bundle) {
        report
            .errors
            .push("freeze manifest must be outside the bundle".into());
    }
    verify_embedded_payload_root(&opts.bundle, &mut report);
    report.status = if report.passed() { "pass" } else { "fail" }.into();
    if report.passed() {
        if let Some(parent) = opts.out.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                report
                    .errors
                    .push(format!("cannot create output directory: {e}"));
            }
        }
        if report.errors.is_empty() {
            if let Err(e) = fs::write(&opts.out, serde_json::to_vec_pretty(&report).unwrap()) {
                report
                    .errors
                    .push(format!("cannot write checksum manifest: {e}"));
            }
        }
    }
    report.status = if report.passed() { "pass" } else { "fail" }.into();
    report
}

pub fn set_phase4_payload_root(bundle: &Path) -> ChecksumManifestReport {
    let initial = build_manifest(bundle);
    let Some(root) = initial.payload_root else {
        return initial;
    };
    let path = bundle.join("manifest.json");
    let mut value: Value = match fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(value) => value,
        None => return failed("cannot read or parse manifest.json"),
    };
    let Some(object) = value.as_object_mut() else {
        return failed("manifest.json must be an object");
    };
    object.insert("bundle_checksum".into(), Value::String(root));
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = fs::write(&tmp, serde_json::to_vec_pretty(&value).unwrap())
        .and_then(|_| fs::rename(&tmp, &path))
    {
        return failed(&format!("cannot update manifest payload root: {e}"));
    }
    let mut report = build_manifest(bundle);
    report.command =
        "refwork-verify phase4-checksum-manifest --set-payload-root --bundle <redacted>".into();
    verify_embedded_payload_root(bundle, &mut report);
    report.status = if report.passed() { "pass" } else { "fail" }.into();
    report
}

pub fn verify_phase4_checksum_manifest(bundle: &Path, manifest: &Path) -> ChecksumManifestReport {
    let expected: ChecksumManifestReport = match fs::read(manifest)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        Some(v) => v,
        None => {
            return failed("cannot read or parse freeze manifest");
        }
    };
    let mut actual = build_manifest(bundle);
    verify_embedded_payload_root(bundle, &mut actual);
    actual.command =
        "refwork-verify phase4-checksum-manifest --verify <redacted> --bundle <redacted>".into();
    if expected.schema_version != 2 {
        actual
            .errors
            .push("freeze manifest schema_version must be 2".into());
    }
    if expected.payload_root != actual.payload_root {
        actual.errors.push("payload root mismatch".into());
    }
    let expected_files = expected
        .files
        .into_iter()
        .map(|e| (e.path.clone(), e))
        .collect::<BTreeMap<_, _>>();
    let actual_files = actual
        .files
        .iter()
        .cloned()
        .map(|e| (e.path.clone(), e))
        .collect::<BTreeMap<_, _>>();
    for path in expected_files
        .keys()
        .filter(|p| !actual_files.contains_key(*p))
    {
        actual.errors.push(format!("missing file {path}"));
    }
    for path in actual_files
        .keys()
        .filter(|p| !expected_files.contains_key(*p))
    {
        actual.errors.push(format!("extra file {path}"));
    }
    for (path, expected) in expected_files {
        if let Some(found) = actual_files.get(&path) {
            if &expected != found {
                actual.errors.push(format!("changed file {path}"));
            }
        }
    }
    actual.status = if actual.passed() { "pass" } else { "fail" }.into();
    actual
}

fn build_manifest(bundle: &Path) -> ChecksumManifestReport {
    let mut report = ChecksumManifestReport {
        schema_version: 2,
        status: "fail".into(),
        ..Default::default()
    };
    if !bundle.is_dir() {
        report.errors.push("bundle root is not a directory".into());
        return report;
    }
    walk(bundle, bundle, &mut report.files, &mut report.errors);
    report.files.sort_by(|a, b| a.path.cmp(&b.path));
    let paths = report
        .files
        .iter()
        .map(|e| e.path.as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "manifest.json",
        "feature-map.yaml",
        "scoring-program.yaml",
        "layout.json",
        "captures/index.jsonl",
        "dedup-groups.jsonl",
    ] {
        if !paths.contains(required) {
            report
                .errors
                .push(format!("missing required file {required}"));
        }
    }
    if !paths.contains("workload-image.yaml") && !paths.contains("workload-image-ref.txt") {
        report
            .errors
            .push("missing workload-image.yaml or workload-image-ref.txt".into());
    }
    let fallback = fs::read(bundle.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .as_deref()
        == Some("phase4-first-room-fallback");
    if !fallback {
        if !paths.contains("score-plan.json") {
            report
                .errors
                .push("missing required file score-plan.json".into());
        }
        if !paths.iter().any(|p| p.starts_with("trajectory/")) {
            report
                .errors
                .push("trajectory must contain at least one file".into());
        }
    }
    if !paths.iter().any(|p| p.starts_with("validation/")) {
        report
            .errors
            .push("validation must contain at least one file".into());
    }
    report.file_count = report.files.len();
    report.total_bytes = report.files.iter().map(|e| e.bytes).sum();
    report.payload_root = payload_root(bundle, &report.files, &mut report.errors);
    report.status = if report.passed() { "pass" } else { "fail" }.into();
    report
}

fn verify_embedded_payload_root(bundle: &Path, report: &mut ChecksumManifestReport) {
    let embedded = fs::read(bundle.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| {
            v.get("bundle_checksum")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    if embedded != report.payload_root {
        report.errors.push("manifest.json.bundle_checksum does not equal canonical payload root; run --set-payload-root before sealing".into());
    }
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<ChecksumFileEntry>, errors: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(v) => v,
        Err(e) => {
            errors.push(format!("cannot read bundle directory: {e}"));
            return;
        }
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let meta = match fs::symlink_metadata(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("cannot inspect bundle entry: {e}"));
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            errors.push("bundle contains a symlink".into());
            continue;
        }
        if meta.is_dir() {
            walk(root, &path, files, errors);
            continue;
        }
        if !meta.is_file() {
            errors.push("bundle contains a non-regular entry".into());
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        match fs::read(&path) {
            Ok(bytes) => files.push(ChecksumFileEntry {
                path: rel,
                bytes: bytes.len() as u64,
                blake3: hash(&bytes),
            }),
            Err(e) => errors.push(format!("cannot read bundle file: {e}")),
        }
    }
}

fn payload_root(
    bundle: &Path,
    files: &[ChecksumFileEntry],
    errors: &mut Vec<String>,
) -> Option<String> {
    let mut payload = Vec::new();
    for entry in files.iter().filter(|e| !e.path.starts_with("validation/")) {
        let mut normalized = entry.clone();
        if entry.path == "manifest.json" {
            let bytes = match fs::read(bundle.join(&entry.path)) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("cannot read manifest.json: {e}"));
                    return None;
                }
            };
            let mut value: Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("manifest.json is invalid JSON: {e}"));
                    return None;
                }
            };
            let Some(object) = value.as_object_mut() else {
                errors.push("manifest.json must be an object".into());
                return None;
            };
            object.insert(
                "bundle_checksum".into(),
                Value::String(CHECKSUM_SENTINEL.into()),
            );
            let bytes = serde_json::to_vec(&value).unwrap();
            normalized.bytes = bytes.len() as u64;
            normalized.blake3 = hash(&bytes);
        }
        payload.push(normalized);
    }
    let unique = payload.iter().map(|e| &e.path).collect::<BTreeSet<_>>();
    if unique.len() != payload.len() {
        errors.push("duplicate payload path".into());
        return None;
    }
    Some(hash(&serde_json::to_vec(&payload).unwrap()))
}

fn hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}
fn failed(message: &str) -> ChecksumManifestReport {
    ChecksumManifestReport {
        schema_version: 2,
        status: "fail".into(),
        errors: vec![message.into()],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase4_checksum_is_recursive_normalized_and_verifiable() {
        let root = tempfile::tempdir().unwrap();
        let seal = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("artifacts/deep")).unwrap();
        fs::create_dir_all(root.path().join("captures")).unwrap();
        fs::create_dir_all(root.path().join("trajectory")).unwrap();
        fs::create_dir_all(root.path().join("validation")).unwrap();
        fs::write(
            root.path().join("manifest.json"),
            r#"{"bundle_checksum":"old","x":1}"#,
        )
        .unwrap();
        fs::write(root.path().join("artifacts/deep/a"), b"payload").unwrap();
        fs::write(root.path().join("validation/report"), b"report").unwrap();
        for path in [
            "feature-map.yaml",
            "scoring-program.yaml",
            "layout.json",
            "captures/index.jsonl",
            "dedup-groups.jsonl",
            "score-plan.json",
            "workload-image-ref.txt",
            "trajectory/trace.jsonl",
        ] {
            fs::write(root.path().join(path), b"fixture").unwrap();
        }
        let out = seal.path().join("freeze.json");
        assert!(set_phase4_payload_root(root.path()).passed());
        let first = write_phase4_checksum_manifest(&ChecksumManifestOptions {
            bundle: root.path().into(),
            out: out.clone(),
        });
        assert!(first.passed(), "{:?}", first.errors);
        assert!(first.files.iter().any(|e| e.path == "artifacts/deep/a"));
        assert!(verify_phase4_checksum_manifest(root.path(), &out).passed());
        let out2 = seal.path().join("freeze-2.json");
        assert!(write_phase4_checksum_manifest(&ChecksumManifestOptions {
            bundle: root.path().into(),
            out: out2.clone()
        })
        .passed());
        assert_eq!(fs::read(&out).unwrap(), fs::read(out2).unwrap());
        fs::write(root.path().join("extra"), b"x").unwrap();
        assert!(verify_phase4_checksum_manifest(root.path(), &out)
            .errors
            .iter()
            .any(|e| e.contains("extra file")));
    }
}
