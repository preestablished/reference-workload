//! `spc-tests`: the SPC700 single-step JSON corpus runner
//! (https://github.com/SingleStepTests/spc700, pinned in `test-roms.lock`
//! as `spc700-singlestep`).
//!
//! The corpus is the planned M2 audio-CPU acceptance gate: 256 JSON files
//! (one per opcode byte), 1,000 cases each, in the same shape as the 65816
//! corpus the `cpu-tests` runner consumes:
//!
//! ```json
//! {
//!   "name": "00 0000",
//!   "initial": { "pc": N, "a": N, "x": N, "y": N, "sp": N, "psw": N,
//!                "ram": [[addr, val], ...] },
//!   "final":   { same fields },
//!   "cycles":  [...]
//! }
//! ```
//!
//! Until the M2 SPC700 core exists in `refwork-emu`, this runner operates in
//! corpus-validation mode: it parses every file, validates the schema and
//! value ranges (16-bit pc, 8-bit registers, 64 KiB address space), and
//! reports case counts — proving the pinned archive is healthy and the
//! parse path works. `run_corpus_execution` is the single seam where the M2
//! core plugs in; it mirrors `cpu_tests::run_single_test` (set state, step
//! once, compare registers + listed ram bytes; cycle traces ignored for the
//! same documented reason as the 65816 runner).

use std::path::PathBuf;

use serde::Deserialize;

// ─── JSON schema ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SpcTestCase {
    pub name: String,
    pub initial: SpcState,
    #[serde(rename = "final")]
    pub expected: SpcState,
    // `cycles` exists in the corpus but is not deserialized (state-only gate).
}

/// Architectural SPC700 state as the corpus represents it.
#[derive(Debug, Deserialize)]
pub struct SpcState {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    /// Processor status word (N V P B H I Z C).
    pub psw: u8,
    /// Sparse RAM contents: `[address, value]` pairs in a 64 KiB space.
    pub ram: Vec<[u64; 2]>,
}

pub struct SpcTestOpts {
    /// Corpus directory (default: target/test-roms/spc700-singlestep).
    pub dir: PathBuf,
    /// Only files whose name contains this substring.
    pub filter: Option<String>,
}

impl Default for SpcTestOpts {
    fn default() -> Self {
        SpcTestOpts {
            dir: PathBuf::from("target/test-roms/spc700-singlestep"),
            filter: None,
        }
    }
}

/// Validate the pinned corpus: parse every JSON file, check schema + ranges,
/// report counts. Returns `Err(n)` with a failure count for CI.
///
/// Once the M2 SPC700 core lands, `run_corpus_execution` replaces the tail
/// of this function as the acceptance gate proper.
pub fn run_spc_tests(opts: &SpcTestOpts) -> Result<(), usize> {
    let mut dir = opts.dir.clone();
    if !dir.exists() {
        eprintln!("spc-tests: directory '{}' not found.", dir.display());
        eprintln!("Run `cargo xtask fetch-test-roms` first, or pass --dir to a local corpus.");
        return Err(1);
    }
    // The published corpus keeps its JSON files under a `v1/` subdirectory.
    if dir.join("v1").is_dir() {
        dir = dir.join("v1");
    }

    let mut json_files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect(),
        Err(e) => {
            eprintln!("spc-tests: cannot read dir: {}", e);
            return Err(1);
        }
    };
    json_files.sort();
    if let Some(f) = &opts.filter {
        json_files.retain(|p| p.to_string_lossy().contains(f.as_str()));
    }
    if json_files.is_empty() {
        eprintln!("spc-tests: no JSON files found in '{}'.", dir.display());
        return Err(1);
    }

    let mut total_cases = 0usize;
    let mut bad_files = 0usize;
    for path in &json_files {
        let fname = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spc-tests: cannot read {}: {}", fname, e);
                bad_files += 1;
                continue;
            }
        };
        let cases: Vec<SpcTestCase> = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spc-tests: cannot parse {}: {}", fname, e);
                bad_files += 1;
                continue;
            }
        };
        if let Err(msg) = validate_cases(&cases) {
            eprintln!("spc-tests: {}: {}", fname, msg);
            bad_files += 1;
            continue;
        }
        total_cases += cases.len();
    }

    println!(
        "spc-tests: corpus OK — {} files, {} cases parsed and range-checked.",
        json_files.len() - bad_files,
        total_cases
    );
    println!(
        "spc-tests: execution gate not yet active — the SPC700 core arrives \
         with milestone M2; this run validates the pinned corpus only."
    );

    if bad_files > 0 {
        eprintln!("spc-tests: {} file(s) failed validation.", bad_files);
        return Err(bad_files);
    }
    Ok(())
}

/// Range-check parsed cases: every RAM address must fit the SPC700's 64 KiB
/// space and every value must be a byte.
fn validate_cases(cases: &[SpcTestCase]) -> Result<(), String> {
    if cases.is_empty() {
        return Err("file contains no test cases".into());
    }
    for tc in cases {
        for st in [&tc.initial, &tc.expected] {
            for pair in &st.ram {
                if pair[0] > 0xFFFF {
                    return Err(format!(
                        "[{}] ram address {:#X} exceeds 64 KiB",
                        tc.name, pair[0]
                    ));
                }
                if pair[1] > 0xFF {
                    return Err(format!(
                        "[{}] ram value {:#X} exceeds a byte",
                        tc.name, pair[1]
                    ));
                }
            }
        }
    }
    Ok(())
}

/// M2 seam: execute one corpus case against the SPC700 core and compare
/// final state (registers + every `[addr, value]` listed in `final.ram`).
/// Mirrors `cpu_tests::run_single_test`. Compiled but unreachable until the
/// core exists; kept here so the M2 change is "fill this in", not "design a
/// runner".
#[allow(dead_code)]
fn run_corpus_execution(_tc: &SpcTestCase) -> Result<(), String> {
    Err("SPC700 core not yet implemented (arrives with milestone M2)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative corpus case (shape verified against the pinned
    /// SingleStepTests/spc700 v1 files).
    const SAMPLE: &str = r#"[
      {
        "name": "00 0000",
        "initial": { "pc": 30256, "a": 56, "x": 78, "y": 127, "sp": 236,
                     "psw": 145, "ram": [[30256, 0]] },
        "final":   { "pc": 30257, "a": 56, "x": 78, "y": 127, "sp": 236,
                     "psw": 145, "ram": [[30256, 0]] },
        "cycles":  [[30256, 0, "read"], [30257, null, "read"]]
      }
    ]"#;

    #[test]
    fn parses_corpus_shape() {
        let cases: Vec<SpcTestCase> = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].name, "00 0000");
        assert_eq!(cases[0].initial.pc, 30256);
        assert_eq!(cases[0].expected.pc, 30257);
        assert_eq!(cases[0].initial.psw, 145);
        validate_cases(&cases).unwrap();
    }

    #[test]
    fn rejects_out_of_range_ram() {
        let bad = SAMPLE.replace("[30256, 0]", "[70000, 0]");
        let cases: Vec<SpcTestCase> = serde_json::from_str(&bad).unwrap();
        assert!(validate_cases(&cases).is_err());
    }
}
