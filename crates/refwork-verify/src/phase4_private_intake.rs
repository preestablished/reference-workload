//! Phase 4 private intake helper.
//!
//! This command prepares the lab-private root for the scorer-golden bundle. It
//! records exact ROM details only inside that private root.

use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PrivateIntakeOptions {
    pub rom_dir: PathBuf,
    pub private_root: PathBuf,
    pub operator_approved: bool,
    pub operator_metadata_policy: String,
    pub operator_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PrivateIntakeReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
    pub rom_regular_file_count: usize,
    pub rom_symlink_count: usize,
    pub created_dirs: Vec<String>,
    pub wrote_private_runbook: bool,
    pub wrote_rom_metadata: bool,
    pub errors: Vec<String>,
}

impl PrivateIntakeReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn prepare_phase4_private_intake(opts: &PrivateIntakeOptions) -> PrivateIntakeReport {
    let mut intake = Intake::new(opts);
    intake.run();
    intake.finish()
}

struct Intake<'a> {
    opts: &'a PrivateIntakeOptions,
    report: PrivateIntakeReport,
}

impl<'a> Intake<'a> {
    fn new(opts: &'a PrivateIntakeOptions) -> Self {
        Self {
            opts,
            report: PrivateIntakeReport {
                schema_version: 1,
                command: "refwork-verify phase4-private-intake --rom-dir <redacted> --private-root <redacted> --operator-approved".to_owned(),
                status: "fail".to_owned(),
                ..PrivateIntakeReport::default()
            },
        }
    }

    fn run(&mut self) {
        if !self.opts.operator_approved {
            self.error("operator approval flag is required");
            return;
        }
        if !self.opts.rom_dir.is_dir() {
            self.error("ROM directory is not present");
            return;
        }

        let (regular_files, symlinks) = match list_rom_dir(&self.opts.rom_dir) {
            Ok(found) => found,
            Err(err) => {
                self.error(err);
                return;
            }
        };
        self.report.rom_regular_file_count = regular_files.len();
        self.report.rom_symlink_count = symlinks;
        if symlinks > 0 {
            self.error("ROM directory contains symlinks requiring policy review");
        }
        if regular_files.len() != 1 {
            self.error(format!(
                "single-ROM guard expected exactly 1 regular file, found {}",
                regular_files.len()
            ));
        }
        if !self.report.errors.is_empty() {
            return;
        }

        if private_root_is_inside_source_checkout(&self.opts.private_root) {
            self.error("private root must be outside source checkouts");
            return;
        }

        for rel in [
            "",
            "workload-image",
            "capture-source",
            "phase3-scorer-corpus",
            "validation",
            "sanitized",
        ] {
            let path = if rel.is_empty() {
                self.opts.private_root.clone()
            } else {
                self.opts.private_root.join(rel)
            };
            if let Err(err) = fs::create_dir_all(&path) {
                self.error(format!(
                    "cannot create private root directory {rel:?}: {err}"
                ));
                return;
            }
            if !rel.is_empty() {
                self.report.created_dirs.push(rel.to_owned());
            }
        }

        let rom_path = &regular_files[0];
        let rom_bytes = match fs::read(rom_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.error(format!("cannot read selected ROM: {err}"));
                return;
            }
        };
        let rom_blake3 = format!("blake3:{}", blake3::hash(&rom_bytes).to_hex());
        let source_head = run_git(&["rev-parse", "HEAD"]);
        let source_status = run_git(&["status", "--short"]);

        let metadata = json!({
            "schema_version": 1,
            "kind": "phase4-private-rom-metadata",
            "rom_path": rom_path.display().to_string(),
            "rom_file_name": rom_path.file_name().and_then(|name| name.to_str()).unwrap_or(""),
            "byte_length": rom_bytes.len(),
            "blake3": rom_blake3,
            "operator_approved": self.opts.operator_approved,
            "operator_metadata_policy": self.opts.operator_metadata_policy,
            "operator_label": self.opts.operator_label,
            "source": {
                "reference_workload_head": source_head.as_deref(),
                "reference_workload_status_short": source_status.as_deref(),
            }
        });
        match serde_json::to_string_pretty(&metadata) {
            Ok(text) => {
                if let Err(err) = fs::write(self.opts.private_root.join("rom-metadata.json"), text)
                {
                    self.error(format!("cannot write rom-metadata.json: {err}"));
                    return;
                }
                self.report.wrote_rom_metadata = true;
            }
            Err(err) => {
                self.error(format!("cannot serialize ROM metadata: {err}"));
                return;
            }
        }

