//! Deterministic Phase 4 layout.json exporter.
//!
//! The exporter compiles the feature-map byte layout used by private
//! `feature_bytes` records. Reports and console output do not include local
//! paths or private capture data.

use refwork_featuremap::{parse_feature_map, Feature, FeatureMap, FeatureType};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LayoutOptions {
    pub map: PathBuf,
    pub out: PathBuf,
    pub capture_spec_hash: String,
    pub layout_version: u64,
    pub compiler_or_exporter_commit: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct LayoutReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub range_count: usize,
    pub total_len: u64,
    pub compiled_from_feature_map_hash: Option<String>,
    pub capture_spec_hash: Option<String>,
    pub compiler_or_exporter_commit: Option<String>,
    pub layout_hash: Option<String>,
    pub output_hash: Option<String>,
    pub errors: Vec<String>,
}

impl LayoutReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
struct LayoutRange {
    region: String,
    layout_version: u64,
    offset: u64,
    len: u64,
}

pub fn write_phase4_layout(opts: &LayoutOptions) -> LayoutReport {
    let mut writer = Writer::new(opts);
    writer.run();
    writer.finish()
}

struct Writer<'a> {
    opts: &'a LayoutOptions,
    report: LayoutReport,
}

impl<'a> Writer<'a> {
    fn new(opts: &'a LayoutOptions) -> Self {
        Self {
            opts,
            report: LayoutReport {
                schema_version: 1,
                command: "refwork-verify phase4-layout --map <redacted> --out <redacted>"
                    .to_owned(),
                status: "fail".to_owned(),
                ..LayoutReport::default()
            },
        }
    }

    fn run(&mut self) {
        if self.opts.layout_version == 0 {
            self.error("layout version must be greater than zero");
        }
        if !looks_hash_or_ref(&self.opts.capture_spec_hash) {
            self.error("capture spec hash must be a blake3 hash or opaque ref");
        } else {
            self.report.capture_spec_hash = Some(self.opts.capture_spec_hash.clone());
        }
        if self.opts.compiler_or_exporter_commit.trim().is_empty() {
            self.error("compiler_or_exporter_commit must not be empty");
        } else {
            self.report.compiler_or_exporter_commit =
                Some(self.opts.compiler_or_exporter_commit.clone());
        }

        let map_text = match fs::read_to_string(&self.opts.map) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read feature map: {err}"));
                self.write_report(None);
                return;
            }
        };
        let map_hash = format!("blake3:{}", blake3::hash(map_text.as_bytes()).to_hex());
        self.report.compiled_from_feature_map_hash = Some(map_hash.clone());

        let map = match self.parse_feature_map(&map_text) {
            Some(map) => map,
            None => {
                self.write_report(None);
                return;
            }
        };
        let ranges = self.compile_ranges(&map);
        self.report.range_count = ranges.len();
        self.report.total_len = ranges.iter().map(|range| range.len).sum();

        if !self.report.errors.is_empty() {
            self.write_report(None);
            return;
        }

        let preimage = serde_json::json!({
            "ranges": ranges,
            "total_len": self.report.total_len,
            "compiled_from_feature_map_hash": map_hash,
            "capture_spec_hash": self.opts.capture_spec_hash,
            "compiler_or_exporter_commit": self.opts.compiler_or_exporter_commit,
        });
        let preimage_bytes = match serde_json::to_vec(&preimage) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.error(format!("cannot serialize layout hash preimage: {err}"));
                self.write_report(None);
                return;
            }
        };
        let layout_hash = format!("blake3:{}", blake3::hash(&preimage_bytes).to_hex());
        self.report.layout_hash = Some(layout_hash.clone());

        let layout = serde_json::json!({
            "ranges": preimage["ranges"],
            "total_len": preimage["total_len"],
            "blake3": layout_hash,
            "compiled_from_feature_map_hash": preimage["compiled_from_feature_map_hash"],
            "capture_spec_hash": preimage["capture_spec_hash"],
            "compiler_or_exporter_commit": preimage["compiler_or_exporter_commit"],
        });
        let text = match serde_json::to_string_pretty(&layout) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot serialize layout.json: {err}"));
                self.write_report(None);
                return;
            }
        };
        if let Some(parent) = self.opts.out.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.error(format!("cannot create layout output directory: {err}"));
                self.write_report(None);
                return;
            }
        }
        match fs::write(&self.opts.out, text.as_bytes()) {
            Ok(()) => {
                self.report.output_hash =
                    Some(format!("blake3:{}", blake3::hash(text.as_bytes()).to_hex()));
                self.write_report(Some(text.as_bytes()));
            }
            Err(err) => {
                self.error(format!("cannot write layout.json: {err}"));
                self.write_report(None);
            }
        }
    }

    fn parse_feature_map(&mut self, map_text: &str) -> Option<FeatureMap> {
        let (map, validation_errors) = match parse_feature_map(map_text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.error(format!("feature-map parse failed: {err}"));
                return None;
            }
        };
        for err in validation_errors {
            self.error(format!("feature-map validation: {err}"));
        }
        if map.features.is_empty() {
            self.error("feature map must contain at least one feature");
        }
        Some(map)
    }

    fn compile_ranges(&mut self, map: &FeatureMap) -> Vec<LayoutRange> {
        let region_sizes = map
            .regions
            .iter()
            .map(|region| (region.name.as_str(), region.size))
            .collect::<BTreeMap<_, _>>();
        let mut ranges = Vec::with_capacity(map.features.len());
        let mut total_len = 0u64;
        for (idx, feature) in map.features.iter().enumerate() {
            let Some(width) = feature_width(feature) else {
                self.error(format!("feature-map features[{idx}] width is unresolved"));
                continue;
            };
            if width == 0 {
                self.error(format!("feature-map features[{idx}] width must be > 0"));
                continue;
            }
            if feature.offset.0 < 0 {
                self.error(format!(
                    "feature-map features[{idx}] offset must be non-negative"
                ));
                continue;
            }
            let Some(region_size) = region_sizes.get(feature.region.as_str()).copied() else {
                self.error(format!(
                    "feature-map features[{idx}] region is not declared"
                ));
                continue;
            };
            let offset = feature.offset.0 as u64;
            let len = width as u64;
            match offset.checked_add(len) {
                Some(end) if end <= region_size => {}
                Some(_) | None => {
                    self.error(format!(
                        "feature-map features[{idx}] offset plus width exceeds region size"
                    ));
                    continue;
                }
            }
            match total_len.checked_add(len) {
                Some(next) => total_len = next,
                None => {
                    self.error("compiled layout total length overflowed u64");
                    continue;
                }
            }
            ranges.push(LayoutRange {
                region: feature.region.clone(),
                layout_version: self.opts.layout_version,
                offset,
                len,
            });
        }
        if ranges.is_empty() {
            self.error("compiled layout must contain at least one range");
        }
        ranges
    }

    fn finish(mut self) -> LayoutReport {
        self.report.status = if self.report.passed() {
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

    fn write_report(&mut self, _layout_bytes: Option<&[u8]>) {
        self.report.status = if self.report.passed() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };
    }
}

fn feature_width(feature: &Feature) -> Option<u32> {
    match feature.feature_type {
        FeatureType::Bytes => feature.width,
        _ => feature.feature_type.derived_width(),
    }
}

fn looks_hash_or_ref(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix("blake3:") {
        hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    } else {
        !value.trim().is_empty()
    }
}
