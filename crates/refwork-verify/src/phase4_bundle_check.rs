//! Phase 4 scorer artifact bundle contract validation.
//!
//! This checker validates the source-owned shape of the private scorer bundle
//! without embedding any real capture bytes in this repository. Tests build
//! synthetic bundles only.

use refwork_featuremap::{parse_feature_map, parse_scoring_program, validate_pair, Stability};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MIN_REAL_CAPTURE_COUNT: usize = 1_000;
const PAD_LAYOUT_ID: &str = "console16-12btn-v1";
const PAD_LAYOUT_VERSION: u64 = 1;
const WRAM_BYTES: u64 = 131_072;
const FRAMEBUFFER_BYTES: u64 = 229_376;
const FRAMEBUFFER_FORMAT: &str = "xrgb8888-256x224-stride1024";

#[derive(Debug, Clone, Serialize, Default)]
pub struct Phase4BundleReport {
    pub schema_version: u32,
    pub command: String,
    pub bundle_root: String,
    pub status: String,
    pub reference_workload_commit: Option<String>,
    pub private_bundle_artifact_id: Option<String>,
    pub top_level_file_hashes: BTreeMap<String, String>,
    pub workload_image: WorkloadImageSummary,
    pub validation_files: Vec<String>,
    pub validation_evidence: ValidationEvidenceSummary,
    pub capture_count: usize,
    pub decoded_feature_count_min: Option<usize>,
    pub decoded_feature_count_max: Option<usize>,
    pub framebuffer: FramebufferSummary,
    pub dedup_groups: DedupSummary,
    pub score_plan_batch_ids: Vec<String>,
    pub trajectory_files: Vec<TrajectorySummary>,
    pub errors: Vec<String>,
}

