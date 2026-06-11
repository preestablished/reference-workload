//! `spc-tests`: the SPC700 single-step JSON corpus runner
//! (https://github.com/SingleStepTests/spc700, pinned in `test-roms.lock`
//! as `spc700-singlestep`).
//!
//! The corpus is the M2 audio-CPU acceptance gate: 256 JSON files (one per
//! opcode byte), 1,000 cases each:
//!
//! ```json
//! {
//!   "name": "XX NNNN",
//!   "initial": { "pc": N, "a": N, "x": N, "y": N, "sp": N, "psw": N,
//!                "ram": [[addr, val], ...] },
//!   "final":   { same fields },
//!   "cycles":  [...]
//! }
//! ```
//!
//! Cycle traces are not deserialized (state-only gate, same rationale as the
//! 65816 runner).
//!
//! In **corpus mode** the SPC700 core treats $F0–$FF as plain flat RAM so the
//! corpus's bare-64-KiB model matches. This is controlled by the
//! `Spc700::corpus_mode` flag, set via `Apu::new_corpus()`.

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
    /// Stop after this many failures (0 = unlimited).
    pub max_fail: usize,
}

impl Default for SpcTestOpts {
    fn default() -> Self {
        SpcTestOpts {
            dir: PathBuf::from("target/test-roms/spc700-singlestep"),
            filter: None,
            max_fail: 0,
        }
    }
}

/// Per-file result for reporting.
struct FileResult {
    file: String,
    passed: usize,
    failed: usize,
    errors: Vec<String>,
}

/// Run the SPC700 single-step corpus. Returns `Ok(())` on all-pass, `Err(n)`
/// where n is the total failure count.
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

    run_corpus_execution(opts, &json_files)
}

// ─── Execution gate ───────────────────────────────────────────────────────────

fn run_corpus_execution(opts: &SpcTestOpts, json_files: &[PathBuf]) -> Result<(), usize> {
    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut file_results: Vec<FileResult> = Vec::new();

    'files: for path in json_files {
        let fname = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spc-tests: cannot read {}: {}", fname, e);
                total_fail += 1;
                continue;
            }
        };

        let cases: Vec<SpcTestCase> = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("spc-tests: cannot parse {}: {}", fname, e);
                total_fail += 1;
                continue;
            }
        };

        let mut fr = FileResult {
            file: fname.clone(),
            passed: 0,
            failed: 0,
            errors: Vec::new(),
        };

        for tc in &cases {
            match run_single_test(tc) {
                Ok(()) => {
                    fr.passed += 1;
                    total_pass += 1;
                }
                Err(msg) => {
                    fr.failed += 1;
                    total_fail += 1;
                    if fr.errors.len() < 5 {
                        fr.errors.push(format!("[{}] {}", tc.name, msg));
                    }
                    if opts.max_fail > 0 && total_fail >= opts.max_fail {
                        file_results.push(fr);
                        break 'files;
                    }
                }
            }
        }

        file_results.push(fr);
    }

    println!("\nspc-tests results:");
    println!("{:<40} {:>8} {:>8}", "file", "passed", "failed");
    println!("{}", "-".repeat(60));
    for fr in &file_results {
        println!("{:<40} {:>8} {:>8}", fr.file, fr.passed, fr.failed);
        for e in fr.errors.iter().take(3) {
            println!("    {}", e);
        }
        if fr.errors.len() > 3 {
            println!("    ... ({} more)", fr.errors.len() - 3);
        }
    }
    println!("{}", "-".repeat(60));
    println!("TOTAL: {} passed, {} failed", total_pass, total_fail);

    if total_fail == 0 {
        println!("spc-tests: ALL PASSED");
        Ok(())
    } else {
        eprintln!("spc-tests: {} FAILURE(S)", total_fail);
        Err(total_fail)
    }
}

// ─── Single-test executor ────────────────────────────────────────────────────

/// Execute one corpus case against the SPC700 core (corpus mode: flat 64 KiB
/// RAM, no I/O overlay). Compare registers and all `final.ram` entries.
///
/// Requires `--features refwork-emu/introspect`.
fn run_single_test(tc: &SpcTestCase) -> Result<(), String> {
    use refwork_emu::introspect::Spc700;

    // Allocate a flat 64 KiB memory image for this test.
    let mut mem: Box<[u8; 0x10000]> = Box::new([0u8; 0x10000]);

    // Load initial RAM.
    for pair in &tc.initial.ram {
        mem[pair[0] as usize] = pair[1] as u8;
    }

    // Set registers from initial state.
    let mut cpu = Spc700::new_corpus();
    cpu.pc = tc.initial.pc;
    cpu.a = tc.initial.a;
    cpu.x = tc.initial.x;
    cpu.y = tc.initial.y;
    cpu.sp = tc.initial.sp;
    cpu.psw = tc.initial.psw;

    // Step once.
    let _ = cpu.step(&mut mem);

    // Compare registers.
    let mut errors: Vec<String> = Vec::new();

    macro_rules! check {
        ($field:ident, $exp:expr, $actual:expr, $fmt:literal) => {
            if $actual != $exp {
                errors.push(format!(
                    concat!("{}: expected ", $fmt, " got ", $fmt),
                    stringify!($field),
                    $exp,
                    $actual
                ));
            }
        };
    }

    check!(pc, tc.expected.pc, cpu.pc, "${:04X}");
    check!(a, tc.expected.a, cpu.a, "${:02X}");
    check!(x, tc.expected.x, cpu.x, "${:02X}");
    check!(y, tc.expected.y, cpu.y, "${:02X}");
    check!(sp, tc.expected.sp, cpu.sp, "${:02X}");
    check!(psw, tc.expected.psw, cpu.psw, "${:02X}");

    // Compare final RAM entries.
    for pair in &tc.expected.ram {
        let addr = pair[0] as usize;
        let expected = pair[1] as u8;
        let actual = mem[addr];
        if actual != expected {
            errors.push(format!(
                "ram[${:04X}]: expected ${:02X} got ${:02X}",
                addr, expected, actual
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

// ─── Range validation (kept for backward compat / schema checks) ─────────────

/// Range-check parsed cases: every RAM address must fit the SPC700's 64 KiB
/// space and every value must be a byte.
#[cfg(test)]
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

    /// Smoke-test the executor on the NOP case (opcode $00).
    #[test]
    fn executes_nop_case() {
        let cases: Vec<SpcTestCase> = serde_json::from_str(SAMPLE).unwrap();
        assert!(
            run_single_test(&cases[0]).is_ok(),
            "NOP corpus case should pass"
        );
    }
}
