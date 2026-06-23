//! Phase 4 trajectory emitter for decoded capture indexes.
//!
//! The command consumes metadata/decoded-feature rows and operator labels. It
//! does not read ROM bytes, framebuffer bytes, or private feature-byte blobs.

use refwork_featuremap::{
    parse_feature_map, parse_scoring_program, validate_pair, BitOp, CompareOp, FeatureMap,
    PenaltyAction, Pred, PredLeaf, ScoringProgram,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TraceOptions {
    pub captures: PathBuf,
    pub map: PathBuf,
    pub scoring: PathBuf,
    pub labels: PathBuf,
    pub out: PathBuf,
    pub report: PathBuf,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TraceReport {
    pub schema_version: u32,
    pub command: String,
    pub captures: String,
    pub map: String,
    pub scoring: String,
    pub labels: String,
    pub out: String,
    pub capture_count: usize,
    pub feature_count: usize,
    pub input_hashes: BTreeMap<String, String>,
    pub output_hash: Option<String>,
    pub status: String,
    pub errors: Vec<String>,
}

impl TraceReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct LabelFile {
    schema_version: u32,
    kind: String,
    labels: Vec<TraceLabel>,
}

#[derive(Debug, Clone, Deserialize)]
struct TraceLabel {
    capture_id: String,
    #[serde(default)]
    expected_highest_stage: Option<String>,
    #[serde(default)]
    prune: Option<bool>,
    #[serde(default)]
    goal: Option<bool>,
    #[serde(default)]
    first_boss_coverage: Option<bool>,
    #[serde(default)]
    active_stages: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct TraceRow {
    schema_version: u32,
    frame_index: u64,
    capture_id: String,
    decoded_order: Vec<String>,
    decoded_values: Vec<i64>,
    active_stages: Vec<String>,
    expected_highest_stage: String,
    prune: bool,
    goal: bool,
    first_boss_coverage: bool,
}

pub fn emit_phase4_trace(opts: &TraceOptions) -> TraceReport {
    let mut emitter = TraceEmitter::new(opts);
    emitter.run();
    emitter.finish()
}

struct TraceEmitter<'a> {
    opts: &'a TraceOptions,
    report: TraceReport,
}

impl<'a> TraceEmitter<'a> {
    fn new(opts: &'a TraceOptions) -> Self {
        Self {
            opts,
            report: TraceReport {
                schema_version: 1,
                command: format!(
                    "refwork-verify trace --captures {} --map {} --scoring {} --labels {} --out {} --report {}",
                    opts.captures.display(),
                    opts.map.display(),
                    opts.scoring.display(),
                    opts.labels.display(),
                    opts.out.display(),
                    opts.report.display()
                ),
                captures: opts.captures.display().to_string(),
                map: opts.map.display().to_string(),
                scoring: opts.scoring.display().to_string(),
                labels: opts.labels.display().to_string(),
                out: opts.out.display().to_string(),
                status: "fail".to_owned(),
                ..TraceReport::default()
            },
        }
    }

    fn run(&mut self) {
        self.hash_input("captures", &self.opts.captures);
        self.hash_input("map", &self.opts.map);
        self.hash_input("scoring", &self.opts.scoring);
        self.hash_input("labels", &self.opts.labels);

        let Some((map, scoring)) = self.load_map_and_scoring() else {
            self.write_report();
            return;
        };
        self.report.feature_count = map.features.len();
        let Some(labels) = self.load_labels() else {
            self.write_report();
            return;
        };

        let rows = self.emit_rows(&map, &scoring, &labels);
        if self.report.errors.is_empty() {
            let mut out = String::new();
            for row in rows {
                match serde_json::to_string(&row) {
                    Ok(line) => {
                        out.push_str(&line);
                        out.push('\n');
                    }
                    Err(err) => self.error(format!("trace row serialization failed: {err}")),
                }
            }
            if self.report.errors.is_empty() {
                if let Some(parent) = self.opts.out.parent() {
                    if let Err(err) = fs::create_dir_all(parent) {
                        self.error(format!(
                            "cannot create output directory {}: {err}",
                            parent.display()
                        ));
                    }
                }
                match fs::write(&self.opts.out, out.as_bytes()) {
                    Ok(()) => {
                        self.report.output_hash =
                            Some(format!("blake3:{}", blake3::hash(out.as_bytes()).to_hex()));
                    }
                    Err(err) => {
                        self.error(format!("cannot write {}: {err}", self.opts.out.display()))
                    }
                }
            }
        }
        self.write_report();
    }

    fn finish(mut self) -> TraceReport {
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

    fn hash_input(&mut self, name: &str, path: &Path) {
        match fs::read(path) {
            Ok(bytes) => {
                self.report.input_hashes.insert(
                    name.to_owned(),
                    format!("blake3:{}", blake3::hash(&bytes).to_hex()),
                );
            }
            Err(err) => self.error(format!("cannot read {}: {err}", path.display())),
        }
    }

    fn load_map_and_scoring(&mut self) -> Option<(FeatureMap, ScoringProgram)> {
        let map_text = self.read_text(&self.opts.map)?;
        let scoring_text = self.read_text(&self.opts.scoring)?;
        let (map, map_errors) = match parse_feature_map(&map_text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.error(format!("feature map parse failed: {err}"));
                return None;
            }
        };
        for err in map_errors {
            self.error(format!("feature map validation: {err}"));
        }
        let (scoring, scoring_errors) = match parse_scoring_program(&scoring_text) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.error(format!("scoring program parse failed: {err}"));
                return None;
            }
        };
        for err in scoring_errors {
            self.error(format!("scoring program validation: {err}"));
        }
        for err in validate_pair(&map, &scoring) {
            self.error(format!("feature-map/scoring validation: {err}"));
        }
        if self.report.errors.is_empty() {
            Some((map, scoring))
        } else {
            None
        }
    }

    fn load_labels(&mut self) -> Option<HashMap<String, TraceLabel>> {
        let labels_text = self.read_text(&self.opts.labels)?;
        let labels: LabelFile = match serde_yaml::from_str(&labels_text) {
            Ok(labels) => labels,
            Err(err) => {
                self.error(format!("labels parse failed: {err}"));
                return None;
            }
        };
        if labels.schema_version != 1 {
            self.error(format!(
                "labels schema_version {} unsupported, expected 1",
                labels.schema_version
            ));
        }
        if labels.kind != "phase4-trace-labels" {
            self.error(format!(
                "labels kind {:?} unsupported, expected phase4-trace-labels",
                labels.kind
            ));
        }
        let mut by_capture = HashMap::new();
        for label in labels.labels {
            if by_capture.insert(label.capture_id.clone(), label).is_some() {
                self.error("labels contain duplicate capture_id");
            }
        }
        if by_capture.is_empty() {
            self.error("labels must contain at least one label");
        }
        if self.report.errors.is_empty() {
            Some(by_capture)
        } else {
            None
        }
    }

    fn emit_rows(
        &mut self,
        map: &FeatureMap,
        scoring: &ScoringProgram,
        labels: &HashMap<String, TraceLabel>,
    ) -> Vec<TraceRow> {
        let file = match fs::File::open(&self.opts.captures) {
            Ok(file) => file,
            Err(err) => {
                self.error(format!(
                    "cannot open captures {}: {err}",
                    self.opts.captures.display()
                ));
                return Vec::new();
            }
        };

        let feature_order = map
            .features
            .iter()
            .map(|feature| feature.name.as_str())
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for (line_idx, line) in BufReader::new(file).lines().enumerate() {
            let line_no = line_idx + 1;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    self.error(format!("captures:{}: read failed: {err}", line_no));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let json: Value = match serde_json::from_str(&line) {
                Ok(json) => json,
                Err(err) => {
                    self.error(format!("captures:{}: invalid JSON: {err}", line_no));
                    continue;
                }
            };
            let Some(capture_id) = string_at(&json, &["capture_id"]) else {
                self.error(format!("captures:{}: missing capture_id", line_no));
                continue;
            };
            let frame_index = u64_at(&json, &["frame_index"])
                .or_else(|| u64_at(&json, &["frame_counter"]))
                .unwrap_or_else(|| {
                    self.error(format!(
                        "captures:{}: missing frame_index/frame_counter",
                        line_no
                    ));
                    0
                });
            let decoded_order = string_array_at(&json, &["decoded_order"]).unwrap_or_else(|| {
                self.error(format!("captures:{}: missing decoded_order", line_no));
                Vec::new()
            });
            let decoded_values = i64_array_at(&json, &["decoded_values"]).unwrap_or_else(|| {
                self.error(format!("captures:{}: missing decoded_values", line_no));
                Vec::new()
            });
            if decoded_order.len() != decoded_values.len() {
                self.error(format!(
                    "captures:{}: decoded_order len {} != decoded_values len {}",
                    line_no,
                    decoded_order.len(),
                    decoded_values.len()
                ));
                continue;
            }
            if decoded_order.iter().map(String::as_str).collect::<Vec<_>>() != feature_order {
                self.error(format!(
                    "captures:{}: decoded_order does not match feature-map order",
                    line_no
                ));
                continue;
            }

            let values = decoded_order
                .iter()
                .cloned()
                .zip(decoded_values.iter().copied())
                .collect::<HashMap<_, _>>();
            let active_stages = scoring
                .stages
                .list
                .iter()
                .filter(|stage| eval_pred(&stage.when, &values))
                .map(|stage| stage.name.clone())
                .collect::<Vec<_>>();
            let expected_highest_stage = active_stages
                .last()
                .cloned()
                .unwrap_or_else(|| "root".to_owned());
            let prune = scoring.penalties.as_ref().is_some_and(|penalties| {
                penalties.iter().any(|penalty| {
                    penalty.action == PenaltyAction::Prune && eval_pred(&penalty.when, &values)
                })
            });
            let goal = eval_pred(&scoring.goal.predicate, &values);
            let first_boss_coverage = active_stages.iter().any(|stage| stage == "first_boss");

            let Some(label) = labels.get(capture_id) else {
                self.error(format!(
                    "captures:{}: no label for capture_id {}",
                    line_no, capture_id
                ));
                continue;
            };
            self.check_label(
                line_no,
                label,
                &active_stages,
                &expected_highest_stage,
                prune,
                goal,
                first_boss_coverage,
            );

            rows.push(TraceRow {
                schema_version: 1,
                frame_index,
                capture_id: capture_id.to_owned(),
                decoded_order,
                decoded_values,
                active_stages,
                expected_highest_stage,
                prune,
                goal,
                first_boss_coverage,
            });
            self.report.capture_count += 1;
        }

        rows
    }

    fn check_label(
        &mut self,
        line_no: usize,
        label: &TraceLabel,
        active_stages: &[String],
        expected_highest_stage: &str,
        prune: bool,
        goal: bool,
        first_boss_coverage: bool,
    ) {
        if let Some(label_active) = &label.active_stages {
            if label_active != active_stages {
                self.error(format!(
                    "captures:{}: label active_stages {:?} != computed {:?}",
                    line_no, label_active, active_stages
                ));
            }
        }
        if let Some(label_highest) = &label.expected_highest_stage {
            if label_highest != expected_highest_stage {
                self.error(format!(
                    "captures:{}: label expected_highest_stage {:?} != computed {:?}",
                    line_no, label_highest, expected_highest_stage
                ));
            }
        } else {
            self.error(format!(
                "captures:{}: label missing expected_highest_stage",
                line_no
            ));
        }
        if let Some(label_prune) = label.prune {
            if label_prune != prune {
                self.error(format!(
                    "captures:{}: label prune {} != computed {}",
                    line_no, label_prune, prune
                ));
            }
        } else {
            self.error(format!("captures:{}: label missing prune", line_no));
        }
        if let Some(label_goal) = label.goal {
            if label_goal != goal {
                self.error(format!(
                    "captures:{}: label goal {} != computed {}",
                    line_no, label_goal, goal
                ));
            }
        } else {
            self.error(format!("captures:{}: label missing goal", line_no));
        }
        if let Some(label_first_boss) = label.first_boss_coverage {
            if label_first_boss != first_boss_coverage {
                self.error(format!(
                    "captures:{}: label first_boss_coverage {} != computed {}",
                    line_no, label_first_boss, first_boss_coverage
                ));
            }
        } else {
            self.error(format!(
                "captures:{}: label missing first_boss_coverage",
                line_no
            ));
        }
    }

    fn read_text(&mut self, path: &Path) -> Option<String> {
        match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(err) => {
                self.error(format!("cannot read {}: {err}", path.display()));
                None
            }
        }
    }

    fn write_report(&mut self) {
        let mut report = self.report.clone();
        report.status = if report.errors.is_empty() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };
        if let Some(parent) = self.opts.report.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.error(format!(
                    "cannot create report directory {}: {err}",
                    parent.display()
                ));
                report.errors = self.report.errors.clone();
                report.status = "fail".to_owned();
            }
        }
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(err) = fs::write(&self.opts.report, json) {
                    self.error(format!(
                        "cannot write report {}: {err}",
                        self.opts.report.display()
                    ));
                }
            }
            Err(err) => self.error(format!("trace report serialization failed: {err}")),
        }
    }
}

