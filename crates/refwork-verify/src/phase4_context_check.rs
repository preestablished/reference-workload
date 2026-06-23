//! Phase 4 context-smoke fixture bundle validation.
//!
//! The checker validates metadata, decoded feature shape, provenance hashes, and
//! optional padlog syntax. It does not certify that synthetic fixtures are live
//! evidence.

use refwork_script::parse as parse_padlog;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const PAD_LAYOUT_ID: &str = "console16-12btn-v1";

#[derive(Debug, Clone, Serialize, Default)]
pub struct Phase4ContextReport {
    pub schema_version: u32,
    pub command: String,
    pub bundle_root: String,
    pub status: String,
    pub evidence_type: Option<String>,
    pub file_hashes: BTreeMap<String, String>,
    pub context_count: usize,
    pub decoded_feature_count_min: Option<usize>,
    pub decoded_feature_count_max: Option<usize>,
    pub recent_input_available: Option<bool>,
    pub recent_input_padlog_frames: Option<usize>,
    pub errors: Vec<String>,
}

impl Phase4ContextReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn check_phase4_context_bundle(bundle: &Path) -> Phase4ContextReport {
    let mut checker = Checker::new(bundle);
    checker.run();
    checker.finish()
}

struct Checker<'a> {
    bundle: &'a Path,
    report: Phase4ContextReport,
}

impl<'a> Checker<'a> {
    fn new(bundle: &'a Path) -> Self {
        Self {
            bundle,
            report: Phase4ContextReport {
                schema_version: 1,
                command: format!(
                    "refwork-verify phase4-context-check --bundle {}",
                    bundle.display()
                ),
                bundle_root: bundle.display().to_string(),
                status: "fail".to_owned(),
                ..Phase4ContextReport::default()
            },
        }
    }

    fn run(&mut self) {
        if !self.bundle.is_dir() {
            self.error(format!(
                "context bundle root is not a directory: {}",
                self.bundle.display()
            ));
            return;
        }

        let manifest_path = self.require_file("manifest.json");
        let contexts_path = self.require_file("contexts.jsonl");
        let validation_report_path = self.require_file("validation/context-export-report.json");
        let padlog_path = self.optional_file("recent-input.padlog");

        let (manifest_capture_count, manifest_feature_hash, manifest_layout_hash) = manifest_path
            .as_ref()
            .and_then(|path| self.read_json(path, "manifest.json"))
            .map(|json| self.check_manifest(&json))
            .unwrap_or((None, None, None));

        if let Some(path) = &validation_report_path {
            if let Some(json) = self.read_json(path, "validation/context-export-report.json") {
                self.check_validation_report(&json);
            }
        }

        if let Some(path) = &padlog_path {
            self.check_padlog(path);
        }

        if self.report.recent_input_available == Some(true) && padlog_path.is_none() {
            self.error(
                "manifest says recent input is available, but recent-input.padlog is missing",
            );
        }

        if let Some(path) = &contexts_path {
            self.check_contexts(
                path,
                manifest_capture_count,
                manifest_feature_hash.as_deref(),
                manifest_layout_hash.as_deref(),
            );
        }
    }

