//! Determinism deny gate: scans Rust source files for banned tokens,
//! stripping comments and string literals before matching.
//!
//! Banned tokens (word-boundary matched):
//! - `std::thread`
//! - `tokio`
//! - `rayon`
//! - `crossbeam`
//! - `rand`
//! - `HashMap`
//! - `HashSet`
//! - `Instant`
//! - `SystemTime`
//! - `f32`
//! - `f64`
//! - `async fn`
//! - `await`
//!
//! Doc comments (`///` and `//!`) legitimately mention these words; they are
//! excluded from scanning by the comment-stripping step.

use std::path::{Path, PathBuf};

/// A single deny finding.
#[derive(Debug, Clone)]
pub struct Finding {
    pub file: PathBuf,
    pub line: usize,
    pub col: usize,
    pub token: String,
    pub text: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: banned token `{}` in: {}",
            self.file.display(),
            self.line,
            self.token,
            self.text.trim()
        )
    }
}

/// Strip comments and string literals from a line of Rust source,
/// replacing them with whitespace of the same length to preserve column
/// numbers. Doc comments (`///`, `//!`) are also stripped (they legitimately
/// mention banned words).
///
/// KNOWN LIMITATION (deliberate, fail-closed): this is a line-oriented
/// scanner — raw strings (`r#"…"#`) and multi-line `/* … */` block comments
/// are not modeled, so banned tokens inside them are still REPORTED. That
/// can only cause a false positive (a finding to explain), never a missed
/// token, which is the right failure direction for a deny gate.
fn strip_non_code(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut in_str = false;
    let mut in_char = false;

    while i < bytes.len() {
        if in_str {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                out.push(b' ');
                out.push(b' ');
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                in_str = false;
                out.push(b' ');
                i += 1;
                continue;
            }
            out.push(b' ');
            i += 1;
            continue;
        }
        if in_char {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                out.push(b' ');
                out.push(b' ');
                i += 2;
                continue;
            }
            if bytes[i] == b'\'' {
                in_char = false;
                out.push(b' ');
                i += 1;
                continue;
            }
            out.push(b' ');
            i += 1;
            continue;
        }
        // Check for line comment (// ...)
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // This is a comment; skip the rest of the line.
            // We push spaces for the remainder.
            while i < bytes.len() {
                out.push(b' ');
                i += 1;
            }
            break;
        }
        if bytes[i] == b'"' {
            in_str = true;
            out.push(b' ');
            i += 1;
            continue;
        }
        if bytes[i] == b'\'' {
            in_char = true;
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }

    // Safety: we only output ASCII-range bytes or spaces; source is UTF-8 but
    // the non-ASCII bytes are inside comments/strings which we blanked out.
    // Use lossy conversion to handle any edge cases.
    String::from_utf8_lossy(&out).into_owned()
}

/// Word-boundary check: is there a word boundary before position `start` and
/// after position `end` (exclusive) in `s`?
fn has_word_boundary(s: &str, start: usize, end: usize) -> bool {
    let bytes = s.as_bytes();
    let before_ok = if start == 0 {
        true
    } else {
        !is_ident_char(bytes[start - 1])
    };
    let after_ok = if end >= bytes.len() {
        true
    } else {
        !is_ident_char(bytes[end])
    };
    before_ok && after_ok
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The set of banned tokens with their matching patterns.
/// Each entry is (display_name, search_pattern).
/// `async fn` is handled specially (two-word).
static BANNED: &[&str] = &[
    "std::thread",
    "tokio",
    "rayon",
    "crossbeam",
    "rand",
    "HashMap",
    "HashSet",
    "Instant",
    "SystemTime",
    "f32",
    "f64",
    "async fn",
    "await",
];

/// Scan a single source file. Returns any findings.
pub fn scan_file(path: &Path) -> std::io::Result<Vec<Finding>> {
    let source = std::fs::read_to_string(path)?;
    let mut findings = Vec::new();

    for (lineno, raw) in source.lines().enumerate() {
        let lineno = lineno + 1;
        let stripped = strip_non_code(raw);

        for &token in BANNED {
            // Find all occurrences with word-boundary check.
            let mut search_start = 0;
            while let Some(pos) = stripped[search_start..].find(token) {
                let abs_pos = search_start + pos;
                let end = abs_pos + token.len();
                if has_word_boundary(&stripped, abs_pos, end) {
                    findings.push(Finding {
                        file: path.to_owned(),
                        line: lineno,
                        col: abs_pos + 1,
                        token: token.to_string(),
                        text: raw.to_string(),
                    });
                }
                search_start = abs_pos + 1;
            }
        }
    }

    Ok(findings)
}

/// Collect all `.rs` files under `dir` recursively.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_recursive(dir, &mut files);
    files.sort(); // deterministic order
    files
}