impl Phase4BundleReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WorkloadImageSummary {
    pub source: Option<String>,
    pub regions: BTreeMap<String, WorkloadRegionSummary>,
    pub framebuffer_format: Option<String>,
    pub pad_layout_id: Option<String>,
    pub pad_layout_version: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WorkloadRegionSummary {
    pub size: u64,
    pub layout_version: Option<u64>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ValidationEvidenceSummary {
    pub workload_image: bool,
    pub feature_map_scoring: bool,
    pub map_check_or_region_layout: bool,
    pub trace: bool,
    pub checksum_manifest: bool,
    pub redaction_scan: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FramebufferSummary {
    pub capture_rows_with_metadata: usize,
    pub encoding: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
    pub stride: Option<u64>,
    pub pixel_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DedupSummary {
    pub total_groups: usize,
    pub same_canonical_state: usize,
    pub distinct_stable_state: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TrajectorySummary {
    pub file: String,
    pub blake3: String,
    pub rows: usize,
    pub first_boss_coverage: bool,
    pub goal_positive: bool,
    pub goal_negative: bool,
}

struct FeatureMapInfo {
    order: Vec<String>,
    stability: BTreeMap<String, Stability>,
}

pub fn check_phase4_bundle(bundle: &Path) -> Phase4BundleReport {
    let mut checker = Checker::new(bundle);
    checker.run();
    checker.finish()
}

struct Checker<'a> {
    bundle: &'a Path,
    report: Phase4BundleReport,
}

impl<'a> Checker<'a> {
    fn new(bundle: &'a Path) -> Self {
        Self {
            bundle,
            report: Phase4BundleReport {
                schema_version: 1,
                command: format!(
                    "refwork-verify phase4-bundle-check --bundle {}",
                    bundle.display()
                ),
                bundle_root: bundle.display().to_string(),
                status: "fail".to_owned(),
                ..Phase4BundleReport::default()
            },
        }
    }

    fn run(&mut self) {
        if !self.bundle.is_dir() {
            self.error(format!(
                "bundle root is not a directory: {}",
                self.bundle.display()
            ));
            return;
        }

        let manifest_path = self.require_file("manifest.json");
        let workload_image_yaml = self.optional_file("workload-image.yaml");
        let workload_image_ref = self.optional_file("workload-image-ref.txt");
        if workload_image_yaml.is_none() && workload_image_ref.is_none() {
            self.error("missing workload-image.yaml or workload-image-ref.txt");
        }
        let feature_map_path = self.require_file("feature-map.yaml");
        let scoring_program_path = self.require_file("scoring-program.yaml");
        let layout_path = self.require_file("layout.json");
        let captures_path = self.require_file("captures/index.jsonl");
        let dedup_path = self.require_file("dedup-groups.jsonl");
        let score_plan_path = self.require_file("score-plan.json");
        let validation_dir = self.require_dir("validation");
        let trajectory_dir = self.require_dir("trajectory");

        let manifest_capture_count = manifest_path
            .as_ref()
            .and_then(|path| self.read_json(path, "manifest.json"))
            .and_then(|json| self.check_manifest(&json));

        if let Some(path) = &workload_image_yaml {
            self.check_workload_image_yaml(path);
        }
        if let Some(path) = &workload_image_ref {
            self.check_workload_image_ref(path);
        }

        let mut feature_info = None;
        if let (Some(map_path), Some(scoring_path)) = (&feature_map_path, &scoring_program_path) {
            feature_info = self.check_feature_map_and_scoring(map_path, scoring_path);
        }

        let (layout_total_len, layout_hash) = layout_path
            .as_ref()
            .and_then(|path| self.read_json(path, "layout.json"))
            .map(|json| self.check_layout(&json))
            .unwrap_or((None, None));

        let mut capture_ids = None;
        if let Some(path) = &captures_path {
            capture_ids = Some(self.check_captures(
                path,
                manifest_capture_count,
                layout_total_len,
                layout_hash.as_deref(),
                feature_info.as_ref().map(|info| info.order.as_slice()),
            ));
        }
        if let Some(path) = &dedup_path {
            self.check_dedup_groups(path, capture_ids.as_ref(), feature_info.as_ref());
        }
        if let Some(path) = &score_plan_path {
            if let Some(json) = self.read_json(path, "score-plan.json") {
                self.check_score_plan(&json, capture_ids.as_ref());
            }
        }
        if let Some(path) = &trajectory_dir {
            self.check_trajectories(
                path,
                feature_info.as_ref().map(|info| info.order.as_slice()),
                capture_ids.as_ref(),
            );
        }
        if let Some(path) = &validation_dir {
            self.check_validation_dir(path);
        }
    }

    fn finish(mut self) -> Phase4BundleReport {
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

    fn require_dir(&mut self, rel: &str) -> Option<PathBuf> {
        let path = self.bundle.join(rel);
        if !path.is_dir() {
            self.error(format!("missing required directory {rel}"));
            None
        } else {
            Some(path)
        }
    }

    fn hash_file(&mut self, rel: &str, path: &Path) {
        match fs::read(path) {
            Ok(bytes) => {
                let hash = blake3::hash(&bytes).to_hex().to_string();
                self.report
                    .top_level_file_hashes
                    .insert(rel.to_owned(), format!("blake3:{hash}"));
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

    fn check_workload_image_ref(&mut self, path: &Path) {
        match fs::read_to_string(path) {
            Ok(text) if !text.trim().is_empty() => {
                self.report.workload_image.source = Some("workload-image-ref.txt".to_owned());
            }
            Ok(_) => self.error("workload-image-ref.txt must not be empty"),
            Err(err) => self.error(format!("cannot read workload-image-ref.txt: {err}")),
        }
    }

    fn check_workload_image_yaml(&mut self, path: &Path) {
        self.report.workload_image.source = Some("workload-image.yaml".to_owned());
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read workload-image.yaml: {err}"));
                return;
            }
        };
        let json: Value = match serde_yaml::from_str(&text) {
            Ok(json) => json,
            Err(err) => {
                self.error(format!("workload-image.yaml is not valid YAML: {err}"));
                return;
            }
        };

        self.require_u64(&json, &["schema_version"]);
        self.expect_string(&json, &["kind"], "workload-image");
        self.require_string(&json, &["meta", "name"]);
        self.require_string(&json, &["meta", "version"]);
        self.expect_string(&json, &["pad_layout", "layout_id"], PAD_LAYOUT_ID);
        self.expect_u64(&json, &["pad_layout", "layout_version"], PAD_LAYOUT_VERSION);
        self.report.workload_image.pad_layout_id =
            string_at(&json, &["pad_layout", "layout_id"]).map(str::to_owned);
        self.report.workload_image.pad_layout_version =
            u64_at(&json, &["pad_layout", "layout_version"]);

        let Some(Value::Array(regions)) = value_at(&json, &["regions"]) else {
            self.error("workload-image.yaml regions must be an array");
            return;
        };
        if regions.is_empty() {
            self.error("workload-image.yaml regions must not be empty");
            return;
        }
        for (idx, region) in regions.iter().enumerate() {
            let name = self.require_string(region, &["name"]).map(str::to_owned);
            let size = self.require_u64(region, &["size"]);
            let layout_version = u64_at(region, &["layout_version"]);
            let format = string_at(region, &["format"]).map(str::to_owned);
            if let (Some(name), Some(size)) = (name, size) {
                if self
                    .report
                    .workload_image
                    .regions
                    .insert(
                        name,
                        WorkloadRegionSummary {
                            size,
                            layout_version,
                            format,
                        },
                    )
                    .is_some()
                {
                    self.error(format!(
                        "workload-image.yaml regions[{idx}] duplicates a name"
                    ));
                }
            }
        }
        self.expect_workload_region("wram", WRAM_BYTES, None);
        self.expect_workload_region("framebuffer", FRAMEBUFFER_BYTES, Some(FRAMEBUFFER_FORMAT));
        self.expect_workload_region("meta", 4_096, None);
    }

    fn expect_workload_region(&mut self, name: &str, size: u64, format: Option<&str>) {
        let Some(region) = self.report.workload_image.regions.get(name).cloned() else {
            self.error(format!(
                "workload-image.yaml missing required region {name:?}"
            ));
            return;
        };
        if region.size != size {
            self.error(format!(
                "workload-image.yaml region {name:?} size {} != expected {size}",
                region.size
            ));
        }
        if let Some(expected) = format {
            match region.format.as_deref() {
                Some(actual) if actual == expected => {
                    self.report.workload_image.framebuffer_format = Some(actual.to_owned());
                }
                Some(actual) => self.error(format!(
                    "workload-image.yaml region {name:?} format {actual:?} != expected {expected:?}"
                )),
                None => self.error(format!(
                    "workload-image.yaml region {name:?} missing framebuffer format"
                )),
            }
        }
    }

    fn check_manifest(&mut self, json: &Value) -> Option<usize> {
        self.require_u64(json, &["schema_version"]);
        if let Some(commit) = self.require_string(json, &["reference_workload_commit"]) {
            if !is_40_hex(commit) {
                self.error("manifest.reference_workload_commit must be a 40-hex git commit");
            }
            self.report.reference_workload_commit = Some(commit.to_owned());
        }

        self.require_any_string(
            json,
            &[&["workload_image", "identity"], &["workload_image", "name"]],
            "workload_image identity/name",
        );
        self.require_any_string(
            json,
            &[
                &["workload_image", "revision"],
                &["workload_image", "version"],
            ],
            "workload_image revision/version",
        );
        self.require_any_string(
            json,
            &[
                &["workload_image", "private_artifact_id"],
                &["workload_image", "artifact_id"],
                &["workload_image", "private_ref"],
            ],
            "workload_image private artifact id/ref",
        );
        self.require_any_hash(
            json,
            &[
                &["workload_image_manifest_hash"],
                &["workload_image", "manifest_hash"],
            ],
            "workload image manifest hash",
        );
        self.require_any_string(
            json,
            &[
                &["workload_image", "validation_stamp"],
                &["image_validation_stamp"],
                &["determinism_last_green"],
            ],
            "image validation stamp",
        );
        self.expect_any_string(
            json,
            &[
                &["workload_image", "pad_layout_id"],
                &["pad_layout", "layout_id"],
            ],
            PAD_LAYOUT_ID,
            "pad layout id",
        );
        self.require_any_string(
            json,
            &[
                &["operator_metadata_policy"],
                &["game_revision_metadata"],
                &["rom_revision_metadata"],
            ],
            "operator metadata policy",
        );
        self.require_hash(json, &["feature_map_hash"]);
        self.require_hash(json, &["scoring_program_hash"]);
        self.require_any_hash(
            json,
            &[&["layout_hash"], &["layout_evidence_ref"]],
            "layout hash/evidence ref",
        );
        self.require_any_hash(
            json,
            &[&["bundle_checksum"], &["checksum_manifest_hash"]],
            "bundle checksum/checksum manifest hash",
        );

        let capture_count = self
            .require_u64(json, &["capture_count"])
            .map(|n| n as usize);

        for path in [
            &["framebuffer_format", "encoding"][..],
            &["framebuffer_format", "pixel_format"][..],
        ] {
            self.require_string(json, path);
        }
        for path in [
            &["framebuffer_format", "width"][..],
            &["framebuffer_format", "height"][..],
            &["framebuffer_format", "stride"][..],
            &["framebuffer_format", "uncompressed_len"][..],
        ] {
            self.require_u64(json, path);
        }

        let artifact_id = self.require_any_string(
            json,
            &[
                &["private_storage", "artifact_id"],
                &["private_storage", "location_ref"],
            ],
            "private storage artifact/location ref",
        );
        self.report.private_bundle_artifact_id = artifact_id;
        self.require_any_string(
            json,
            &[
                &["private_storage", "access_requirement"],
                &["private_storage", "access_group"],
            ],
            "private storage access requirement/group",
        );
        self.require_string(json, &["private_storage", "retention"]);
        self.require_string(json, &["private_storage", "compression_format"]);
        self.require_u64(json, &["private_storage", "max_expected_size_bytes"]);
        self.require_string(json, &["clean_room_provenance"]);

        capture_count
    }

    fn check_feature_map_and_scoring(
        &mut self,
        map_path: &Path,
        scoring_path: &Path,
    ) -> Option<FeatureMapInfo> {
        let map_text = match fs::read_to_string(map_path) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read feature-map.yaml: {err}"));
                return None;
            }
        };
        let scoring_text = match fs::read_to_string(scoring_path) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read scoring-program.yaml: {err}"));
                return None;
            }
        };

        reject_placeholder_markers(self, "feature-map.yaml", &map_text);
        reject_placeholder_markers(self, "scoring-program.yaml", &scoring_text);

        let (map, map_errors) = match parse_feature_map(&map_text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.error(format!("feature-map.yaml parse failed: {err}"));
                return None;
            }
        };
        for err in map_errors {
            self.error(format!("feature-map.yaml validation: {err}"));
        }
        if !map
            .features
            .iter()
            .any(|feature| feature.stability == Stability::Stable)
        {
            self.error("feature-map.yaml must mark at least one feature stability: stable");
        }

        let (scoring, scoring_errors) = match parse_scoring_program(&scoring_text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.error(format!("scoring-program.yaml parse failed: {err}"));
                return Some(feature_map_info(&map));
            }
        };
        self.reject_scorer_owned_novelty_extensions(&scoring_text);
        for err in scoring_errors {
            self.error(format!("scoring-program.yaml validation: {err}"));
        }
        for err in validate_pair(&map, &scoring) {
            self.error(format!("feature-map/scoring validation: {err}"));
        }
        if !scoring
            .stages
            .list
            .iter()
            .any(|stage| stage.name == "first_boss")
        {
            self.error("scoring-program.yaml must include a first_boss stage");
        }

        Some(feature_map_info(&map))
    }