        let runbook = format!(
            "# Phase 4 Scorer Private Runbook\n\n\
             This file is private. Do not copy exact paths, ROM metadata, capture ids, labels, decoded vectors, or retrieval commands into public notes.\n\n\
             ## Selected ROM\n\n\
             - path: `{}`\n\
             - byte_length: `{}`\n\
             - blake3: `{}`\n\
             - operator_metadata_policy: `{}`\n\
             - operator_label: `{}`\n\
             - operator_approved: `{}`\n\n\
             ## Source Baseline\n\n\
             - reference_workload_head: `{}`\n\n\
             ```text\n{}\n```\n\n\
             ## Next Private Commands\n\n\
             ```sh\n\
             cargo test --locked -p refwork-verify phase4 -- --nocapture\n\
             cargo run --locked -p refwork-featuremap -- validate <private-feature-map.yaml> --scoring <private-scoring-program.yaml>\n\
             cargo run --locked -p refwork-verify -- phase4-layout --map <private-feature-map.yaml> --out <private-layout.json> --capture-spec-hash <private-capture-spec-hash>\n\
             cargo run --locked -p refwork-verify -- map-check --rom <operator-private-rom> --map <private-feature-map.yaml> --script <private-script.padlog> --expect <private-map-check.expect.yaml>\n\
             cargo run --locked -p refwork-verify -- trace --captures <private-capture-index.jsonl> --map <private-feature-map.yaml> --scoring <private-scoring-program.yaml> --labels <private-operator-labels.yaml> --out <private-trajectory-output.jsonl> --report <private-trace-report.json>\n\
             cargo run --locked -p refwork-verify -- phase4-score-plan --captures <private-capture-index.jsonl> --out <private-score-plan.json> --first-boss <capture-id> --goal-positive <capture-id> --goal-negative <capture-id>\n\
             cargo run --locked -p refwork-verify -- phase4-checksum-manifest --bundle <private-phase3-scorer-corpus> --out <private-checksum-manifest.json>\n\
             cargo run --locked -p refwork-verify -- phase4-bundle-check --bundle <private-phase3-scorer-corpus> --report <private-phase4-bundle-check.json>\n\
             ```\n",
            rom_path.display(),
            rom_bytes.len(),
            rom_blake3,
            self.opts.operator_metadata_policy,
            self.opts.operator_label.as_deref().unwrap_or("withheld"),
            self.opts.operator_approved,
            source_head.as_deref().unwrap_or("unavailable"),
            source_status.as_deref().unwrap_or("unavailable"),
        );
        if let Err(err) = fs::write(self.opts.private_root.join("PRIVATE-RUNBOOK.md"), runbook) {
            self.error(format!("cannot write PRIVATE-RUNBOOK.md: {err}"));
            return;
        }
        self.report.wrote_private_runbook = true;
    }

    fn finish(mut self) -> PrivateIntakeReport {
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
}

fn list_rom_dir(path: &Path) -> Result<(Vec<PathBuf>, usize), String> {
    let entries = fs::read_dir(path).map_err(|err| format!("cannot read ROM directory: {err}"))?;
    let mut files = Vec::new();
    let mut symlinks = 0usize;
    for entry in entries {
        let entry = entry.map_err(|err| format!("cannot inspect ROM directory entry: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("cannot inspect ROM directory file type: {err}"))?;
        if file_type.is_symlink() {
            symlinks += 1;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok((files, symlinks))
}

fn private_root_is_inside_source_checkout(private_root: &Path) -> bool {
    let root = private_root
        .parent()
        .unwrap_or(private_root)
        .canonicalize()
        .unwrap_or_else(|_| private_root.to_path_buf());
    root.ancestors()
        .any(|ancestor| ancestor.join(".git").is_dir())
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned(),
    )
}
