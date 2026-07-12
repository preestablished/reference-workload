//! Separately typed validation for the explicitly approved first-room fallback.

use crate::phase4_artifact_check::check_phase4_artifacts;
use refwork_featuremap::{parse_feature_map, Stability};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Default)]
pub struct FallbackReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub bundle_kind: String,
    pub capture_count: usize,
    pub same_canonical_groups: usize,
    pub distinct_stable_groups: usize,
    pub errors: Vec<String>,
}
impl FallbackReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn check_phase4_fallback(bundle: &Path) -> FallbackReport {
    let mut c = Checker {
        bundle,
        report: FallbackReport {
            schema_version: 1,
            command: "refwork-verify phase4-fallback-check --bundle <redacted> --report <redacted>"
                .into(),
            status: "fail".into(),
            bundle_kind: "first-room-fallback".into(),
            ..Default::default()
        },
    };
    c.run();
    c.report.status = if c.report.passed() { "pass" } else { "fail" }.into();
    c.report
}
struct Checker<'a> {
    bundle: &'a Path,
    report: FallbackReport,
}
impl Checker<'_> {
    fn run(&mut self) {
        if !self.bundle.is_dir() {
            self.error("bundle root is not a directory");
            return;
        }
        for file in [
            "manifest.json",
            "feature-map.yaml",
            "scoring-program.yaml",
            "layout.json",
            "captures/index.jsonl",
            "dedup-groups.jsonl",
        ] {
            if !self.bundle.join(file).is_file() {
                self.error(format!("missing required file {file}"));
            }
        }
        if !self.bundle.join("workload-image.yaml").is_file()
            && !self.bundle.join("workload-image-ref.txt").is_file()
        {
            self.error("missing workload image ref")
        }
        if !self.bundle.join("validation").is_dir() {
            self.error("missing validation directory")
        }
        let manifest = match read_json(&self.bundle.join("manifest.json")) {
            Ok(v) => v,
            Err(e) => {
                self.error(e);
                return;
            }
        };
        self.manifest(&manifest);
        let stable = self.map_stability();
        self.dedup(&stable);
        let artifacts = check_phase4_artifacts(self.bundle);
        if !artifacts.passed() {
            self.error(format!(
                "artifact verification failed with {} issue(s)",
                artifacts.errors.len()
            ))
        } else if artifacts.capture_count != self.report.capture_count {
            self.error("manifest capture_count does not match artifact index")
        }
        self.validation();
    }
    fn manifest(&mut self, v: &Value) {
        self.expect(v, "schema_version", Value::from(1));
        self.expect(v, "kind", Value::from("phase4-first-room-fallback"));
        self.expect(v, "scope_version", Value::from("first-room-v1"));
        self.expect(v, "fulfillment_claim", Value::from("partial"));
        self.report.capture_count = v["capture_count"].as_u64().unwrap_or(0) as usize;
        if self.report.capture_count == 0 {
            self.error("capture_count must be positive")
        }
        for (key, expected) in [
            ("first_room", true),
            ("decode_goldens", true),
            ("first_boss", false),
            ("goal_positive", false),
            ("trajectory", false),
        ] {
            if v["coverage"][key].as_bool() != Some(expected) {
                self.error(format!("coverage.{key} must be {expected}"))
            }
        }
        for key in ["same_canonical", "distinct_stable", "volatile_only_change"] {
            if v["coverage"][key].as_bool() != Some(true) {
                self.error(format!("coverage.{key} must be true"))
            }
        }
        for key in [
            "reference_workload_commit",
            "feature_map_hash",
            "scoring_program_hash",
            "layout_hash",
            "exporter_commit",
        ] {
            if v.get(key).and_then(Value::as_str).is_none() {
                self.error(format!("manifest.{key} is required"))
            }
        }
        for path in [
            ["private_storage", "artifact_id"],
            ["private_storage", "access_requirement"],
            ["private_storage", "retention"],
            ["follow_on", "task_id"],
            ["follow_on", "owner_role"],
        ] {
            if at(v, &path)
                .and_then(Value::as_str)
                .is_none_or(|s| s.trim().is_empty())
            {
                self.error(format!("manifest.{} is required", path.join(".")))
            }
        }
    }
    fn map_stability(&mut self) -> BTreeMap<String, Stability> {
        let text = match fs::read_to_string(self.bundle.join("feature-map.yaml")) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("cannot read feature map: {e}"));
                return BTreeMap::new();
            }
        };
        let (map, errors) = match parse_feature_map(&text) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("feature map parse failed: {e}"));
                return BTreeMap::new();
            }
        };
        for e in errors {
            self.error(format!("feature map validation: {e}"))
        }
        if map.meta.workload.to_ascii_lowercase().contains("demo")
            || map
                .meta
                .game_revision
                .to_ascii_lowercase()
                .contains("synthetic")
        {
            self.error("placeholder/demo feature map is forbidden")
        };
        map.features
            .into_iter()
            .map(|f| (f.name, f.stability))
            .collect()
    }
    fn dedup(&mut self, stable: &BTreeMap<String, Stability>) {
        let text = match fs::read_to_string(self.bundle.join("dedup-groups.jsonl")) {
            Ok(v) => v,
            Err(e) => {
                self.error(format!("cannot read dedup groups: {e}"));
                return;
            }
        };
        let mut ids = BTreeSet::new();
        for (line_no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    self.error(format!("dedup line {} invalid: {e}", line_no + 1));
                    continue;
                }
            };
            let id = v["group_id"].as_str().unwrap_or("");
            if id.is_empty() || !ids.insert(id.to_owned()) {
                self.error("dedup group ids must be nonempty and unique")
            };
            let changed = v["changed_features"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            match v["expected_relation"].as_str() {
                Some("same_canonical_state") => {
                    self.report.same_canonical_groups += 1;
                    if changed.is_empty()
                        || changed.iter().any(|f| {
                            f.as_str().and_then(|n| stable.get(n)) != Some(&Stability::Volatile)
                        })
                    {
                        self.error(
                            "same-canonical group changes must be named volatile-only features",
                        )
                    }
                }
                Some("distinct_stable_state") => {
                    self.report.distinct_stable_groups += 1;
                    if !changed.iter().any(|f| {
                        f.as_str()
                            .and_then(|n| stable.get(n))
                            .is_some_and(|s| *s != Stability::Volatile)
                    }) {
                        self.error("distinct-stable group must name a stable feature change")
                    }
                }
                _ => self.error("unknown dedup expected_relation"),
            }
        }
        if self.report.same_canonical_groups == 0 {
            self.error("missing same-canonical dedup evidence")
        }
        if self.report.distinct_stable_groups == 0 {
            self.error("missing distinct-stable dedup evidence")
        }
    }
    fn validation(&mut self) {
        let dir = self.bundle.join("validation");
        let mut passing = 0;
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if !entry.path().is_file() {
                continue;
            }
            if let Ok(v) = read_json(&entry.path()) {
                if v["status"] == "pass" {
                    passing += 1
                }
            }
        }
        if passing == 0 {
            self.error("validation directory contains no passing machine-readable report")
        }
    }
    fn expect(&mut self, v: &Value, key: &str, want: Value) {
        if v.get(key) != Some(&want) {
            self.error(format!("manifest.{key} has invalid value"))
        }
    }
    fn error(&mut self, e: impl Into<String>) {
        if self.report.errors.len() < 200 {
            self.report.errors.push(e.into())
        }
    }
}
fn read_json(path: &Path) -> Result<Value, String> {
    let b = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_slice(&b).map_err(|e| format!("invalid JSON in {}: {e}", path.display()))
}
fn at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(v, |v, k| v.get(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(b: &[u8]) -> String {
        format!("blake3:{}", blake3::hash(b).to_hex())
    }
    fn fixture() -> tempfile::TempDir {
        let r = tempfile::tempdir().unwrap();
        for d in [
            "captures",
            "artifacts/feature-bytes",
            "artifacts/framebuffer",
            "validation",
        ] {
            fs::create_dir_all(r.path().join(d)).unwrap()
        }
        let h = hash(b"x");
        let manifest = serde_json::json!({"schema_version":1,"kind":"phase4-first-room-fallback","scope_version":"first-room-v1","fulfillment_claim":"partial","capture_count":2,"reference_workload_commit":"0123456789012345678901234567890123456789","feature_map_hash":h,"scoring_program_hash":h,"layout_hash":h,"exporter_commit":"0123456789012345678901234567890123456789","coverage":{"first_room":true,"decode_goldens":true,"same_canonical":true,"distinct_stable":true,"volatile_only_change":true,"first_boss":false,"goal_positive":false,"trajectory":false},"private_storage":{"artifact_id":"artifact:fallback","access_requirement":"role:lab","retention":"test"},"follow_on":{"task_id":"task:full-corpus","owner_role":"role:operator"}});
        fs::write(
            r.path().join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(r.path().join("workload-image-ref.txt"), "artifact:image").unwrap();
        fs::write(r.path().join("scoring-program.yaml"), "fixture").unwrap();
        fs::write(r.path().join("layout.json"), "{}").unwrap();
        let map = r#"schema_version: 1
kind: feature-map
meta: {name: private-fixture, workload: operator-workload, game_revision: revision-private, version: 1}
regions: [{name: wram, size: 131072}]
features:
  - {name: stable_room, region: wram, offset: 1, type: u8, semantics: room_id, stability: stable}
  - {name: volatile_timer, region: wram, offset: 2, type: u8, semantics: timer, stability: volatile}
"#;
        fs::write(r.path().join("feature-map.yaml"), map).unwrap();
        let pixels = vec![1u8; 229376];
        let mut lines = String::new();
        for i in 0..2 {
            let feature = vec![i as u8, 7];
            let fb = lz4_flex::compress_prepend_size(&pixels);
            let fr = format!("artifacts/feature-bytes/{i}.bin");
            let br = format!("artifacts/framebuffer/{i}.lz4");
            fs::write(r.path().join(&fr), &feature).unwrap();
            fs::write(r.path().join(&br), &fb).unwrap();
            let row = serde_json::json!({"capture_id":format!("cap-{i}"),"feature_bytes":{"ref":fr,"len":feature.len(),"blake3":hash(&feature)},"framebuffer":{"ref":br,"len":fb.len(),"encoding":"fb_lz4","width":256,"height":224,"stride":1024,"pixel_format":"xrgb8888","uncompressed_len":229376,"blake3":hash(&fb),"uncompressed_blake3":hash(&pixels)}});
            lines.push_str(&serde_json::to_string(&row).unwrap());
            lines.push('\n')
        }
        fs::write(r.path().join("captures/index.jsonl"), lines).unwrap();
        let dedup=[serde_json::json!({"group_id":"same","expected_relation":"same_canonical_state","changed_features":["volatile_timer"]}),serde_json::json!({"group_id":"different","expected_relation":"distinct_stable_state","changed_features":["stable_room"]})].iter().map(|v|serde_json::to_string(v).unwrap()).collect::<Vec<_>>().join("\n");
        fs::write(r.path().join("dedup-groups.jsonl"), format!("{dedup}\n")).unwrap();
        fs::write(
            r.path().join("validation/report.json"),
            r#"{"status":"pass"}"#,
        )
        .unwrap();
        r
    }
    #[test]
    fn phase4_fallback_accepts_separately_typed_bundle() {
        let r = fixture();
        let report = check_phase4_fallback(r.path());
        assert!(report.passed(), "{:?}", report.errors);
        let seal = tempfile::tempdir().unwrap();
        assert!(crate::phase4_checksum_manifest::set_phase4_payload_root(r.path()).passed());
        let frozen = crate::phase4_checksum_manifest::write_phase4_checksum_manifest(
            &crate::phase4_checksum_manifest::ChecksumManifestOptions {
                bundle: r.path().into(),
                out: seal.path().join("freeze.json"),
            },
        );
        assert!(frozen.passed(), "{:?}", frozen.errors);
    }
    #[test]
    fn phase4_fallback_rejects_full_or_fulfilled_claim() {
        let r = fixture();
        let path = r.path().join("manifest.json");
        let mut v: Value = read_json(&path).unwrap();
        v["kind"] = "phase4-scorer-golden".into();
        v["fulfillment_claim"] = "fulfilled".into();
        fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
        let report = check_phase4_fallback(r.path());
        assert!(!report.passed());
        assert!(report.errors.iter().any(|e| e.contains("kind")));
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("fulfillment_claim")))
    }
}