    fn finish(mut self) -> Phase4ContextReport {
        self.report.status = if self.report.errors.is_empty() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };
        self.report
    }

    fn error(&mut self, msg: impl Into<String>) {
        if self.report.errors.len() < 200 {
            self.report.errors.push(msg.into());
        } else if self.report.errors.len() == 200 {
            self.report
                .errors
                .push("additional errors suppressed after 200 findings".to_owned());
        }
    }

    fn require_file(&mut self, rel: &str) -> Option<PathBuf> {
        let path = self.bundle.join(rel);
        if !path.is_file() {
            self.error(format!("missing required file {rel}"));
            return None;
        }
        self.hash_file(rel, &path);
        Some(path)
    }

    fn optional_file(&mut self, rel: &str) -> Option<PathBuf> {
        let path = self.bundle.join(rel);
        if path.is_file() {
            self.hash_file(rel, &path);
            Some(path)
        } else {
            None
        }
    }

    fn hash_file(&mut self, rel: &str, path: &Path) {
        match fs::read(path) {
            Ok(bytes) => {
                self.report.file_hashes.insert(
                    rel.to_owned(),
                    format!("blake3:{}", blake3::hash(&bytes).to_hex()),
                );
            }
            Err(err) => self.error(format!("cannot read {rel}: {err}")),
        }
    }

    fn read_json(&mut self, path: &Path, label: &str) -> Option<Value> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read {label}: {err}"));
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(json) => Some(json),
            Err(err) => {
                self.error(format!("{label} is not valid JSON: {err}"));
                None
            }
        }
    }

    fn check_manifest(&mut self, json: &Value) -> (Option<usize>, Option<String>, Option<String>) {
        self.require_u64(json, &["schema_version"]);
        self.expect_string(json, &["kind"], "phase4-context-smoke");
        if let Some(evidence_type) = self.require_string(json, &["evidence_type"]) {
            match evidence_type {
                "live" | "synthetic" => {
                    self.report.evidence_type = Some(evidence_type.to_owned());
                }
                other => self.error(format!(
                    "manifest.evidence_type must be live or synthetic, got {other:?}"
                )),
            }
        }
        if let Some(commit) = self.require_string(json, &["reference_workload_commit"]) {
            if !is_40_hex(commit) {
                self.error("manifest.reference_workload_commit must be a 40-hex git commit");
            }
        }
        self.require_any_hash_or_ref(
            json,
            &[
                &["workload_image", "manifest_hash"],
                &["workload_image", "artifact_id"],
            ],
            "workload_image manifest hash or artifact id",
        );
        self.expect_string(json, &["pad_layout", "layout_id"], PAD_LAYOUT_ID);
        self.expect_u64(json, &["pad_layout", "layout_version"], 1);
        self.require_hash_or_ref(json, &["pad_layout", "table_hash"]);
        let feature_hash = self
            .require_hash_or_ref(json, &["feature_map_hash"])
            .map(str::to_owned);
        self.require_hash_or_ref(json, &["scoring_program_hash"]);
        let layout_hash = self.require_any_hash_or_ref(
            json,
            &[&["layout_hash"], &["capture_spec_hash"]],
            "layout hash or capture spec hash",
        );
        let capture_count = self
            .require_u64(json, &["capture_count"])
            .map(|n| n as usize);
        let recent_available = self.require_bool(json, &["recent_input_available"]);
        self.report.recent_input_available = recent_available;
        if recent_available == Some(false) {
            self.require_string(json, &["recent_input_unavailable_reason"]);
        }
        self.require_any_string(
            json,
            &[
                &["private_storage", "artifact_id"],
                &["private_storage", "storage_artifact_id"],
                &["storage_artifact_id"],
            ],
            "private storage artifact id",
        );
        self.require_any_string(
            json,
            &[
                &["private_storage", "access_requirement"],
                &["private_storage", "role"],
                &["role_based_access_requirement"],
            ],
            "role-based access requirement",
        );
        self.require_any_string(
            json,
            &[&["private_storage", "retention"], &["retention"]],
            "retention expectation",
        );
        self.require_string(json, &["clean_room_provenance"]);

        (capture_count, feature_hash, layout_hash)
    }

    fn check_validation_report(&mut self, json: &Value) {
        self.require_u64(json, &["schema_version"]);
        self.require_string(json, &["command"]);
        self.require_hash_or_ref(json, &["workload_image_manifest_hash"]);
        self.require_hash_or_ref(json, &["feature_map_hash"]);
        self.require_u64(json, &["capture_count"]);
        self.expect_string(json, &["status"], "pass");
    }

    fn check_padlog(&mut self, path: &Path) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read recent-input.padlog: {err}"));
                return;
            }
        };
        match parse_padlog(&text) {
            Ok(log) => {
                self.report.recent_input_padlog_frames = Some(log.len());
                if log.is_empty() {
                    self.error("recent-input.padlog must contain at least one frame");
                }
            }
            Err(err) => self.error(format!("recent-input.padlog parse failed: {err}")),
        }
    }

    fn check_contexts(
        &mut self,
        path: &Path,
        manifest_capture_count: Option<usize>,
        manifest_feature_hash: Option<&str>,
        manifest_layout_hash: Option<&str>,
    ) {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(err) => {
                self.error(format!("cannot read contexts.jsonl: {err}"));
                return;
            }
        };

        let mut count = 0usize;
        let mut decoded_min: Option<usize> = None;
        let mut decoded_max: Option<usize> = None;
        for (line_idx, line) in BufReader::new(file).lines().enumerate() {
            let line_no = line_idx + 1;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    self.error(format!("contexts.jsonl:{line_no}: read failed: {err}"));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            count += 1;
            let row: Value = match serde_json::from_str(&line) {
                Ok(row) => row,
                Err(err) => {
                    self.error(format!("contexts.jsonl:{line_no}: invalid JSON: {err}"));
                    continue;
                }
            };
            self.reject_forbidden_payload_fields(&row, &format!("contexts.jsonl:{line_no}"));
            self.require_u64(&row, &["schema_version"]);
            self.require_string(&row, &["capture_id"]);
            self.require_any_string(
                &row,
                &[&["node_ref"], &["source_id"]],
                "context node/source ref",
            );
            if value_at(&row, &["frame_index"]).is_none()
                && value_at(&row, &["frame_counter"]).is_none()
            {
                self.error(format!(
                    "contexts.jsonl:{line_no}: missing frame_index or frame_counter"
                ));
            }
            self.compare_hash(
                &row,
                &["workload_image_manifest_hash"],
                None,
                &format!("contexts.jsonl:{line_no}"),
            );
            self.compare_hash(
                &row,
                &["feature_map_hash"],
                manifest_feature_hash,
                &format!("contexts.jsonl:{line_no}"),
            );
            let row_layout_hash = self
                .compare_hash(
                    &row,
                    &["layout_hash"],
                    manifest_layout_hash,
                    &format!("contexts.jsonl:{line_no}"),
                )
                .or_else(|| {
                    self.compare_hash(
                        &row,
                        &["capture_spec_hash"],
                        manifest_layout_hash,
                        &format!("contexts.jsonl:{line_no}"),
                    )
                });
            if row_layout_hash.is_none() {
                self.error(format!(
                    "contexts.jsonl:{line_no}: missing layout_hash or capture_spec_hash"
                ));
            }

            let order_len = self.array_len(&row, &["decoded_order"]);
            let values_len = self.array_len(&row, &["decoded_values"]);
            if let (Some(order_len), Some(values_len)) = (order_len, values_len) {
                if order_len == 0 {
                    self.error(format!("contexts.jsonl:{line_no}: decoded_order is empty"));
                }
                if order_len != values_len {
                    self.error(format!(
                        "contexts.jsonl:{line_no}: decoded_order len {order_len} != decoded_values len {values_len}"
                    ));
                }
                decoded_min = Some(decoded_min.map_or(order_len, |current| current.min(order_len)));
                decoded_max = Some(decoded_max.map_or(order_len, |current| current.max(order_len)));
            }
            self.require_object(&row, &["decoded_by_name"]);
            self.check_framebuffer(&row, line_no);
            self.check_regions(&row, line_no);
            self.check_recent_input(&row, line_no);
        }

        self.report.context_count = count;
        self.report.decoded_feature_count_min = decoded_min;
        self.report.decoded_feature_count_max = decoded_max;
        if count == 0 {
            self.error("contexts.jsonl must contain at least one context row");
        }
        if let Some(expected) = manifest_capture_count {
            if count != expected {
                self.error(format!(
                    "manifest capture_count {expected} != contexts.jsonl row count {count}"
                ));
            }
        }
    }

    fn check_framebuffer(&mut self, row: &Value, line_no: usize) {
        let label = format!("contexts.jsonl:{line_no}: framebuffer");
        self.require_string(row, &["framebuffer", "encoding"]);
        self.require_u64(row, &["framebuffer", "width"]);
        self.require_u64(row, &["framebuffer", "height"]);
        self.require_u64(row, &["framebuffer", "stride"]);
        self.require_string(row, &["framebuffer", "pixel_format"]);
        self.require_hash_or_ref(row, &["framebuffer", "blake3"]);
        if value_at(row, &["framebuffer", "bytes"]).is_some() {
            self.error(format!("{label} must not contain framebuffer bytes"));
        }
    }

    fn check_regions(&mut self, row: &Value, line_no: usize) {
        let Some(Value::Array(regions)) = value_at(row, &["regions"]) else {
            self.error(format!(
                "contexts.jsonl:{line_no}: regions must be an array"
            ));
            return;
        };
        if regions.is_empty() {
            self.error(format!(
                "contexts.jsonl:{line_no}: regions must not be empty"
            ));
        }
        for (idx, region) in regions.iter().enumerate() {
            self.require_string(region, &["name"]);
            self.require_u64(region, &["size"]);
            self.require_u64(region, &["layout_version"]);
            self.require_hash_or_ref(region, &["blake3"]);
            if value_at(region, &["bytes"]).is_some() {
                self.error(format!(
                    "contexts.jsonl:{line_no}: regions[{idx}] must not contain raw bytes"
                ));
            }
        }
    }

    fn check_recent_input(&mut self, row: &Value, line_no: usize) {
        let Some(recent) = value_at(row, &["recent_input"]) else {
            self.error(format!("contexts.jsonl:{line_no}: missing recent_input"));
            return;
        };
        let Some(available) = self.require_bool(recent, &["available"]) else {
            return;
        };
        if available {
            if value_at(recent, &["padlog_ref"]).is_none() && value_at(recent, &["words"]).is_none()
            {
                self.error(format!(
                    "contexts.jsonl:{line_no}: recent_input available but missing padlog_ref or words"
                ));
            }
            if let Some(Value::Array(words)) = value_at(recent, &["words"]) {
                for (idx, word) in words.iter().enumerate() {
                    match word.as_u64() {
                        Some(value) if value <= 0x0fff => {}
                        Some(value) => self.error(format!(
                            "contexts.jsonl:{line_no}: recent_input.words[{idx}] sets reserved bits: {value:#06x}"
                        )),
                        None => self.error(format!(
                            "contexts.jsonl:{line_no}: recent_input.words[{idx}] must be an integer"
                        )),
                    }
                }
            }
        } else {
            self.require_string(recent, &["reason"]);
        }
    }

    fn reject_forbidden_payload_fields(&mut self, value: &Value, label: &str) {
        let forbidden = [
            "raw_wram",
            "wram_bytes",
            "framebuffer_bytes",
            "screenshot",
            "save_ram",
            "rom_bytes",
        ];
        if let Some(object) = value.as_object() {
            for key in object.keys() {
                if forbidden.contains(&key.as_str()) {
                    self.error(format!("{label}: forbidden private payload field {key:?}"));
                }
            }
        }
    }

    fn require_any_string(
        &mut self,
        value: &Value,
        paths: &[&[&str]],
        label: &str,
    ) -> Option<String> {
        for path in paths {
            if let Some(value) = string_at(value, path) {
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
        self.error(format!("missing {label}"));
        None
    }

    fn require_any_hash_or_ref(
        &mut self,
        value: &Value,
        paths: &[&[&str]],
        label: &str,
    ) -> Option<String> {
        for path in paths {
            if let Some(value) = string_at(value, path) {
                if looks_hash_or_ref(value) {
                    return Some(value.to_owned());
                }
            }
        }
        self.error(format!("missing or invalid {label}"));
        None
    }

    fn require_string<'v>(&mut self, value: &'v Value, path: &[&str]) -> Option<&'v str> {
        match value_at(value, path) {
            Some(Value::String(value)) if !value.is_empty() => Some(value),
            Some(Value::String(_)) => {
                self.error(format!("{} must not be empty", dotted(path)));
                None
            }
            Some(_) => {
                self.error(format!("{} must be a string", dotted(path)));
                None
            }
            None => {
                self.error(format!("missing {}", dotted(path)));
                None
            }
        }
    }

    fn expect_string(&mut self, value: &Value, path: &[&str], expected: &str) {
        if let Some(actual) = self.require_string(value, path) {
            if actual != expected {
                self.error(format!(
                    "{} expected {expected:?}, got {actual:?}",
                    dotted(path)
                ));
            }
        }
    }

    fn require_hash_or_ref<'v>(&mut self, value: &'v Value, path: &[&str]) -> Option<&'v str> {
        let actual = self.require_string(value, path)?;
        if !looks_hash_or_ref(actual) {
            self.error(format!(
                "{} must be a blake3 hash or opaque ref",
                dotted(path)
            ));
            return None;
        }
        Some(actual)
    }

    fn compare_hash<'v>(
        &mut self,
        value: &'v Value,
        path: &[&str],
        expected: Option<&str>,
        label: &str,
    ) -> Option<&'v str> {
        let actual = self.require_hash_or_ref(value, path)?;
        if let Some(expected) = expected {
            if actual != expected {
                self.error(format!(
                    "{label}: {} {actual:?} != manifest {expected:?}",
                    dotted(path)
                ));
            }
        }
        Some(actual)
    }

    fn require_u64(&mut self, value: &Value, path: &[&str]) -> Option<u64> {
        match value_at(value, path) {
            Some(Value::Number(value)) => value.as_u64().or_else(|| {
                self.error(format!("{} must be an unsigned integer", dotted(path)));
                None
            }),
            Some(_) => {
                self.error(format!("{} must be an unsigned integer", dotted(path)));
                None
            }
            None => {
                self.error(format!("missing {}", dotted(path)));
                None
            }
        }
    }

    fn expect_u64(&mut self, value: &Value, path: &[&str], expected: u64) {
        if let Some(actual) = self.require_u64(value, path) {
            if actual != expected {
                self.error(format!(
                    "{} expected {expected}, got {actual}",
                    dotted(path)
                ));
            }
        }
    }

    fn require_bool(&mut self, value: &Value, path: &[&str]) -> Option<bool> {
        match value_at(value, path) {
            Some(Value::Bool(value)) => Some(*value),
            Some(_) => {
                self.error(format!("{} must be a boolean", dotted(path)));
                None
            }
            None => {
                self.error(format!("missing {}", dotted(path)));
                None
            }
        }
    }

    fn require_object(&mut self, value: &Value, path: &[&str]) -> Option<()> {
        match value_at(value, path) {
            Some(Value::Object(_)) => Some(()),
            Some(_) => {
                self.error(format!("{} must be an object", dotted(path)));
                None
            }
            None => {
                self.error(format!("missing {}", dotted(path)));
                None
            }
        }
    }

    fn array_len(&mut self, value: &Value, path: &[&str]) -> Option<usize> {
        match value_at(value, path) {
            Some(Value::Array(values)) => Some(values.len()),
            Some(_) => {
                self.error(format!("{} must be an array", dotted(path)));
                None
            }
            None => {
                self.error(format!("missing {}", dotted(path)));
                None
            }
        }
    }
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for part in path {
        current = current.as_object()?.get(*part)?;
    }
    Some(current)
}

fn string_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    match value_at(value, path) {
        Some(Value::String(value)) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn dotted(path: &[&str]) -> String {
    path.join(".")
}

fn is_40_hex(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn looks_hash_or_ref(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix("blake3:") {
        hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    } else {
        !value.trim().is_empty()
    }
}
