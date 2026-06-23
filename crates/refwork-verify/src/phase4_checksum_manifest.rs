//! Phase 4 private-bundle checksum manifest writer.
//!
//! The manifest records relative paths, byte counts, and BLAKE3 hashes only.
//! It deliberately omits the absolute private bundle path and file contents.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ChecksumManifestOptions {
    pub bundle: PathBuf,
    pub out: PathBuf,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ChecksumManifestReport {
    pub schema_version: u32,
    pub command: String,
    pub status: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct ChecksumFileEntry {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

pub fn write_phase4_checksum_manifest(opts: &ChecksumManifestOptions) -> ChecksumManifestReport {
    let mut writer = Writer::new(opts);
    writer.run();
    writer.finish()
}

struct Writer<'a> {
    opts: &'a ChecksumManifestOptions,
    report: ChecksumManifestReport,
}

impl<'a> Writer<'a> {
    fn new(opts: &'a ChecksumManifestOptions) -> Self {
        Self {
            opts,
            report: ChecksumManifestReport {
                schema_version: 1,
                command:
                    "refwork-verify phase4-checksum-manifest --bundle <redacted> --out <redacted>"
                        .to_owned(),
                status: "fail".to_owned(),
                ..ChecksumManifestReport::default()
            },
        }
    }

    fn run(&mut self) {
        if !self.opts.bundle.is_dir() {
            self.error("bundle root is not a directory");
            self.write_report();
            return;
        }

        self.add_required("manifest.json");
        self.add_workload_image();
        for rel in [
            "feature-map.yaml",
            "scoring-program.yaml",
            "layout.json",
            "captures/index.jsonl",
            "dedup-groups.jsonl",
            "score-plan.json",
        ] {
            self.add_required(rel);
        }
        self.add_dir_files("trajectory");
        self.add_dir_files("validation");
        self.report
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        self.report.file_count = self.report.files.len();
        self.report.total_bytes = self.report.files.iter().map(|entry| entry.bytes).sum();
        self.write_report();
    }

    fn finish(mut self) -> ChecksumManifestReport {
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

    fn add_workload_image(&mut self) {
        let yaml = self.opts.bundle.join("workload-image.yaml");
        let reference = self.opts.bundle.join("workload-image-ref.txt");
        match (yaml.is_file(), reference.is_file()) {
            (true, true) => {
                self.add_file("workload-image.yaml", &yaml);
                self.add_file("workload-image-ref.txt", &reference);
            }
            (true, false) => self.add_file("workload-image.yaml", &yaml),
            (false, true) => self.add_file("workload-image-ref.txt", &reference),
            (false, false) => {
                self.error("missing workload-image.yaml or workload-image-ref.txt");
            }
        }
    }

    fn add_required(&mut self, rel: &str) {
        let path = self.opts.bundle.join(rel);
        if !path.is_file() {
            self.error(format!("missing required file {rel}"));
            return;
        }
        self.add_file(rel, &path);
    }

    fn add_dir_files(&mut self, rel_dir: &str) {
        let dir = self.opts.bundle.join(rel_dir);
        if !dir.is_dir() {
            self.error(format!("missing required directory {rel_dir}"));
            return;
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                self.error(format!("cannot read directory {rel_dir}: {err}"));
                return;
            }
        };
        let mut paths = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !same_path(&path, &self.opts.out) {
                paths.push(path);
            }
        }
        paths.sort();
        if paths.is_empty() {
            self.error(format!("{rel_dir} must contain at least one file"));
        }
        for path in paths {
            let rel = match path.strip_prefix(&self.opts.bundle) {
                Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
                Err(_) => {
                    self.error("internal path escaped bundle root");
                    continue;
                }
            };
            self.add_file(&rel, &path);
        }
    }

    fn add_file(&mut self, rel: &str, path: &Path) {
        match fs::read(path) {
            Ok(bytes) => self.report.files.push(ChecksumFileEntry {
                path: rel.to_owned(),
                bytes: bytes.len() as u64,
                blake3: format!("blake3:{}", blake3::hash(&bytes).to_hex()),
            }),
            Err(err) => self.error(format!("cannot read {rel}: {err}")),
        }
    }

    fn write_report(&mut self) {
        let mut report = self.report.clone();
        report.file_count = report.files.len();
        report.total_bytes = report.files.iter().map(|entry| entry.bytes).sum();
        report.status = if report.passed() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };

        if let Some(parent) = self.opts.out.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.error(format!("cannot create output directory: {err}"));
                return;
            }
        }
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(err) = fs::write(&self.opts.out, json) {
                    self.error(format!("cannot write checksum manifest: {err}"));
                }
            }
            Err(err) => self.error(format!("checksum manifest serialization failed: {err}")),
        }
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