fn eval_pred(pred: &Pred, values: &HashMap<String, i64>) -> bool {
    match pred {
        Pred::All { all } => all.iter().all(|pred| eval_pred(pred, values)),
        Pred::Any { any } => any.iter().any(|pred| eval_pred(pred, values)),
        Pred::Not { not } => !eval_pred(not, values),
        Pred::Leaf(leaf) => eval_leaf(leaf, values),
    }
}

fn eval_leaf(leaf: &PredLeaf, values: &HashMap<String, i64>) -> bool {
    match leaf {
        PredLeaf::Compare { feature, op, value } => {
            let Some(actual) = values.get(feature).copied() else {
                return false;
            };
            match op {
                CompareOp::Eq => actual == value.0,
                CompareOp::Ne => actual != value.0,
                CompareOp::Lt => actual < value.0,
                CompareOp::Le => actual <= value.0,
                CompareOp::Gt => actual > value.0,
                CompareOp::Ge => actual >= value.0,
            }
        }
        PredLeaf::BitTest { feature, op, bit } => {
            let Some(actual) = values.get(feature).copied() else {
                return false;
            };
            let mask = 1_i64.checked_shl((*bit).into()).unwrap_or(0);
            match op {
                BitOp::BitSet => actual & mask != 0,
                BitOp::BitClear => actual & mask == 0,
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
    value_at(value, path)?.as_str()
}

fn u64_at(value: &Value, path: &[&str]) -> Option<u64> {
    value_at(value, path)?.as_u64()
}

fn string_array_at(value: &Value, path: &[&str]) -> Option<Vec<String>> {
    value_at(value, path)?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn i64_array_at(value: &Value, path: &[&str]) -> Option<Vec<i64>> {
    value_at(value, path)?
        .as_array()?
        .iter()
        .map(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        })
        .collect()
}
