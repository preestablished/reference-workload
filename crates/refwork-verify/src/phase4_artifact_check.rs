//! Read-only integrity verification for Phase 4 capture artifacts.

use refwork_dh_client::decompress_fb_lz4;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const FRAMEBUFFER_LEN: usize = 256 * 224 * 4;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ArtifactCheckReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub capture_count: usize,
    pub feature_artifact_count: usize,
    pub framebuffer_artifact_count: usize,
    pub stored_bytes: u64,
    pub decoded_framebuffer_bytes: u64,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("captures")).unwrap();
        fs::create_dir_all(root.path().join("artifacts/feature-bytes")).unwrap();
        fs::create_dir_all(root.path().join("artifacts/framebuffer")).unwrap();
        let feature = [1u8, 2, 3];
        let pixels = vec![7u8; FRAMEBUFFER_LEN];
        let framebuffer = lz4_flex::compress_prepend_size(&pixels);
        fs::write(root.path().join("artifacts/feature-bytes/a.bin"), feature).unwrap();
        fs::write(
            root.path().join("artifacts/framebuffer/a.lz4"),
            &framebuffer,
        )
        .unwrap();
        let row = serde_json::json!({
            "feature_bytes": {"ref":"artifacts/feature-bytes/a.bin", "len":feature.len(), "blake3":hash(&feature)},
            "framebuffer": {"ref":"artifacts/framebuffer/a.lz4", "len":framebuffer.len(), "blake3":hash(&framebuffer),
                "uncompressed_blake3":hash(&pixels), "encoding":"fb_lz4", "width":256, "height":224,
                "stride":1024, "pixel_format":"xrgb8888", "uncompressed_len":FRAMEBUFFER_LEN}
        });
        let mut index = fs::File::create(root.path().join("captures/index.jsonl")).unwrap();
        writeln!(index, "{}", serde_json::to_string(&row).unwrap()).unwrap();
        root
    }

    fn hash(bytes: &[u8]) -> String {
        format!("blake3:{}", blake3::hash(bytes).to_hex())
    }

    #[test]
    fn phase4_artifact_accepts_contained_hashed_artifacts() {
        let root = fixture();
        assert!(check_phase4_artifacts(root.path()).passed());
    }

    #[test]
    fn phase4_artifact_rejects_corrupt_and_escaping_artifacts() {
        let root = fixture();
        fs::write(root.path().join("artifacts/feature-bytes/a.bin"), [9]).unwrap();
        let report = check_phase4_artifacts(root.path());
        assert!(!report.passed());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("len does not match")));

        let index = root.path().join("captures/index.jsonl");
        let text = fs::read_to_string(&index)
            .unwrap()
            .replace("artifacts/feature-bytes/a.bin", "../outside.bin");
        fs::write(index, text).unwrap();
        let report = check_phase4_artifacts(root.path());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("contained relative path")));
    }

    #[test]
    fn phase4_artifact_rejects_missing_and_truncated_framebuffers() {
        let root = fixture();
        fs::remove_file(root.path().join("artifacts/feature-bytes/a.bin")).unwrap();
        assert!(check_phase4_artifacts(root.path())
            .errors
            .iter()
            .any(|e| e.contains("artifact is missing")));
        let root = fixture();
        fs::write(root.path().join("artifacts/framebuffer/a.lz4"), [0, 1, 2]).unwrap();
        let report = check_phase4_artifacts(root.path());
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("decompression failed")));
        assert!(report.errors.iter().any(|e| e.contains("blake3 mismatch")))
    }

    #[cfg(unix)]
    #[test]
    fn phase4_artifact_rejects_symlinks_and_pixel_hash_mismatch() {
        use std::os::unix::fs::symlink;
        let root = fixture();
        let index = root.path().join("captures/index.jsonl");
        let text = fs::read_to_string(&index).unwrap();
        let mut row: Value = serde_json::from_str(text.trim()).unwrap();
        row["framebuffer"]["uncompressed_blake3"] =
            "blake3:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".into();
        fs::write(
            &index,
            format!("{}\n", serde_json::to_string(&row).unwrap()),
        )
        .unwrap();
        assert!(check_phase4_artifacts(root.path())
            .errors
            .iter()
            .any(|e| e.contains("uncompressed_blake3 mismatch")));
        let fb = root.path().join("artifacts/framebuffer/a.lz4");
        let target = root.path().join("artifacts/framebuffer/real.lz4");
        fs::rename(&fb, &target).unwrap();
        symlink(&target, &fb).unwrap();
        assert!(check_phase4_artifacts(root.path())
            .errors
            .iter()
            .any(|e| e.contains("non-symlink")));
    }
}

impl ArtifactCheckReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn check_phase4_artifacts(bundle: &Path) -> ArtifactCheckReport {
    let mut checker = Checker {
        bundle,
        canonical_root: bundle.canonicalize().ok(),
        seen_refs: BTreeSet::new(),
        report: ArtifactCheckReport {
            schema_version: 1,
            command: "refwork-verify phase4-artifact-check --bundle <redacted> --report <redacted>"
                .into(),
            status: "fail".into(),
            ..Default::default()
        },
    };
    checker.run();
    checker.report.status = if checker.report.passed() {
        "pass"
    } else {
        "fail"
    }
    .into();
    checker.report
}