    fn reject_scorer_owned_novelty_extensions(&mut self, scoring_text: &str) {
        let Ok(json) = serde_yaml::from_str::<Value>(scoring_text) else {
            return;
        };
        for field in ["novelty", "visual_novelty", "scorer_novelty"] {
            if value_at(&json, &[field]).is_some() {
                self.error(format!(
                    "scoring-program.yaml must not include scorer-owned top-level field {field:?}"
                ));
            }
        }
    }

    fn check_layout(&mut self, json: &Value) -> (Option<u64>, Option<String>) {
        let total_len = self.require_u64(json, &["total_len"]);
        if total_len == Some(0) {
            self.error("layout.json total_len must be greater than zero");
        }
        let layout_hash = self.require_hash(json, &["blake3"]).map(str::to_owned);
        self.require_hash(json, &["compiled_from_feature_map_hash"]);
        self.require_hash(json, &["capture_spec_hash"]);
        self.require_string(json, &["compiler_or_exporter_commit"]);
        match value_at(json, &["ranges"]) {
            Some(Value::Array(ranges)) if !ranges.is_empty() => {
                for (idx, range) in ranges.iter().enumerate() {
                    self.require_string(range, &["region"]);
                    self.require_u64(range, &["layout_version"]);
                    self.require_u64(range, &["offset"]);
                    self.require_u64(range, &["len"]);
                    if self.require_u64(range, &["len"]) == Some(0) {
                        self.error(format!("layout.json ranges[{idx}].len must be > 0"));
                    }
                }
            }
            Some(Value::Array(_)) => self.error("layout.json ranges must not be empty"),
            Some(_) => self.error("layout.json ranges must be an array"),
            None => self.error("missing layout.json ranges"),
        }
        (total_len, layout_hash)
    }

