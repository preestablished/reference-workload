//! `refwork-verify` CLI — hand-rolled argument parser (no clap).
//!
//! Subcommands:
//!
//!   play --rom <file> --script <run.padlog>
//!        [--map <yaml>] [--snap <frame>=<out.bin> ...]
//!        [--watch <feature> ...] [--hash-every N]
//!        [--frames N] [--report out.json]
//!        [--continue-past-faults]
//!
//!   map-check --rom <file> --map <yaml> --script <run.padlog>
//!             --expect <expectations.yaml>
//!
//!   double-run --rom <file> --script <run.padlog>
//!              [--frames N] [--report out.json]
//!              [--nondet-test   (TEST-ONLY)]
//!
//! Pad policy (frames beyond script length): the last pad word in the script
//! is held for all remaining frames.  This matches the xtask hash-chain
//! policy and makes double-run reproducible.

#![forbid(unsafe_code)]

use refwork_featuremap::parse_feature_map;
use refwork_script::parse as parse_padlog;
use refwork_verify::double_run::double_run;
use refwork_verify::expectations::parse_expectations;
use refwork_verify::map_check::{map_check, MapCheckResult};
use refwork_verify::play::{play, PlayOptions};
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
        process::exit(1);
    }
    match args[0].as_str() {
        "play" => cmd_play(&args[1..]),
        "map-check" => cmd_map_check(&args[1..]),
        "double-run" => cmd_double_run(&args[1..]),
        "--help" | "-h" | "help" => {
            usage();
        }
        other => {
            eprintln!("refwork-verify: unknown subcommand '{}'", other);
            usage();
            process::exit(1);
        }
    }
}