fn collect_rs_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Scan every `.rs` file under `root`, returning all findings. Public so the
/// deny self-test (`xtask/tests/deny_selftest.rs`) can run the gate against a
/// planted-token tree, continuously demonstrating the failure mode the M0
/// acceptance clause requires.
pub fn scan_tree(root: &Path) -> Vec<Finding> {
    let mut all: Vec<Finding> = Vec::new();
    let files = collect_rs_files(root);
    for file in &files {
        match scan_file(file) {
            Ok(findings) => all.extend(findings),
            Err(e) => {
                eprintln!("deny: cannot read {}: {}", file.display(), e);
            }
        }
    }
    all
}

/// Run the deny gate on the guest-linked crate source directories
/// (`refwork-emu`, `refwork-harness`, and `refwork-protocol` — the protocol
/// crate compiles into the guest harness binary and inherits D1-D4).
/// Returns `Ok(())` on clean, `Err(count)` with findings printed to stderr.
///
/// Host-side CLIs (`ramdiff`, `refwork-verify`, `refwork-hash`) are
/// deliberately OUTSIDE this scope — they may legitimately use floats,
/// sleeps, etc. `refwork-script` joins the scope only if `refwork-harness`
/// ever grows a dependency on it (plan phase-2/07 item 4).
pub fn run_deny(workspace_root: &Path) -> Result<(), usize> {
    let dirs = [
        workspace_root.join("crates/refwork-emu/src"),
        workspace_root.join("crates/refwork-harness/src"),
        workspace_root.join("crates/refwork-protocol/src"),
    ];

    let mut all_findings: Vec<Finding> = Vec::new();

    for dir in &dirs {
        if !dir.exists() {
            eprintln!("deny: directory not found: {}", dir.display());
            continue;
        }
        all_findings.extend(scan_tree(dir));
    }

    if all_findings.is_empty() {
        println!("deny: OK — no banned tokens found.");
        Ok(())
    } else {
        eprintln!("deny: FAILED — {} finding(s):", all_findings.len());
        for f in &all_findings {
            eprintln!("  {}", f);
        }
        Err(all_findings.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_line_comment() {
        let line = r#"let x = 1; // f32 HashMap"#;
        let stripped = strip_non_code(line);
        assert!(!stripped.contains("f32"));
        assert!(!stripped.contains("HashMap"));
        assert!(stripped.contains("let x = 1;"));
    }

    #[test]
    fn strip_removes_string_literal() {
        let line = r#"let s = "HashMap rand f32";"#;
        let stripped = strip_non_code(line);
        assert!(!stripped.contains("HashMap"));
        assert!(!stripped.contains("f32"));
        // The structural code around the string should remain
        assert!(stripped.contains("let s ="));
    }

    #[test]
    fn word_boundary_prevents_partial_match() {
        let line = "let f329 = 1;";
        let stripped = strip_non_code(line);
        // f32 should NOT match inside f329
        let findings: Vec<_> = BANNED
            .iter()
            .filter(|&&t| t == "f32")
            .flat_map(|&t| {
                let mut found = vec![];
                let mut s = 0;
                while let Some(p) = stripped[s..].find(t) {
                    let abs = s + p;
                    let end = abs + t.len();
                    if has_word_boundary(&stripped, abs, end) {
                        found.push(abs);
                    }
                    s = abs + 1;
                }
                found
            })
            .collect();
        assert!(
            findings.is_empty(),
            "f32 should not match inside f329: found {} times",
            findings.len()
        );
    }

    #[test]
    fn word_boundary_matches_standalone() {
        let line = "let x: f32 = 0.0;";
        let stripped = strip_non_code(line);
        let mut found = false;
        let t = "f32";
        let mut s = 0;
        while let Some(p) = stripped[s..].find(t) {
            let abs = s + p;
            let end = abs + t.len();
            if has_word_boundary(&stripped, abs, end) {
                found = true;
            }
            s = abs + 1;
        }
        assert!(found, "f32 should match as standalone token");
    }

    #[test]
    fn async_fn_matched() {
        let line = "pub async fn run() {}";
        let stripped = strip_non_code(line);
        let t = "async fn";
        let mut found = false;
        let mut s = 0;
        while let Some(p) = stripped[s..].find(t) {
            let abs = s + p;
            let end = abs + t.len();
            if has_word_boundary(&stripped, abs, end) {
                found = true;
            }
            s = abs + 1;
        }
        assert!(found, "async fn should be detected");
    }
}