    fn check_captures(
        &mut self,
        path: &Path,
        manifest_capture_count: Option<usize>,
        layout_total_len: Option<u64>,
        layout_hash: Option<&str>,
        feature_order: Option<&[String]>,
    ) -> HashSet<String> {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(err) => {
                self.error(format!("cannot read captures/index.jsonl: {err}"));
                return HashSet::new();
            }
        };

        let mut ids = HashSet::new();
        let mut count = 0usize;
        let mut decoded_min: Option<usize> = None;
        let mut decoded_max: Option<usize> = None;
        let mut fb_summary = FramebufferSummary::default();

        for (line_idx, line) in BufReader::new(file).lines().enumerate() {
            let line_no = line_idx + 1;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    self.error(format!(
                        "captures/index.jsonl:{line_no}: read failed: {err}"
                    ));
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
                    self.error(format!(
                        "captures/index.jsonl:{line_no}: invalid JSON: {err}"
                    ));
                    continue;
                }
            };
            self.reject_forbidden_capture_payload_fields(
                &row,
                &format!("captures/index.jsonl:{line_no}"),
            );

            if let Some(id) = self.require_string(&row, &["capture_id"]) {
                if !ids.insert(id.to_owned()) {
                    self.error(format!(
                        "captures/index.jsonl:{line_no}: duplicate capture_id {id}"
                    ));
                }
            }
            self.require_any_string(
                &row,
                &[&["node_ref"], &["source_id"]],
                "capture node/source ref",
            );
            self.require_string(&row, &["capture_source"]);
            if value_at(&row, &["frame_index"]).is_none()
                && value_at(&row, &["frame_counter"]).is_none()
            {
                self.error(format!(
                    "captures/index.jsonl:{line_no}: missing frame_index or frame_counter"
                ));
            }
            if let Some(expected) = layout_hash {
                if let Some(actual) = self.require_hash(&row, &["layout_hash"]) {
                    if actual != expected {
                        self.error(format!(
                            "captures/index.jsonl:{line_no}: layout_hash {actual:?} != layout.json blake3 {expected:?}"
                        ));
                    }
                }
            } else {
                self.require_hash(&row, &["layout_hash"]);
            }

            self.require_string(&row, &["feature_bytes", "ref"]);
            if value_at(&row, &["feature_bytes", "bytes"]).is_some() {
                self.error(format!(
                    "captures/index.jsonl:{line_no}: feature_bytes must use private refs, not inline bytes"
                ));
            }
            if let Some(actual_len) = self.require_u64(&row, &["feature_bytes", "len"]) {
                if let Some(expected_len) = layout_total_len {
                    if actual_len != expected_len {
                        self.error(format!(
                            "captures/index.jsonl:{line_no}: feature_bytes.len {actual_len} != layout total_len {expected_len}"
                        ));
                    }
                }
            }
            self.require_hash(&row, &["feature_bytes", "blake3"]);

            let decoded_order = self.string_array(&row, &["decoded_order"]);
            let decoded_values_len = self.array_len(&row, &["decoded_values"]);
            if let (Some(decoded_order), Some(values_len)) = (decoded_order, decoded_values_len) {
                let order_len = decoded_order.len();
                if order_len == 0 {
                    self.error(format!(
                        "captures/index.jsonl:{line_no}: decoded_order must not be empty"
                    ));
                }
                if order_len != values_len {
                    self.error(format!(
                        "captures/index.jsonl:{line_no}: decoded_order len {order_len} != decoded_values len {values_len}"
                    ));
                }
                if let Some(expected_order) = feature_order {
                    if decoded_order != expected_order {
                        self.error(format!(
                            "captures/index.jsonl:{line_no}: decoded_order must match feature-map order"
                        ));
                    }
                }
                decoded_min = Some(decoded_min.map_or(order_len, |current| current.min(order_len)));
                decoded_max = Some(decoded_max.map_or(order_len, |current| current.max(order_len)));
            }

            let fb_path = &["framebuffer"][..];
            self.require_string(&row, &[fb_path[0], "ref"]);
            self.require_hash(&row, &[fb_path[0], "blake3"]);
            if value_at(&row, &[fb_path[0], "bytes"]).is_some() {
                self.error(format!(
                    "captures/index.jsonl:{line_no}: framebuffer must use private refs, not inline bytes"
                ));
            }
            let encoding = self
                .require_string(&row, &[fb_path[0], "encoding"])
                .map(str::to_owned);
            let pixel_format = self
                .require_string(&row, &[fb_path[0], "pixel_format"])
                .map(str::to_owned);
            let width = self.require_u64(&row, &[fb_path[0], "width"]);
            let height = self.require_u64(&row, &[fb_path[0], "height"]);
            let stride = self.require_u64(&row, &[fb_path[0], "stride"]);
            self.require_u64(&row, &[fb_path[0], "uncompressed_len"]);
            if encoding.is_some()
                && pixel_format.is_some()
                && width.is_some()
                && height.is_some()
                && stride.is_some()
            {
                fb_summary.capture_rows_with_metadata += 1;
                fb_summary.encoding.get_or_insert_with(|| encoding.unwrap());
                fb_summary
                    .pixel_format
                    .get_or_insert_with(|| pixel_format.unwrap());
                fb_summary.width.get_or_insert(width.unwrap());
                fb_summary.height.get_or_insert(height.unwrap());
                fb_summary.stride.get_or_insert(stride.unwrap());
            }
        }

        self.report.capture_count = count;
        self.report.decoded_feature_count_min = decoded_min;
        self.report.decoded_feature_count_max = decoded_max;
        self.report.framebuffer = fb_summary;

        if count < MIN_REAL_CAPTURE_COUNT {
            self.error(format!(
                "captures/index.jsonl has {count} captures, expected at least {MIN_REAL_CAPTURE_COUNT}"
            ));
        }
        if let Some(expected) = manifest_capture_count {
            if count != expected {
                self.error(format!(
                    "manifest capture_count {expected} != captures/index.jsonl count {count}"
                ));
            }
        }
        ids
    }

    fn check_validation_dir(&mut self, dir: &Path) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                self.error(format!("cannot read validation directory: {err}"));
                return;
            }
        };

        let mut files = Vec::new();
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
        files.sort();

        if files.is_empty() {
            self.error("validation directory must contain validation report files");
            return;
        }

        let mut evidence = ValidationEvidenceSummary::default();
        for path in files {
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let rel = format!("validation/{file_name}");
            self.hash_file(&rel, &path);
            self.report.validation_files.push(rel.clone());
            let lower = file_name.to_ascii_lowercase();
            evidence.workload_image |= lower.contains("workload-image")
                || lower.contains("image-validation")
                || lower.contains("determinism.last_green");
            evidence.map_check_or_region_layout |= lower.contains("map-check")
                || lower.contains("region-layout")
                || lower.contains("layout-validation");
            evidence.feature_map_scoring |= lower.contains("feature")
                || lower.contains("scoring")
                || lower.contains("validate");
            evidence.trace |= lower.contains("trace");
            evidence.checksum_manifest |= lower.contains("checksum")
                || lower.contains("hash-manifest")
                || lower.contains("top-level-hash");
            evidence.redaction_scan |= lower.contains("redaction");

            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Some(json) = self.read_json(&path, &rel) {
                    self.require_string(&json, &["command"]);
                    match self.require_string(&json, &["status"]) {
                        Some("pass") => {}
                        Some(other) => {
                            self.error(format!("{rel} status expected \"pass\", got {other:?}"))
                        }
                        None => {}
                    }
                }
            }
        }
        self.report.validation_evidence = evidence;

        if !self.report.validation_evidence.workload_image {
            self.error("validation directory must include WorkloadImage validation evidence");
        }
        if !self.report.validation_evidence.feature_map_scoring {
            self.error("validation directory must include feature-map/scoring validation evidence");
        }
        if !self.report.validation_evidence.map_check_or_region_layout {
            self.error("validation directory must include map-check or region-layout evidence");
        }
        if !self.report.validation_evidence.trace {
            self.error("validation directory must include trace report evidence");
        }
        if !self.report.validation_evidence.checksum_manifest {
            self.error(
                "validation directory must include checksum manifest or top-level hash evidence",
            );
        }
        if !self.report.validation_evidence.redaction_scan {
            self.error("validation directory must include redaction scan evidence");
        }
    }

    fn reject_forbidden_capture_payload_fields(&mut self, value: &Value, label: &str) {
        let forbidden = [
            "raw_wram",
            "wram_bytes",
            "framebuffer_bytes",
            "screenshot",
            "save_ram",
            "rom_bytes",
            "raw_capture_bytes",
        ];
        if let Some(object) = value.as_object() {
            for key in object.keys() {
                if forbidden.contains(&key.as_str()) {
                    self.error(format!("{label}: forbidden inline payload field {key:?}"));
                }
            }
        }
    }

    fn check_dedup_groups(
        &mut self,
        path: &Path,
        capture_ids: Option<&HashSet<String>>,
        feature_info: Option<&FeatureMapInfo>,
    ) {
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(err) => {
                self.error(format!("cannot read dedup-groups.jsonl: {err}"));
                return;
            }
        };
        let mut summary = DedupSummary::default();
        for (line_idx, line) in BufReader::new(file).lines().enumerate() {
            let line_no = line_idx + 1;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    self.error(format!("dedup-groups.jsonl:{line_no}: read failed: {err}"));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            summary.total_groups += 1;
            let row: Value = match serde_json::from_str(&line) {
                Ok(row) => row,
                Err(err) => {
                    self.error(format!("dedup-groups.jsonl:{line_no}: invalid JSON: {err}"));
                    continue;
                }
            };
            self.require_string(&row, &["group_id"]);
            let relation = self.require_string(&row, &["expected_relation"]);
            self.reject_forbidden_dedup_hash_fields(&row, &format!("dedup-groups.jsonl:{line_no}"));
            let changed_features = self.optional_string_array(&row, &["changed_features"]);
            let changed_offsets = self.optional_array_len(&row, &["changed_offset_ranges"]);
            if changed_features.as_ref().map_or(true, Vec::is_empty)
                && changed_offsets.unwrap_or(0) == 0
            {
                self.error(format!(
                    "dedup-groups.jsonl:{line_no}: expected changed_features or changed_offset_ranges"
                ));
            }
            match relation {
                Some("same_canonical_state") => {
                    summary.same_canonical_state += 1;
                    self.check_same_canonical_changed_features(
                        line_no,
                        changed_features.as_deref(),
                        feature_info,
                    );
                }
                Some("distinct_stable_state") => {
                    summary.distinct_stable_state += 1;
                    self.check_distinct_stable_changed_features(
                        line_no,
                        changed_features.as_deref(),
                        feature_info,
                    );
                }
                Some(other) => self.error(format!(
                    "dedup-groups.jsonl:{line_no}: unknown expected_relation {other:?}"
                )),
                None => {}
            }
            match self.string_array(&row, &["capture_ids"]) {
                Some(ids) if ids.len() >= 2 => {
                    self.check_capture_refs(
                        &ids,
                        capture_ids,
                        &format!("dedup-groups.jsonl:{line_no}: capture_ids"),
                    );
                }
                Some(ids) => self.error(format!(
                    "dedup-groups.jsonl:{line_no}: capture_ids needs at least 2 entries, got {}",
                    ids.len()
                )),
                None => {}
            }
        }
        if summary.same_canonical_state == 0 {
            self.error("dedup-groups.jsonl must contain a same_canonical_state group");
        }
        if summary.distinct_stable_state == 0 {
            self.error("dedup-groups.jsonl must contain a distinct_stable_state group");
        }
        self.report.dedup_groups = summary;
    }

    fn reject_forbidden_dedup_hash_fields(&mut self, value: &Value, label: &str) {
        for field in [
            "canonical_hash",
            "state_hash",
            "scorer_hash",
            "archive_hash",
            "precomputed_hash",
        ] {
            if value_at(value, &[field]).is_some() {
                self.error(format!(
                    "{label}: dedup labels must not include precomputed scorer hash field {field:?}"
                ));
            }
        }
    }

    fn check_same_canonical_changed_features(
        &mut self,
        line_no: usize,
        changed_features: Option<&[String]>,
        feature_info: Option<&FeatureMapInfo>,
    ) {
        let Some(features) = changed_features else {
            return;
        };
        let Some(info) = feature_info else {
            return;
        };
        for feature in features {
            match info.stability.get(feature) {
                Some(Stability::Volatile) => {}
                Some(Stability::Stable) => self.error(format!(
                    "dedup-groups.jsonl:{line_no}: same_canonical_state changed feature {feature:?} is stable"
                )),
                None => self.error(format!(
                    "dedup-groups.jsonl:{line_no}: changed feature {feature:?} is not in feature-map.yaml"
                )),
            }
        }
    }

    fn check_distinct_stable_changed_features(
        &mut self,
        line_no: usize,
        changed_features: Option<&[String]>,
        feature_info: Option<&FeatureMapInfo>,
    ) {
        let Some(features) = changed_features else {
            return;
        };
        let Some(info) = feature_info else {
            return;
        };
        let mut saw_stable = false;
        for feature in features {
            match info.stability.get(feature) {
                Some(Stability::Stable) => saw_stable = true,
                Some(Stability::Volatile) => {}
                None => self.error(format!(
                    "dedup-groups.jsonl:{line_no}: changed feature {feature:?} is not in feature-map.yaml"
                )),
            }
        }
        if !features.is_empty() && !saw_stable {
            self.error(format!(
                "dedup-groups.jsonl:{line_no}: distinct_stable_state must name at least one stable changed feature"
            ));
        }
    }

    fn check_score_plan(&mut self, json: &Value, capture_ids: Option<&HashSet<String>>) {
        self.require_u64(json, &["schema_version"]);
        let Some(Value::Array(batches)) = value_at(json, &["batches"]) else {
            self.error("score-plan.json batches must be an array");
            return;
        };
        if batches.is_empty() {
            self.error("score-plan.json batches must not be empty");
        }
        let mut batch_ids = Vec::new();
        let mut seen_batch_ids = HashSet::new();
        for (idx, batch) in batches.iter().enumerate() {
            if let Some(batch_id) = self.require_string(batch, &["client_batch_id"]) {
                if !seen_batch_ids.insert(batch_id.to_owned()) {
                    self.error(format!(
                        "score-plan.json batches[{idx}].client_batch_id {batch_id:?} is duplicated"
                    ));
                }
                batch_ids.push(batch_id.to_owned());
            }
            match self.string_array(batch, &["capture_ids"]) {
                Some(ids) if ids.len() == 32 => {
                    self.check_duplicate_strings(
                        &ids,
                        &format!("score-plan.json batches[{idx}].capture_ids"),
                    );
                    self.check_capture_refs(
                        &ids,
                        capture_ids,
                        &format!("score-plan.json batches[{idx}].capture_ids"),
                    );
                }
                Some(len) => self.error(format!(
                    "score-plan.json batches[{idx}].capture_ids has {} entries, expected K=32",
                    len.len()
                )),
                None => {}
            }
        }
        self.report.score_plan_batch_ids = batch_ids;
        let checkpoint_after_batch = self
            .require_string(json, &["checkpoint_after_batch"])
            .map(str::to_owned);
        if let Some(batch_id) = checkpoint_after_batch {
            self.check_batch_ref(
                &batch_id,
                &seen_batch_ids,
                "score-plan.json checkpoint_after_batch",
            );
        }
        match self.string_array(json, &["restore_control_batch_ids"]) {
            Some(ids) if !ids.is_empty() => {
                self.check_duplicate_strings(&ids, "score-plan.json restore_control_batch_ids");
                for batch_id in ids {
                    self.check_batch_ref(
                        &batch_id,
                        &seen_batch_ids,
                        "score-plan.json restore_control_batch_ids",
                    );
                }
            }
            Some(_) => self.error("score-plan.json restore_control_batch_ids must not be empty"),
            None => {}
        }
        for label in ["first_boss", "goal_positive", "goal_negative"] {
            match self.string_array(json, &["labels", label]) {
                Some(ids) if !ids.is_empty() => {
                    self.check_capture_refs(
                        &ids,
                        capture_ids,
                        &format!("score-plan.json labels.{label}"),
                    );
                }
                Some(_) => self.error(format!("score-plan.json labels.{label} must not be empty")),
                None => {}
            }
        }
    }

    fn check_batch_ref(&mut self, batch_id: &str, batch_ids: &HashSet<String>, label: &str) {
        if !batch_ids.contains(batch_id) {
            self.error(format!(
                "{label} references unknown client_batch_id {batch_id:?}"
            ));
        }
    }

    fn check_duplicate_strings(&mut self, values: &[String], label: &str) {
        let mut seen = HashSet::new();
        for value in values {
            if !seen.insert(value) {
                self.error(format!("{label} contains duplicate value {value:?}"));
            }
        }
    }

    fn check_trajectories(
        &mut self,
        dir: &Path,
        feature_order: Option<&[String]>,
        capture_ids: Option<&HashSet<String>>,
    ) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                self.error(format!("cannot read trajectory directory: {err}"));
                return;
            }
        };

        let mut paths = Vec::new();
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                paths.push(path);
            }
        }
        paths.sort();

        if paths.is_empty() {
            self.error("trajectory directory must contain at least one .jsonl file");
            return;
        }

        let mut any_first_boss = false;
        let mut any_goal_positive = false;
        let mut any_goal_negative = false;
        for path in paths {
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let hash = match fs::read(&path) {
                Ok(bytes) => format!("blake3:{}", blake3::hash(&bytes).to_hex()),
                Err(err) => {
                    self.error(format!("cannot read trajectory/{file_name}: {err}"));
                    continue;
                }
            };
            self.report
                .top_level_file_hashes
                .insert(format!("trajectory/{file_name}"), hash.clone());
            let file = match fs::File::open(&path) {
                Ok(file) => file,
                Err(err) => {
                    self.error(format!("cannot open trajectory/{file_name}: {err}"));
                    continue;
                }
            };
            let mut summary = TrajectorySummary {
                file: format!("trajectory/{file_name}"),
                blake3: hash,
                ..TrajectorySummary::default()
            };
            for (line_idx, line) in BufReader::new(file).lines().enumerate() {
                let line_no = line_idx + 1;
                let line = match line {
                    Ok(line) => line,
                    Err(err) => {
                        self.error(format!(
                            "trajectory/{file_name}:{line_no}: read failed: {err}"
                        ));
                        continue;
                    }
                };
                if line.trim().is_empty() {
                    continue;
                }
                summary.rows += 1;
                let row: Value = match serde_json::from_str(&line) {
                    Ok(row) => row,
                    Err(err) => {
                        self.error(format!(
                            "trajectory/{file_name}:{line_no}: invalid JSON: {err}"
                        ));
                        continue;
                    }
                };
                self.require_u64(&row, &["frame_index"]);
                if let Some(capture_id) = self.require_string(&row, &["capture_id"]) {
                    self.check_capture_refs(
                        &[capture_id.to_owned()],
                        capture_ids,
                        &format!("trajectory/{file_name}:{line_no}: capture_id"),
                    );
                }
                let decoded_order = self.string_array(&row, &["decoded_order"]);
                let value_len = self.array_len(&row, &["decoded_values"]);
                if let (Some(decoded_order), Some(value_len)) = (decoded_order, value_len) {
                    let order_len = decoded_order.len();
                    if order_len != value_len {
                        self.error(format!(
                            "trajectory/{file_name}:{line_no}: decoded_order len {order_len} != decoded_values len {value_len}"
                        ));
                    }
                    if let Some(expected_order) = feature_order {
                        if decoded_order != expected_order {
                            self.error(format!(
                                "trajectory/{file_name}:{line_no}: decoded_order must match feature-map order"
                            ));
                        }
                    }
                }
                self.array_len(&row, &["active_stages"]);
                if value_at(&row, &["expected_highest_stage"]).is_none()
                    && value_at(&row, &["expected_highest_stage_index"]).is_none()
                {
                    self.error(format!(
                        "trajectory/{file_name}:{line_no}: missing expected_highest_stage or expected_highest_stage_index"
                    ));
                }
                self.require_bool(&row, &["prune"]);
                if let Some(goal) = self.require_bool(&row, &["goal"]) {
                    if goal {
                        summary.goal_positive = true;
                    } else {
                        summary.goal_negative = true;
                    }
                }
                if self.require_bool(&row, &["first_boss_coverage"]) == Some(true) {
                    summary.first_boss_coverage = true;
                }
            }
            if summary.rows == 0 {
                self.error(format!("trajectory/{file_name} must not be empty"));
            }
            any_first_boss |= summary.first_boss_coverage;
            any_goal_positive |= summary.goal_positive;
            any_goal_negative |= summary.goal_negative;
            self.report.trajectory_files.push(summary);
        }

        if !any_first_boss {
            self.error("trajectory labels must include first_boss_coverage: true");
        }
        if !any_goal_positive {
            self.error("trajectory labels must include at least one goal: true row");
        }
        if !any_goal_negative {
            self.error("trajectory labels must include at least one goal: false row");
        }
    }

    fn check_capture_refs(
        &mut self,
        refs: &[String],
        capture_ids: Option<&HashSet<String>>,
        label: &str,
    ) {
        let Some(capture_ids) = capture_ids else {
            return;
        };
        for capture_id in refs {
            if !capture_ids.contains(capture_id) {
                self.error(format!(
                    "{label} references unknown capture_id {capture_id:?}"
                ));
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

    fn require_any_hash(
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

    fn expect_any_string(&mut self, value: &Value, paths: &[&[&str]], expected: &str, label: &str) {
        for path in paths {
            if let Some(value) = string_at(value, path) {
                if value == expected {
                    return;
                }
                self.error(format!("{label} expected {expected:?}, got {value:?}"));
                return;
            }
        }
        self.error(format!("missing {label}"));
    }

    fn expect_string(&mut self, value: &Value, path: &[&str], expected: &str) {
        match self.require_string(value, path) {
            Some(actual) if actual == expected => {}
            Some(actual) => self.error(format!(
                "{} expected {expected:?}, got {actual:?}",
                dotted(path)
            )),
            None => {}
        }
    }

    fn expect_u64(&mut self, value: &Value, path: &[&str], expected: u64) {
        match self.require_u64(value, path) {
            Some(actual) if actual == expected => {}
            Some(actual) => self.error(format!(
                "{} expected {expected}, got {actual}",
                dotted(path)
            )),
            None => {}
        }
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

    fn require_hash<'v>(&mut self, value: &'v Value, path: &[&str]) -> Option<&'v str> {
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

    fn optional_array_len(&mut self, value: &Value, path: &[&str]) -> Option<usize> {
        if value_at(value, path).is_none() {
            return None;
        }
        self.array_len(value, path)
    }

    fn optional_string_array(&mut self, value: &Value, path: &[&str]) -> Option<Vec<String>> {
        if value_at(value, path).is_none() {
            return None;
        }
        self.string_array(value, path)
    }

    fn string_array(&mut self, value: &Value, path: &[&str]) -> Option<Vec<String>> {
        match value_at(value, path) {
            Some(Value::Array(values)) => {
                let mut out = Vec::with_capacity(values.len());
                for (idx, value) in values.iter().enumerate() {
                    match value {
                        Value::String(value) if !value.is_empty() => out.push(value.clone()),
                        Value::String(_) => {
                            self.error(format!("{}[{idx}] must not be empty", dotted(path)));
                            return None;
                        }
                        _ => {
                            self.error(format!("{}[{idx}] must be a string", dotted(path)));
                            return None;
                        }
                    }
                }
                Some(out)
            }
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

fn reject_placeholder_markers(checker: &mut Checker<'_>, label: &str, text: &str) {
    let lower = text.to_ascii_lowercase();
    for marker in [
        "placeholder",
        "illustrative values",
        "not validated game addresses",
    ] {
        if lower.contains(marker) {
            checker.error(format!(
                "{label} contains marker {marker:?}; Phase 4 golden bundle must use real operator-validated data"
            ));
            return;
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

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    match value_at(value, path) {
        Some(Value::Number(value)) => value.as_u64(),
        _ => None,
    }
}

fn feature_map_info(map: &refwork_featuremap::FeatureMap) -> FeatureMapInfo {
    FeatureMapInfo {
        order: map
            .features
            .iter()
            .map(|feature| feature.name.clone())
            .collect(),
        stability: map
            .features
            .iter()
            .map(|feature| (feature.name.clone(), feature.stability.clone()))
            .collect(),
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