struct Checker<'a> {
    bundle: &'a Path,
    canonical_root: Option<PathBuf>,
    seen_refs: BTreeSet<String>,
    report: ArtifactCheckReport,
}

impl Checker<'_> {
    fn run(&mut self) {
        if self.canonical_root.is_none() || !self.bundle.is_dir() {
            self.error("bundle root is not a readable directory");
            return;
        }
        let index = self.bundle.join("captures/index.jsonl");
        let text = match fs::read_to_string(index) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("cannot read captures/index.jsonl: {e}"));
                return;
            }
        };
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            self.report.capture_count += 1;
            let row: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    self.error(format!(
                        "captures/index.jsonl line {} is invalid JSON: {e}",
                        line_no + 1
                    ));
                    continue;
                }
            };
            self.check_feature(&row, line_no + 1);
            self.check_framebuffer(&row, line_no + 1);
        }
        if self.report.capture_count == 0 {
            self.error("captures/index.jsonl contains no rows");
        }
    }

    fn check_feature(&mut self, row: &Value, line: usize) {
        let Some(meta) = row.get("feature_bytes") else {
            self.error(format!("line {line}: missing feature_bytes"));
            return;
        };
        let Some(bytes) = self.read_artifact(meta, line, "feature_bytes") else {
            return;
        };
        self.report.feature_artifact_count += 1;
        self.report.stored_bytes += bytes.len() as u64;
    }

    fn check_framebuffer(&mut self, row: &Value, line: usize) {
        let Some(meta) = row.get("framebuffer") else {
            self.error(format!("line {line}: missing framebuffer"));
            return;
        };
        for (field, expected) in [("encoding", "fb_lz4"), ("pixel_format", "xrgb8888")] {
            if meta.get(field).and_then(Value::as_str) != Some(expected) {
                self.error(format!(
                    "line {line}: framebuffer.{field} must be {expected}"
                ));
            }
        }
        for (field, expected) in [
            ("width", 256),
            ("height", 224),
            ("stride", 1024),
            ("uncompressed_len", FRAMEBUFFER_LEN as u64),
        ] {
            if meta.get(field).and_then(Value::as_u64) != Some(expected) {
                self.error(format!(
                    "line {line}: framebuffer.{field} must be {expected}"
                ));
            }
        }
        let Some(bytes) = self.read_artifact(meta, line, "framebuffer") else {
            return;
        };
        self.report.framebuffer_artifact_count += 1;
        self.report.stored_bytes += bytes.len() as u64;
        match decompress_fb_lz4(&bytes) {
            Ok(decoded) => {
                self.report.decoded_framebuffer_bytes += decoded.len() as u64;
                if decoded.len() != FRAMEBUFFER_LEN {
                    self.error(format!(
                        "line {line}: framebuffer decodes to {} bytes, expected {FRAMEBUFFER_LEN}",
                        decoded.len()
                    ));
                }
                self.check_hash(
                    meta,
                    "uncompressed_blake3",
                    &decoded,
                    line,
                    "decoded framebuffer",
                );
            }
            Err(e) => self.error(format!(
                "line {line}: framebuffer decompression failed: {e}"
            )),
        }
    }

    fn read_artifact(&mut self, meta: &Value, line: usize, kind: &str) -> Option<Vec<u8>> {
        let Some(reference) = meta.get("ref").and_then(Value::as_str) else {
            self.error(format!("line {line}: {kind}.ref missing"));
            return None;
        };
        let rel = Path::new(reference);
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
            self.error(format!(
                "line {line}: {kind}.ref is not a contained relative path"
            ));
            return None;
        }
        if !self.seen_refs.insert(reference.to_owned()) {
            self.error(format!("line {line}: duplicate artifact ref"));
            return None;
        }
        let path = self.bundle.join(rel);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("line {line}: {kind} artifact is missing: {e}"));
                return None;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            self.error(format!(
                "line {line}: {kind} artifact must be a regular non-symlink file"
            ));
            return None;
        }
        let canonical = match path.canonicalize() {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("line {line}: cannot resolve {kind} artifact: {e}"));
                return None;
            }
        };
        if !canonical.starts_with(self.canonical_root.as_ref().unwrap()) {
            self.error(format!("line {line}: {kind} artifact escapes bundle root"));
            return None;
        }
        let bytes = match fs::read(path) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("line {line}: cannot read {kind} artifact: {e}"));
                return None;
            }
        };
        if meta.get("len").and_then(Value::as_u64) != Some(bytes.len() as u64) {
            self.error(format!(
                "line {line}: {kind}.len does not match stored artifact"
            ));
        }
        self.check_hash(meta, "blake3", &bytes, line, kind);
        Some(bytes)
    }

    fn check_hash(&mut self, meta: &Value, field: &str, bytes: &[u8], line: usize, kind: &str) {
        let actual = format!("blake3:{}", blake3::hash(bytes).to_hex());
        if meta.get(field).and_then(Value::as_str) != Some(actual.as_str()) {
            self.error(format!("line {line}: {kind} {field} mismatch"));
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        if self.report.errors.len() < 200 {
            self.report.errors.push(message.into());
        }
    }
}