fn usage() {
    println!("Usage: refwork-verify <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("Subcommands:");
    println!();
    println!("  play --rom <file> --script <run.padlog>");
    println!("       [--map <feature-map.yaml>]");
    println!("       [--snap <frame>=<out.bin> ...]   framebuffer snapshot at frame N");
    println!("       [--watch <feature> ...]           print change events for feature");
    println!("       [--hash-every N]                  print chain hash every N frames");
    println!("       [--frames N]                      run N frames (default: script length)");
    println!("       [--report <out.json>]             write JSON report");
    println!("       [--continue-past-faults]          LAB-ONLY recon: keep running on fault");
    println!();
    println!("  Pad policy: frames beyond script length hold the last pad word.");
    println!();
    println!("  map-check --rom <file> --map <feature-map.yaml>");
    println!("            --script <run.padlog> --expect <expectations.yaml>");
    println!("  Rejects --continue-past-faults artifacts.");
    println!();
    println!("  double-run --rom <file> --script <run.padlog>");
    println!("             [--frames N]");
    println!("             [--report <out.json>]");
    println!("             [--nondet-test]   TEST-ONLY: perturb run-2 pad stream");
    println!("  Rejects --continue-past-faults artifacts.");
}

// ─── play ─────────────────────────────────────────────────────────────────────

fn cmd_play(args: &[String]) {
    let mut rom_path: Option<PathBuf> = None;
    let mut script_path: Option<PathBuf> = None;
    let mut map_path: Option<PathBuf> = None;
    let mut snaps: Vec<(u64, String)> = Vec::new();
    let mut watch: Vec<String> = Vec::new();
    let mut hash_every: u64 = 0;
    let mut frames: u64 = 0;
    let mut report_path: Option<String> = None;
    let mut continue_past_faults = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom_path = Some(require_arg("play", "--rom", args, i));
            }
            "--script" => {
                i += 1;
                script_path = Some(require_arg("play", "--script", args, i));
            }
            "--map" => {
                i += 1;
                map_path = Some(require_arg("play", "--map", args, i));
            }
            "--snap" => {
                i += 1;
                let val = require_arg_str("play", "--snap", args, i);
                match val.split_once('=') {
                    Some((frame_str, out)) => {
                        let frame: u64 = frame_str.parse().unwrap_or_else(|_| {
                            eprintln!("play: --snap frame '{}' is not an integer", frame_str);
                            process::exit(1);
                        });
                        snaps.push((frame, out.to_string()));
                    }
                    None => {
                        eprintln!("play: --snap requires <frame>=<path>");
                        process::exit(1);
                    }
                }
            }
            "--watch" => {
                i += 1;
                watch.push(require_arg_str("play", "--watch", args, i).to_string());
            }
            "--hash-every" => {
                i += 1;
                let n = require_arg_str("play", "--hash-every", args, i);
                hash_every = n.parse().unwrap_or_else(|_| {
                    eprintln!("play: --hash-every requires a positive integer");
                    process::exit(1);
                });
            }
            "--frames" => {
                i += 1;
                let n = require_arg_str("play", "--frames", args, i);
                frames = n.parse().unwrap_or_else(|_| {
                    eprintln!("play: --frames requires a positive integer");
                    process::exit(1);
                });
            }
            "--report" => {
                i += 1;
                report_path = Some(require_arg_str("play", "--report", args, i).to_string());
            }
            "--continue-past-faults" => {
                continue_past_faults = true;
            }
            other => {
                eprintln!("play: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let rom_path = rom_path.unwrap_or_else(|| {
        eprintln!("play: --rom is required");
        process::exit(1);
    });
    let script_path = script_path.unwrap_or_else(|| {
        eprintln!("play: --script is required");
        process::exit(1);
    });

    let rom = load_file(&rom_path);
    let script_text = load_text(&script_path);
    let script = parse_padlog(&script_text).unwrap_or_else(|e| {
        eprintln!("play: script parse error: {}", e);
        process::exit(1);
    });

    let map_parsed;
    let feature_map = if let Some(mp) = map_path {
        let text = load_text(&mp);
        let (m, errs) = parse_feature_map(&text).unwrap_or_else(|e| {
            eprintln!("play: feature map parse error: {}", e);
            process::exit(1);
        });
        if !errs.is_empty() {
            for e in &errs {
                eprintln!("play: feature map warning: {}", e);
            }
        }
        map_parsed = m;
        Some(&map_parsed)
    } else {
        None
    };

    let mut opts = PlayOptions::new(rom, &script);
    opts.feature_map = feature_map;
    opts.watch = watch;
    opts.hash_every = hash_every;
    opts.frames = frames;
    opts.continue_past_faults = continue_past_faults;
    opts.snaps = snaps;
    opts.report_path = report_path;
    opts.on_feature_change = Some(Box::new(|frame, name, old, new| {
        println!("frame {}: {} changed {} -> {}", frame, name, old, new);
    }));
    opts.on_fault = Some(Box::new(|frame, desc| {
        eprintln!("FAULT at frame {}: {}", frame, desc);
    }));
    opts.on_hash = Some(Box::new(|frame, hex| {
        println!("hash-chain: frame={} chain={}", frame, hex);
    }));

    match play(opts) {
        Ok(report) => {
            println!(
                "play: done — frames={} chain={}",
                report.final_frame, report.final_chain_hash
            );
            if !report.faults.is_empty() {
                eprintln!("play: {} fault(s) recorded", report.faults.len());
                if !continue_past_faults {
                    process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("play: error: {}", e);
            process::exit(1);
        }
    }
}

// ─── map-check ────────────────────────────────────────────────────────────────

fn cmd_map_check(args: &[String]) {
    let mut rom_path: Option<PathBuf> = None;
    let mut map_path: Option<PathBuf> = None;
    let mut script_path: Option<PathBuf> = None;
    let mut expect_path: Option<PathBuf> = None;
    let mut continue_past_faults = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom_path = Some(require_arg("map-check", "--rom", args, i));
            }
            "--map" => {
                i += 1;
                map_path = Some(require_arg("map-check", "--map", args, i));
            }
            "--script" => {
                i += 1;
                script_path = Some(require_arg("map-check", "--script", args, i));
            }
            "--expect" => {
                i += 1;
                expect_path = Some(require_arg("map-check", "--expect", args, i));
            }
            "--continue-past-faults" => {
                continue_past_faults = true;
            }
            other => {
                eprintln!("map-check: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Reject --continue-past-faults.
    if continue_past_faults {
        eprintln!(
            "map-check: --continue-past-faults is rejected by map-check. \
             This flag is for lab reconnaissance only."
        );
        process::exit(1);
    }

    let rom_path = rom_path.unwrap_or_else(|| {
        eprintln!("map-check: --rom is required");
        process::exit(1);
    });
    let map_path = map_path.unwrap_or_else(|| {
        eprintln!("map-check: --map is required");
        process::exit(1);
    });
    let script_path = script_path.unwrap_or_else(|| {
        eprintln!("map-check: --script is required");
        process::exit(1);
    });
    let expect_path = expect_path.unwrap_or_else(|| {
        eprintln!("map-check: --expect is required");
        process::exit(1);
    });

    let rom = load_file(&rom_path);
    let script_text = load_text(&script_path);
    let script = parse_padlog(&script_text).unwrap_or_else(|e| {
        eprintln!("map-check: script parse error: {}", e);
        process::exit(1);
    });
    let map_text = load_text(&map_path);
    let (feature_map, map_errs) = parse_feature_map(&map_text).unwrap_or_else(|e| {
        eprintln!("map-check: feature map parse error: {}", e);
        process::exit(1);
    });
    if !map_errs.is_empty() {
        for e in &map_errs {
            eprintln!("map-check: feature map warning: {}", e);
        }
    }
    let expect_text = load_text(&expect_path);
    let expectations = match parse_expectations(&expect_text) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("map-check: expectations parse error: {}", e);
            process::exit(2);
        }
    };

    match map_check(rom, &script, &feature_map, &expectations, None) {
        Ok(MapCheckResult::Pass) => {
            println!("map-check: PASS");
        }
        Ok(MapCheckResult::ExpectationsError(msg)) => {
            eprintln!("map-check: expectations error: {}", msg);
            process::exit(2);
        }
        Ok(MapCheckResult::Failure {
            frame,
            feature,
            expected_description,
            actual,
            raw_bytes,
        }) => {
            eprintln!(
                "map-check: FAIL at frame {} — feature {:?}: expected {}, got {} (raw: {:?})",
                frame, feature, expected_description, actual, raw_bytes
            );
            process::exit(1);
        }
        Err(e) => {
            eprintln!("map-check: error: {}", e);
            process::exit(1);
        }
    }
}

// ─── double-run ───────────────────────────────────────────────────────────────

fn cmd_double_run(args: &[String]) {
    let mut rom_path: Option<PathBuf> = None;
    let mut script_path: Option<PathBuf> = None;
    let mut frames: u64 = 0;
    let mut report_path: Option<String> = None;
    let mut nondet_test = false;
    let mut continue_past_faults = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom_path = Some(require_arg("double-run", "--rom", args, i));
            }
            "--script" => {
                i += 1;
                script_path = Some(require_arg("double-run", "--script", args, i));
            }
            "--frames" => {
                i += 1;
                let n = require_arg_str("double-run", "--frames", args, i);
                frames = n.parse().unwrap_or_else(|_| {
                    eprintln!("double-run: --frames requires a positive integer");
                    process::exit(1);
                });
            }
            "--report" => {
                i += 1;
                report_path = Some(require_arg_str("double-run", "--report", args, i).to_string());
            }
            "--nondet-test" => {
                // TEST-ONLY: perturb run-2 pad stream to trigger a divergence.
                nondet_test = true;
            }
            "--continue-past-faults" => {
                continue_past_faults = true;
            }
            other => {
                eprintln!("double-run: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Reject --continue-past-faults.
    if continue_past_faults {
        eprintln!(
            "double-run: --continue-past-faults is rejected by double-run. \
             A faulted core produces non-authoritative state."
        );
        process::exit(1);
    }

    // Also check the environment variable alias.
    if std::env::var("REFWORK_NONDET_TEST").as_deref() == Ok("1") {
        nondet_test = true;
    }

    let rom_path = rom_path.unwrap_or_else(|| {
        eprintln!("double-run: --rom is required");
        process::exit(1);
    });
    let script_path = script_path.unwrap_or_else(|| {
        eprintln!("double-run: --script is required");
        process::exit(1);
    });

    let rom = load_file(&rom_path);
    let script_text = load_text(&script_path);
    let script = parse_padlog(&script_text).unwrap_or_else(|e| {
        eprintln!("double-run: script parse error: {}", e);
        process::exit(1);
    });

    if nondet_test {
        eprintln!();
        eprintln!("WARNING: --nondet-test / REFWORK_NONDET_TEST is active.");
        eprintln!("         This is a TEST-ONLY mode that deliberately perturbs run 2.");
        eprintln!("         The result MUST NOT be used for acceptance or CI testing.");
        eprintln!();
    }

    let report = match double_run(rom, &script, frames, nondet_test) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("double-run: error: {}", e);
            process::exit(1);
        }
    };

    if let Some(path) = &report_path {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            eprintln!("double-run: report serialization failed: {}", e);
            process::exit(1);
        });
        if let Err(e) = std::fs::write(path, json) {
            eprintln!("double-run: cannot write report to {}: {}", path, e);
            process::exit(1);
        }
    }

    if report.deterministic {
        println!(
            "double-run: PASS — deterministic over {} frames (chain={})",
            report.frames_run, report.chain_a
        );
    } else {
        eprintln!(
            "double-run: FAIL — divergence at frame {:?} ({} vs {}), region: {:?}",
            report.first_divergent_frame, report.chain_a, report.chain_b, report.divergent_region
        );
        process::exit(1);
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn require_arg(cmd: &str, flag: &str, args: &[String], i: usize) -> PathBuf {
    if i >= args.len() {
        eprintln!("{}: {} requires a path argument", cmd, flag);
        process::exit(1);
    }
    PathBuf::from(&args[i])
}

fn require_arg_str<'a>(cmd: &str, flag: &str, args: &'a [String], i: usize) -> &'a str {
    if i >= args.len() {
        eprintln!("{}: {} requires an argument", cmd, flag);
        process::exit(1);
    }
    &args[i]
}

fn load_file(path: &PathBuf) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read '{}': {}", path.display(), e);
        process::exit(1);
    })
}

fn load_text(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("error: cannot read '{}': {}", path.display(), e);
        process::exit(1);
    })
}
