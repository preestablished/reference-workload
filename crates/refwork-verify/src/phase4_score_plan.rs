//! Deterministic Phase 4 score-plan writer.
//!
//! The command consumes a private capture index and emits K=32 batch membership
//! plus operator-supplied label capture ids. It does not compute scorer outputs.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

const K: usize = 32;

#[derive(Debug, Clone)]
pub struct ScorePlanOptions {
    pub captures: PathBuf,
    pub out: PathBuf,
    pub client_batch_prefix: String,
    pub first_boss: Vec<String>,
    pub goal_positive: Vec<String>,
    pub goal_negative: Vec<String>,
    pub checkpoint_after_batch: Option<String>,
    pub restore_control_batch_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScorePlanReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub capture_count: usize,
    pub full_batch_count: usize,
    pub emitted_capture_count: usize,
    pub trailing_capture_count: usize,
    pub batch_ids: Vec<String>,
    pub first_boss_label_count: usize,
    pub goal_positive_label_count: usize,
    pub goal_negative_label_count: usize,
    pub output_hash: Option<String>,
    pub errors: Vec<String>,
}

impl ScorePlanReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn write_phase4_score_plan(opts: &ScorePlanOptions) -> ScorePlanReport {
    let mut writer = Writer::new(opts);
    writer.run();
    writer.finish()
}

struct Writer<'a> {
    opts: &'a ScorePlanOptions,
    report: ScorePlanReport,
}

impl<'a> Writer<'a> {
    fn new(opts: &'a ScorePlanOptions) -> Self {
        Self {
            opts,
            report: ScorePlanReport {
                schema_version: 1,
                command: "refwork-verify phase4-score-plan --captures <redacted> --out <redacted>"
                    .to_owned(),
                status: "fail".to_owned(),
                ..ScorePlanReport::default()
            },
        }
    }

    fn run(&mut self) {
        if self.opts.client_batch_prefix.is_empty() {
            self.error("client batch prefix must not be empty");
        }
        let capture_ids = self.read_capture_ids();
        self.report.capture_count = capture_ids.len();
        self.report.full_batch_count = capture_ids.len() / K;
        self.report.emitted_capture_count = self.report.full_batch_count * K;
        self.report.trailing_capture_count = capture_ids.len() % K;
        if self.report.full_batch_count == 0 {
            self.error("capture index must contain at least 32 captures for a K=32 score plan");
        }

        let capture_set = capture_ids.iter().cloned().collect::<HashSet<_>>();
        self.check_labels("first_boss", &self.opts.first_boss, &capture_set);
        self.check_labels("goal_positive", &self.opts.goal_positive, &capture_set);
        self.check_labels("goal_negative", &self.opts.goal_negative, &capture_set);
        if self.opts.first_boss.is_empty() {
            self.error("at least one --first-boss capture id is required");
        }
        if self.opts.goal_positive.is_empty() {
            self.error("at least one --goal-positive capture id is required");
        }
        if self.opts.goal_negative.is_empty() {
            self.error("at least one --goal-negative capture id is required");
        }

        if !self.report.errors.is_empty() {
            self.write_report(None);
            return;
        }

        let batches = capture_ids
            .chunks_exact(K)
            .enumerate()
            .map(|(idx, ids)| {
                let batch_id = format!("{}-{:04}", self.opts.client_batch_prefix, idx + 1);
                self.report.batch_ids.push(batch_id.clone());
                serde_json::json!({
                    "client_batch_id": batch_id,
                    "capture_ids": ids,
                })
            })
            .collect::<Vec<_>>();
        let default_checkpoint = self.report.batch_ids[0].clone();
        let checkpoint_after_batch = self
            .opts
            .checkpoint_after_batch
            .clone()
            .unwrap_or(default_checkpoint);
        self.check_batch_ref(&checkpoint_after_batch, "checkpoint_after_batch");

        let restore_control_batch_ids = if self.opts.restore_control_batch_ids.is_empty() {
            vec![checkpoint_after_batch.clone()]
        } else {
            self.opts.restore_control_batch_ids.clone()
        };
        for batch_id in &restore_control_batch_ids {
            self.check_batch_ref(batch_id, "restore_control_batch_ids");
        }
        if !self.report.errors.is_empty() {
            self.write_report(None);
            return;
        }

        let plan = serde_json::json!({
            "schema_version": 1,
            "k": K,
            "batches": batches,
            "checkpoint_after_batch": checkpoint_after_batch,
            "restore_control_batch_ids": restore_control_batch_ids,
            "labels": {
                "first_boss": self.opts.first_boss,
                "goal_positive": self.opts.goal_positive,
                "goal_negative": self.opts.goal_negative,
            }
        });
        let text = match serde_json::to_string_pretty(&plan) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot serialize score plan: {err}"));
                self.write_report(None);
                return;
            }
        };
        if let Some(parent) = self.opts.out.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.error(format!("cannot create score-plan output directory: {err}"));
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
                self.error(format!("cannot write score plan: {err}"));
                self.write_report(None);
            }
        }
    }

    fn finish(mut self) -> ScorePlanReport {
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

    fn read_capture_ids(&mut self) -> Vec<String> {
        let file = match fs::File::open(&self.opts.captures) {
            Ok(file) => file,
            Err(err) => {
                self.error(format!("cannot open captures index: {err}"));
                return Vec::new();
            }
        };
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        for (line_idx, line) in BufReader::new(file).lines().enumerate() {
            let line_no = line_idx + 1;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    self.error(format!("captures index line {line_no}: read failed: {err}"));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(err) => {
                    self.error(format!(
                        "captures index line {line_no}: invalid JSON: {err}"
                    ));
                    continue;
                }
            };
            let Some(capture_id) = value.get("capture_id").and_then(Value::as_str) else {
                self.error(format!("captures index line {line_no}: missing capture_id"));
                continue;
            };
            if capture_id.is_empty() {
                self.error(format!(
                    "captures index line {line_no}: capture_id must not be empty"
                ));
                continue;
            }
            if !seen.insert(capture_id.to_owned()) {
                self.error(format!(
                    "captures index line {line_no}: duplicate capture_id"
                ));
                continue;
            }
            ids.push(capture_id.to_owned());
        }
        ids
    }

    fn check_labels(&mut self, name: &str, ids: &[String], capture_set: &HashSet<String>) {
        let mut seen = HashSet::new();
        for id in ids {
            if !seen.insert(id) {
                self.error(format!("label {name} contains duplicate capture id"));
            }
            if !capture_set.contains(id) {
                self.error(format!("label {name} references unknown capture id"));
            }
        }
        match name {
            "first_boss" => self.report.first_boss_label_count = ids.len(),
            "goal_positive" => self.report.goal_positive_label_count = ids.len(),
            "goal_negative" => self.report.goal_negative_label_count = ids.len(),
            _ => {}
        }
    }

    fn check_batch_ref(&mut self, batch_id: &str, label: &str) {
        if !self.report.batch_ids.iter().any(|id| id == batch_id) {
            self.error(format!("{label} references unknown client_batch_id"));
        }
    }

    fn write_report(&mut self, _plan_bytes: Option<&[u8]>) {
        self.report.status = if self.report.passed() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };
    }
}
