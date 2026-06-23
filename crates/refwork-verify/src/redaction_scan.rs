//! Public-note redaction scanner for Phase 4 fulfillment handoff text.
//!
//! The scanner reports finding kinds and locations, but never echoes matched
//! private literals or source excerpts into the report.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RedactionScanOptions {
    pub input: PathBuf,
    pub report: Option<PathBuf>,
    pub forbidden_literals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RedactionScanReport {
    pub schema_version: u32,
    pub command: String,
    pub input: String,
    pub status: String,
    pub bytes: usize,
    pub lines: usize,
    pub forbidden_literal_count: usize,
    pub finding_count: usize,
    pub findings: Vec<RedactionFinding>,
    pub errors: Vec<String>,
}

impl RedactionScanReport {
    pub fn passed(&self) -> bool {
        self.errors.is_empty() && self.findings.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RedactionFinding {
    pub kind: String,
    pub line: usize,
    pub column: usize,
}

pub fn scan_redactions(opts: &RedactionScanOptions) -> RedactionScanReport {
    let mut scanner = Scanner::new(opts);
    scanner.run();
    scanner.finish()
}

struct Scanner<'a> {
    opts: &'a RedactionScanOptions,
    report: RedactionScanReport,
}

impl<'a> Scanner<'a> {
    fn new(opts: &'a RedactionScanOptions) -> Self {
        Self {
            opts,
            report: RedactionScanReport {
                schema_version: 1,
                command: "refwork-verify redaction-scan --input <redacted>".to_owned(),
                input: "<redacted>".to_owned(),
                status: "fail".to_owned(),
                forbidden_literal_count: opts.forbidden_literals.len(),
                ..RedactionScanReport::default()
            },
        }
    }

    fn run(&mut self) {
        let text = match fs::read_to_string(&self.opts.input) {
            Ok(text) => text,
            Err(err) => {
                self.error(format!("cannot read input: {err}"));
                self.write_report();
                return;
            }
        };
        self.report.bytes = text.len();
        self.report.lines = text.lines().count();

        for (line_idx, line) in text.lines().enumerate() {
            let line_no = line_idx + 1;
            self.scan_forbidden_literals(line, line_no);
            self.scan_forbidden_fields(line, line_no);
            self.scan_private_capture_ids(line, line_no);
            self.scan_payload_tokens(line, line_no);
            self.scan_private_file_names(line, line_no);
            self.scan_secret_retrieval_terms(line, line_no);
        }

        self.write_report();
    }

    fn finish(mut self) -> RedactionScanReport {
        self.report.finding_count = self.report.findings.len();
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

    fn finding(&mut self, kind: &str, line: usize, column: usize) {
        if self.report.findings.len() < 500 {
            self.report.findings.push(RedactionFinding {
                kind: kind.to_owned(),
                line,
                column,
            });
        } else if self.report.findings.len() == 500 {
            self.report.findings.push(RedactionFinding {
                kind: "additional_findings_suppressed".to_owned(),
                line,
                column,
            });
        }
    }

    fn scan_forbidden_literals(&mut self, line: &str, line_no: usize) {
        let literals = self.opts.forbidden_literals.clone();
        for literal in literals {
            if literal.is_empty() {
                continue;
            }
            for col in find_all(line, &literal) {
                self.finding("operator_forbidden_literal", line_no, col);
            }
        }
    }

    fn scan_forbidden_fields(&mut self, line: &str, line_no: usize) {
        for needle in [
            "decoded_values",
            "raw_wram",
            "wram_bytes",
            "rom_bytes",
            "save_ram",
            "framebuffer.bytes",
            "framebuffer_bytes",
            "raw_capture_bytes",
        ] {
            for col in find_all(line, needle) {
                self.finding("private_payload_field", line_no, col);
            }
        }
    }

    fn scan_private_capture_ids(&mut self, line: &str, line_no: usize) {
        for (col, token) in tokens_with_columns(line) {
            let token = trim_token(token);
            if looks_private_capture_id(token) {
                self.finding("private_capture_id", line_no, col);
            }
        }
    }

    fn scan_payload_tokens(&mut self, line: &str, line_no: usize) {
        for (col, token) in tokens_with_columns(line) {
            let token = trim_token(token);
            if looks_long_payload(token) {
                self.finding("long_base64_like_payload", line_no, col);
            }
        }
    }

    fn scan_private_file_names(&mut self, line: &str, line_no: usize) {
        for (col, token) in tokens_with_columns(line) {
            let token = trim_token(token);
            if looks_private_file_name(token) {
                self.finding("private_file_name", line_no, col);
            }
        }
    }

    fn scan_secret_retrieval_terms(&mut self, line: &str, line_no: usize) {
        let lower = line.to_ascii_lowercase();
        for needle in [
            "authorization:",
            "bearer ",
            "token_owner",
            "token owner",
            "secret_access_key",
            "aws_access_key",
            "gh auth token",
            "private retrieval command",
            "curl -h",
        ] {
            for col in find_all(&lower, needle) {
                self.finding("private_retrieval_or_secret_detail", line_no, col);
            }
        }
    }

    fn write_report(&mut self) {
        let Some(path) = &self.opts.report else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.error(format!("cannot create report directory: {err}"));
                return;
            }
        }
        let mut report = self.report.clone();
        report.finding_count = report.findings.len();
        report.status = if report.passed() {
            "pass".to_owned()
        } else {
            "fail".to_owned()
        };
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(err) = fs::write(path, json) {
                    self.error(format!("cannot write report: {err}"));
                }
            }
            Err(err) => self.error(format!("redaction report serialization failed: {err}")),
        }
    }
}

pub fn load_forbidden_literals(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("cannot read forbid file: {err}"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut offset = 0;
    while let Some(idx) = haystack[offset..].find(needle) {
        let col = offset + idx + 1;
        out.push(col);
        offset += idx + needle.len();
    }
    out
}

fn tokens_with_columns(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(start_idx) = start.take() {
                out.push((start_idx + 1, &line[start_idx..idx]));
            }
        } else if start.is_none() {
            start = Some(idx);
        }
    }
    if let Some(start_idx) = start {
        out.push((start_idx + 1, &line[start_idx..]));
    }
    out
}

fn trim_token(token: &str) -> &str {
    token.trim_matches(|ch: char| {
        matches!(
            ch,
            '"' | '\'' | '`' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
        )
    })
}

fn looks_private_capture_id(token: &str) -> bool {
    let Some(rest) = token
        .strip_prefix("cap-")
        .or_else(|| token.strip_prefix("capture-"))
        .or_else(|| token.strip_prefix("ctx-"))
    else {
        return false;
    };
    rest.len() >= 6 && rest.chars().all(|ch| ch.is_ascii_digit())
}

fn looks_long_payload(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() < 80 {
        return false;
    }
    let mut has_payload_marker = false;
    for &byte in bytes {
        let ch = byte as char;
        if ch == '+' || ch == '/' || ch == '=' {
            has_payload_marker = true;
        } else if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
            return false;
        }
    }
    has_payload_marker || bytes.len() >= 120
}

fn looks_private_file_name(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    if [".sfc", ".smc", ".fig", ".swc"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
    {
        return true;
    }
    let has_frame_extension = [".png", ".jpg", ".jpeg", ".raw", ".bin", ".lz4"]
        .iter()
        .any(|suffix| lower.ends_with(suffix));
    has_frame_extension && (lower.contains("screenshot") || lower.contains("framebuffer"))
}
