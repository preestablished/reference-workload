//! Release-binary symbol audit for determinism-sensitive entry points.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const BANNED_SYMBOLS: &[&str] = &[
    "clock_gettime",
    "clock_nanosleep",
    "gettimeofday",
    "nanosleep",
    "sleep",
    "usleep",
    "pthread_create",
    "pthread_join",
    "pthread_detach",
    "pthread_cond_timedwait",
    "pthread_timedjoin_np",
    "thrd_create",
    "clone",
    "clone3",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub symbol: String,
    pub line: String,
}

#[derive(Debug)]
pub enum AuditError {
    MissingBinary(PathBuf),
    RunNm(std::io::Error),
    NmFailed { status: String, stderr: String },
    BannedSymbols(Vec<Finding>),
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::MissingBinary(path) => {
                write!(f, "binary does not exist: {}", path.display())
            }
            AuditError::RunNm(err) => write!(f, "failed to run nm: {err}"),
            AuditError::NmFailed { status, stderr } => {
                write!(f, "nm exited with {status}: {}", stderr.trim())
            }
            AuditError::BannedSymbols(findings) => {
                write!(f, "{} banned symbol(s) found", findings.len())
            }
        }
    }
}

impl std::error::Error for AuditError {}

pub fn run_audit_syms(bin: &Path) -> Result<(), AuditError> {
    if !bin.exists() {
        return Err(AuditError::MissingBinary(bin.to_owned()));
    }

    let output = Command::new("nm")
        .arg("-a")
        .arg(bin)
        .output()
        .map_err(AuditError::RunNm)?;
    if !output.status.success() {
        return Err(AuditError::NmFailed {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let findings = find_banned_symbols(&stdout);
    if findings.is_empty() {
        println!("audit-syms: OK - no banned symbols in {}", bin.display());
        Ok(())
    } else {
        eprintln!(
            "audit-syms: FAILED - {} banned symbol(s) in {}:",
            findings.len(),
            bin.display()
        );
        for finding in &findings {
            eprintln!("  {} ({})", finding.symbol, finding.line.trim());
        }
        Err(AuditError::BannedSymbols(findings))
    }
}

pub fn find_banned_symbols(nm_output: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();

    for line in nm_output.lines() {
        let Some(symbol) = symbol_from_nm_line(line) else {
            continue;
        };
        let symbol = normalize_symbol(symbol);
        if BANNED_SYMBOLS.contains(&symbol) && seen.insert(symbol.to_string()) {
            findings.push(Finding {
                symbol: symbol.to_string(),
                line: line.to_string(),
            });
        }
    }

    findings
}

fn symbol_from_nm_line(line: &str) -> Option<&str> {
    let symbol = line.split_whitespace().last()?;
    if symbol.ends_with(':') {
        None
    } else {
        Some(symbol)
    }
}

fn normalize_symbol(symbol: &str) -> &str {
    symbol.split('@').next().unwrap_or(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_exact_banned_symbols_with_versions() {
        let output = "\
                 U clock_gettime@@GLIBC_2.17
                 U pthread_create@GLIBC_2.34
        000000000001d1e0 t register_tm_clones
        0000000000000000 T clone
        ";

        let findings = find_banned_symbols(output);
        let symbols: Vec<&str> = findings
            .iter()
            .map(|finding| finding.symbol.as_str())
            .collect();

        assert_eq!(symbols, vec!["clock_gettime", "pthread_create", "clone"]);
    }

    #[test]
    fn allows_runtime_support_and_partial_names() {
        let output = "\
                 U pthread_self@GLIBC_2.2.5
                 U pthread_getattr_np@GLIBC_2.32
        000000000001d1b0 t deregister_tm_clones
        000000000001d1e0 t register_tm_clones
        0000000000000000 T std_sys_thread_marker
        ";

        assert!(find_banned_symbols(output).is_empty());
    }

    #[test]
    fn deduplicates_findings() {
        let output = "\
                 U nanosleep@@GLIBC_2.2.5
                 U nanosleep@@GLIBC_2.2.5
        ";

        let findings = find_banned_symbols(output);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].symbol, "nanosleep");
    }
}
