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
//!   trace --captures <capture-index.jsonl> --map <feature-map.yaml>
//!         --scoring <scoring-program.yaml> --labels <labels.yaml>
//!         --out <trajectory.jsonl> --report <trace-report.json>
//!
//!   double-run --rom <file> --script <run.padlog>
//!              [--frames N] [--report out.json]
//!              [--nondet-test   (TEST-ONLY)]
//!
//!   phase4-bundle-check --bundle <private-bundle-dir> [--report out.json]
//!
//!   phase4-checksum-manifest --bundle <private-bundle-dir> --out out.json
//!
//!   phase4-context-check --bundle <private-context-dir> [--report out.json]
//!
//!   phase4-layout --map <feature-map.yaml> --out layout.json
//!                 --capture-spec-hash <hash-or-ref>
//!                 [--layout-version N]
//!                 [--compiler-or-exporter-commit <text>]
//!
//!   phase4-private-intake --private-root <private-root>
//!                         [--rom-dir <rom-dir>] --operator-approved
//!                         [--operator-metadata-policy <text>]
//!                         [--operator-label <text>]
//!
//!   phase4-score-plan --captures <capture-index.jsonl> --out score-plan.json
//!                     --first-boss <capture-id>
//!                     --goal-positive <capture-id> --goal-negative <capture-id>
//!
//!   redaction-scan --input <public-note.md> [--report out.json]
//!                  [--forbid <literal> ...] [--forbid-file <private-list>]
//!
//!   vm-first-room --worker <uds-path|http://host:port>
//!                 --snapshot-ref <64-hex> --script <first-room.padlog>
//!                 --map <feature-map.yaml> --expect <vm-expect.yaml>
//!                 --report <out.json>
//!                 [--image-manifest <workload-image.yaml>]
//!                 [--frames N] [--port N]
//!
//!   vm-suite --worker <uds-path|http://host:port>
//!            --snapshot-ref <64-hex> --script <run.padlog>
//!            --report <out.json>
//!            [--ram-region wram] [--ram-size 131072]
//!            [--frames N] [--snapshot-at M] [--iterations K]
//!            [--nondet-test   (TEST-ONLY)] [--port N]
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
use refwork_verify::phase4_artifact_check::check_phase4_artifacts;
use refwork_verify::phase4_bundle_check::check_phase4_bundle;
use refwork_verify::phase4_capture_export::{export_phase4_captures, CaptureExportOptions};
use refwork_verify::phase4_checksum_manifest::{
    set_phase4_payload_root, verify_phase4_checksum_manifest, write_phase4_checksum_manifest,
    ChecksumManifestOptions,
};
use refwork_verify::phase4_context_check::check_phase4_context_bundle;
use refwork_verify::phase4_context_export::{export_phase4_context, ContextExportOptions};
use refwork_verify::phase4_fallback_check::check_phase4_fallback;
use refwork_verify::phase4_layout::{write_phase4_layout, LayoutOptions};
use refwork_verify::phase4_private_intake::{prepare_phase4_private_intake, PrivateIntakeOptions};
use refwork_verify::phase4_score_plan::{write_phase4_score_plan, ScorePlanOptions};
use refwork_verify::phase4_trace::{emit_phase4_trace, TraceOptions};
use refwork_verify::play::{play, PlayOptions};
use refwork_verify::redaction_scan::{
    load_forbidden_literals, scan_redactions, RedactionScanOptions,
};
use refwork_verify::vm_first_room::{vm_first_room, write_report, VmFirstRoomOptions};
use refwork_verify::vm_suite::{vm_suite, write_suite_report, VmSuiteOptions};
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
        "trace" => cmd_trace(&args[1..]),
        "double-run" => cmd_double_run(&args[1..]),
        "phase4-bundle-check" => cmd_phase4_bundle_check(&args[1..]),
        "phase4-artifact-check" => cmd_phase4_artifact_check(&args[1..]),
        "phase4-capture-export" => cmd_phase4_capture_export(&args[1..]),
        "phase4-checksum-manifest" => cmd_phase4_checksum_manifest(&args[1..]),
        "phase4-context-check" => cmd_phase4_context_check(&args[1..]),
        "phase4-context-export" => cmd_phase4_context_export(&args[1..]),
        "phase4-fallback-check" => cmd_phase4_fallback_check(&args[1..]),
        "phase4-layout" => cmd_phase4_layout(&args[1..]),
        "phase4-private-intake" => cmd_phase4_private_intake(&args[1..]),
        "phase4-score-plan" => cmd_phase4_score_plan(&args[1..]),
        "redaction-scan" => cmd_redaction_scan(&args[1..]),
        "vm-first-room" => cmd_vm_first_room(&args[1..]),
        "vm-suite" => cmd_vm_suite(&args[1..]),
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
    println!("  trace --captures <capture-index.jsonl> --map <feature-map.yaml>");
    println!("        --scoring <scoring-program.yaml> --labels <labels.yaml>");
    println!("        --out <trajectory.jsonl> --report <trace-report.json>");
    println!();
    println!("  double-run --rom <file> --script <run.padlog>");
    println!("             [--frames N]");
    println!("             [--report <out.json>]");
    println!("             [--nondet-test]   TEST-ONLY: perturb run-2 pad stream");
    println!("  Rejects --continue-past-faults artifacts.");
    println!();
    println!("  phase4-bundle-check --bundle <private-bundle-dir>");
    println!("                      [--report <out.json>]");
    println!();
    println!("  phase4-artifact-check --bundle <private-bundle-dir> --report <out.json>");
    println!();
    println!("  phase4-capture-export --endpoint <worker> --snapshot <blake3> --padlog <file>");
    println!(
        "                        --map <feature-map.yaml> --layout <layout.json> --bundle <dir>"
    );
    println!(
        "                        --count N --cadence N --hard-icount-cap N --source-ref <opaque>"
    );
    println!();
    println!("  phase4-checksum-manifest --bundle <private-bundle-dir>");
    println!("                           (--set-payload-root | --out <out.json> | --verify <freeze.json>)");
    println!();
    println!("  phase4-context-check --bundle <private-context-dir>");
    println!("                       [--report <out.json>]");
    println!();
    println!("  phase4-context-export --corpus <bundle> --out <context-dir> --capture-id <id> ...");
    println!("                        --context-artifact-id <opaque> --access-requirement <role>");
    println!("                        --retention <text> --pad-table-hash <blake3> [--recent-input <padlog>]");
    println!();
    println!("  phase4-fallback-check --bundle <private-bundle-dir> [--report <out.json>]");
    println!();
    println!("  phase4-layout --map <feature-map.yaml> --out <layout.json>");
    println!("                --capture-spec-hash <hash-or-ref>");
    println!("                [--layout-version N]");
    println!("                [--compiler-or-exporter-commit <text>]");
    println!();
    println!("  phase4-private-intake --private-root <private-root>");
    println!("                        [--rom-dir <rom-dir>] --operator-approved");
    println!("                        [--operator-metadata-policy <text>]");
    println!("                        [--operator-label <text>]");
    println!();
    println!("  phase4-score-plan --captures <capture-index.jsonl> --out <score-plan.json>");
    println!("                    [--client-batch-prefix <prefix>]");
    println!("                    --first-boss <capture-id>");
    println!("                    --goal-positive <capture-id> --goal-negative <capture-id>");
    println!("                    [--checkpoint-after-batch <client_batch_id>]");
    println!("                    [--restore-control-batch <client_batch_id> ...]");
    println!();
    println!("  redaction-scan --input <public-note.md>");
    println!("                 [--report <out.json>]");
    println!("                 [--forbid <literal> ...] [--forbid-file <private-list>]");
    println!();
    println!("  vm-first-room --worker <uds-path|http://host:port>");
    println!("                --snapshot-ref <64-hex BLAKE3 of the READY snapshot>");
    println!("                --script <first-room.padlog> --map <feature-map.yaml>");
    println!("                --expect <vm-expect.yaml> --report <out.json>");
    println!("                [--image-manifest <workload-image.yaml>]");
    println!("                [--frames N] [--port N]");
    println!("  Phase 3 exit gate 3: drives the first room in-VM through the worker");
    println!("  gRPC API (RestoreSnapshot -> InjectInputs -> Run -> GetFramebuffer).");
    println!("  The report carries hashes and decoded feature values only — never");
    println!("  pixels, WRAM dumps, ROM bytes, or padlog semantics.");
    println!();
    println!("  vm-suite --worker <uds-path|http://host:port>");
    println!("           --snapshot-ref <64-hex> --script <run.padlog>");
    println!("           --report <out.json>");
    println!("           [--ram-region wram] [--ram-size 131072]");
    println!("           [--frames N] [--snapshot-at M]");
    println!("           [--iterations K]   20 for the zero-flake stamp");
    println!("           [--nondet-test]    TEST-ONLY: perturb run-2 pad stream");
    println!("           [--port N]");
    println!("  Phase 3 exit gate 1 (M5): in-VM double-run + mid-game snapshot/");
    println!("  restore continuity, hashed host-side at every FrameMark. Not the");
    println!("  in-process double-run — this one runs through the worker gRPC API.");
}

fn cmd_phase4_artifact_check(args: &[String]) {
    let mut bundle = None;
    let mut report_path = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle = Some(require_arg("phase4-artifact-check", "--bundle", args, i));
            }
            "--report" => {
                i += 1;
                report_path = Some(require_arg("phase4-artifact-check", "--report", args, i));
            }
            other => {
                eprintln!("phase4-artifact-check: unknown option '{other}'");
                process::exit(1);
            }
        }
        i += 1;
    }
    let bundle = bundle.unwrap_or_else(|| missing_required("phase4-artifact-check", "--bundle"));
    let report = check_phase4_artifacts(&bundle);
    if let Some(path) = report_path {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            eprintln!("phase4-artifact-check: report serialization failed: {e}");
            process::exit(1);
        });
        if let Err(e) = std::fs::write(&path, json) {
            eprintln!("phase4-artifact-check: cannot write report: {e}");
            process::exit(1);
        }
    }
    if report.passed() {
        println!(
            "phase4-artifact-check: PASS — captures={} artifacts={}",
            report.capture_count,
            report.feature_artifact_count + report.framebuffer_artifact_count
        );
    } else {
        eprintln!(
            "phase4-artifact-check: FAIL — {} issue(s)",
            report.errors.len()
        );
        for error in report.errors {
            eprintln!("  - {error}");
        }
        process::exit(1);
    }
}

fn cmd_phase4_capture_export(args: &[String]) {
    let mut values = std::collections::BTreeMap::new();
    let mut production = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--production" {
            production = true;
            i += 1;
            continue;
        }
        let key = args[i].clone();
        if !key.starts_with("--") {
            eprintln!("phase4-capture-export: unexpected argument");
            process::exit(1);
        }
        if ![
            "--endpoint",
            "--snapshot",
            "--padlog",
            "--map",
            "--layout",
            "--bundle",
            "--count",
            "--cadence",
            "--hard-icount-cap",
            "--source-ref",
        ]
        .contains(&key.as_str())
        {
            eprintln!("phase4-capture-export: unknown option '{key}'");
            process::exit(1);
        }
        i += 1;
        values.insert(
            key,
            require_arg_str("phase4-capture-export", "value", args, i).to_owned(),
        );
        i += 1;
    }
    let take = |name: &str| {
        values.get(name).cloned().unwrap_or_else(|| {
            eprintln!("phase4-capture-export: {name} is required");
            process::exit(1)
        })
    };
    let number = |name: &str| -> u32 {
        take(name).parse().unwrap_or_else(|_| {
            eprintln!("phase4-capture-export: {name} must be an integer");
            process::exit(1)
        })
    };
    let opts = CaptureExportOptions {
        endpoint: take("--endpoint"),
        snapshot_hash: take("--snapshot"),
        padlog: take("--padlog").into(),
        map: take("--map").into(),
        layout: take("--layout").into(),
        bundle: take("--bundle").into(),
        count: number("--count"),
        cadence: number("--cadence"),
        hard_icount_cap: take("--hard-icount-cap")
            .parse::<u64>()
            .unwrap_or_else(|_| {
                eprintln!("phase4-capture-export: --hard-icount-cap must be an integer");
                process::exit(1)
            }),
        production,
        source_ref: take("--source-ref"),
    };
    let report = export_phase4_captures(&opts);
    let report_path = opts.bundle.join("validation/capture-export-report.json");
    if let Err(e) = std::fs::create_dir_all(report_path.parent().unwrap())
        .and_then(|_| std::fs::write(&report_path, serde_json::to_vec_pretty(&report).unwrap()))
    {
        eprintln!("phase4-capture-export: cannot write private export report: {e}");
        process::exit(1);
    }
    if report.passed() {
        println!(
            "phase4-capture-export: PASS — captures={}",
            report.completed
        )
    } else {
        eprintln!(
            "phase4-capture-export: FAIL — {} issue(s)",
            report.errors.len()
        );
        for e in report.errors {
            eprintln!("  - {e}")
        }
        process::exit(1)
    }
}

// ─── vm-suite ────────────────────────────────────────────────────────────────

fn cmd_vm_suite(args: &[String]) {
    let mut worker: Option<String> = None;
    let mut snapshot_ref: Option<String> = None;
    let mut script: Option<PathBuf> = None;
    let mut ram_region = "wram".to_owned();
    let mut ram_size: u32 = 131072;
    let mut frames: Option<u32> = None;
    let mut snapshot_at: Option<u32> = None;
    let mut iterations: u32 = 1;
    let mut report: Option<PathBuf> = None;
    let mut nondet_test = false;
    let mut port: u32 = 0;

    let parse_u32 = |flag: &str, value: &str| -> u32 {
        value.parse().unwrap_or_else(|_| {
            eprintln!("vm-suite: {flag} requires an unsigned integer");
            process::exit(1);
        })
    };

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--worker" => {
                i += 1;
                worker = Some(require_arg_str("vm-suite", "--worker", args, i).to_owned());
            }
            "--snapshot-ref" => {
                i += 1;
                snapshot_ref =
                    Some(require_arg_str("vm-suite", "--snapshot-ref", args, i).to_owned());
            }
            "--script" => {
                i += 1;
                script = Some(require_arg("vm-suite", "--script", args, i));
            }
            "--ram-region" => {
                i += 1;
                ram_region = require_arg_str("vm-suite", "--ram-region", args, i).to_owned();
            }
            "--ram-size" => {
                i += 1;
                ram_size = parse_u32(
                    "--ram-size",
                    require_arg_str("vm-suite", "--ram-size", args, i),
                );
            }
            "--frames" => {
                i += 1;
                frames = Some(parse_u32(
                    "--frames",
                    require_arg_str("vm-suite", "--frames", args, i),
                ));
            }
            "--snapshot-at" => {
                i += 1;
                snapshot_at = Some(parse_u32(
                    "--snapshot-at",
                    require_arg_str("vm-suite", "--snapshot-at", args, i),
                ));
            }
            "--iterations" => {
                i += 1;
                iterations = parse_u32(
                    "--iterations",
                    require_arg_str("vm-suite", "--iterations", args, i),
                );
            }
            "--report" => {
                i += 1;
                report = Some(require_arg("vm-suite", "--report", args, i));
            }
            "--nondet-test" => {
                // TEST-ONLY: perturb run-2 pad stream to prove the suite can fail.
                nondet_test = true;
            }
            "--port" => {
                i += 1;
                port = parse_u32("--port", require_arg_str("vm-suite", "--port", args, i));
            }
            other => {
                eprintln!("vm-suite: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    if nondet_test || std::env::var("REFWORK_NONDET_TEST").as_deref() == Ok("1") {
        nondet_test = true;
        eprintln!();
        eprintln!("WARNING: --nondet-test / REFWORK_NONDET_TEST is active.");
        eprintln!("         This is a TEST-ONLY mode that deliberately perturbs run 2.");
        eprintln!("         The result MUST NOT be used for acceptance or CI stamps.");
        eprintln!();
    }

    let opts = VmSuiteOptions {
        worker: worker.unwrap_or_else(|| missing_required_str("vm-suite", "--worker")),
        snapshot_ref: snapshot_ref
            .unwrap_or_else(|| missing_required_str("vm-suite", "--snapshot-ref")),
        script: script.unwrap_or_else(|| missing_required("vm-suite", "--script")),
        ram_region,
        ram_size,
        frames,
        snapshot_at,
        iterations,
        report: report.unwrap_or_else(|| missing_required("vm-suite", "--report")),
        nondet_test,
        port,
    };

    let report_data = match vm_suite(&opts) {
        Ok(report_data) => report_data,
        Err(e) => {
            eprintln!("vm-suite: {e}");
            process::exit(2);
        }
    };
    let report_hash = match write_suite_report(&report_data, &opts.report) {
        Ok(hash) => hash,
        Err(e) => {
            eprintln!("vm-suite: {e}");
            process::exit(2);
        }
    };

    if report_data.passed() {
        println!(
            "vm-suite: PASS — iterations={} frames={} zero-flake report_blake3={}",
            report_data.iterations.len(),
            report_data.frames,
            report_hash
        );
    } else {
        eprintln!(
            "vm-suite: FAIL — report {} (blake3 {})",
            opts.report.display(),
            report_hash
        );
        for iteration in &report_data.iterations {
            if !iteration.double_run.ok {
                eprintln!(
                    "  - iteration {}: double-run diverged at frame {:?}",
                    iteration.iteration, iteration.double_run.first_divergent_frame
                );
            }
            if !iteration.restore_continuity.ok {
                eprintln!(
                    "  - iteration {}: restore-continuity diverged at frame {:?}",
                    iteration.iteration, iteration.restore_continuity.first_divergent_frame
                );
            }
        }
        for failure in &report_data.failures {
            eprintln!(
                "  - iteration {} [{}] {}: {}",
                failure.iteration, failure.stage, failure.code, failure.message
            );
        }
        process::exit(1);
    }
}

// ─── vm-first-room ───────────────────────────────────────────────────────────

fn cmd_vm_first_room(args: &[String]) {
    let mut worker: Option<String> = None;
    let mut snapshot_ref: Option<String> = None;
    let mut script: Option<PathBuf> = None;
    let mut map: Option<PathBuf> = None;
    let mut expect: Option<PathBuf> = None;
    let mut image_manifest: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;
    let mut frames: Option<u32> = None;
    let mut port: u32 = 0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--worker" => {
                i += 1;
                worker = Some(require_arg_str("vm-first-room", "--worker", args, i).to_owned());
            }
            "--snapshot-ref" => {
                i += 1;
                snapshot_ref =
                    Some(require_arg_str("vm-first-room", "--snapshot-ref", args, i).to_owned());
            }
            "--script" => {
                i += 1;
                script = Some(require_arg("vm-first-room", "--script", args, i));
            }
            "--map" => {
                i += 1;
                map = Some(require_arg("vm-first-room", "--map", args, i));
            }
            "--expect" => {
                i += 1;
                expect = Some(require_arg("vm-first-room", "--expect", args, i));
            }
            "--image-manifest" => {
                i += 1;
                image_manifest = Some(require_arg("vm-first-room", "--image-manifest", args, i));
            }
            "--report" => {
                i += 1;
                report = Some(require_arg("vm-first-room", "--report", args, i));
            }
            "--frames" => {
                i += 1;
                let n = require_arg_str("vm-first-room", "--frames", args, i);
                frames = Some(n.parse().unwrap_or_else(|_| {
                    eprintln!("vm-first-room: --frames requires a positive integer");
                    process::exit(1);
                }));
            }
            "--port" => {
                i += 1;
                let n = require_arg_str("vm-first-room", "--port", args, i);
                port = n.parse().unwrap_or_else(|_| {
                    eprintln!("vm-first-room: --port requires an integer 0..=3");
                    process::exit(1);
                });
            }
            other => {
                eprintln!("vm-first-room: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = VmFirstRoomOptions {
        worker: worker.unwrap_or_else(|| missing_required_str("vm-first-room", "--worker")),
        snapshot_ref: snapshot_ref
            .unwrap_or_else(|| missing_required_str("vm-first-room", "--snapshot-ref")),
        script: script.unwrap_or_else(|| missing_required("vm-first-room", "--script")),
        map: map.unwrap_or_else(|| missing_required("vm-first-room", "--map")),
        expect: expect.unwrap_or_else(|| missing_required("vm-first-room", "--expect")),
        image_manifest,
        report: report.unwrap_or_else(|| missing_required("vm-first-room", "--report")),
        frames,
        port,
    };

    let report_data = match vm_first_room(&opts) {
        Ok(report_data) => report_data,
        Err(e) => {
            eprintln!("vm-first-room: {e}");
            process::exit(2);
        }
    };
    let report_hash = match write_report(&report_data, &opts.report) {
        Ok(hash) => hash,
        Err(e) => {
            eprintln!("vm-first-room: {e}");
            process::exit(2);
        }
    };

    if report_data.passed() {
        println!(
            "vm-first-room: PASS — frames_run={} transition_by={} report_blake3={}",
            report_data.frames_run,
            report_data
                .room_transition
                .as_ref()
                .map(|t| t.observed_by_frame.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            report_hash
        );
    } else {
        eprintln!(
            "vm-first-room: FAIL — {} failure(s), report {} (blake3 {})",
            report_data.failures.len(),
            opts.report.display(),
            report_hash
        );
        for failure in &report_data.failures {
            eprintln!(
                "  - [{}] {}: {}",
                failure.stage, failure.code, failure.message
            );
        }
        process::exit(1);
    }
}

// ─── trace ───────────────────────────────────────────────────────────────────

fn cmd_trace(args: &[String]) {
    let mut captures: Option<PathBuf> = None;
    let mut map: Option<PathBuf> = None;
    let mut scoring: Option<PathBuf> = None;
    let mut labels: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--captures" => {
                i += 1;
                captures = Some(require_arg("trace", "--captures", args, i));
            }
            "--map" => {
                i += 1;
                map = Some(require_arg("trace", "--map", args, i));
            }
            "--scoring" => {
                i += 1;
                scoring = Some(require_arg("trace", "--scoring", args, i));
            }
            "--labels" => {
                i += 1;
                labels = Some(require_arg("trace", "--labels", args, i));
            }
            "--out" => {
                i += 1;
                out = Some(require_arg("trace", "--out", args, i));
            }
            "--report" => {
                i += 1;
                report = Some(require_arg("trace", "--report", args, i));
            }
            other => {
                eprintln!("trace: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = TraceOptions {
        captures: captures.unwrap_or_else(|| missing_required("trace", "--captures")),
        map: map.unwrap_or_else(|| missing_required("trace", "--map")),
        scoring: scoring.unwrap_or_else(|| missing_required("trace", "--scoring")),
        labels: labels.unwrap_or_else(|| missing_required("trace", "--labels")),
        out: out.unwrap_or_else(|| missing_required("trace", "--out")),
        report: report.unwrap_or_else(|| missing_required("trace", "--report")),
    };

    let report = emit_phase4_trace(&opts);
    if report.passed() {
        println!(
            "trace: PASS — captures={} out={}",
            report.capture_count,
            opts.out.display()
        );
    } else {
        eprintln!("trace: FAIL — {} issue(s)", report.errors.len());
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
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

// ─── phase4-bundle-check ─────────────────────────────────────────────────────

fn cmd_phase4_bundle_check(args: &[String]) {
    let mut bundle_path: Option<PathBuf> = None;
    let mut report_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle_path = Some(require_arg("phase4-bundle-check", "--bundle", args, i));
            }
            "--report" => {
                i += 1;
                report_path =
                    Some(require_arg_str("phase4-bundle-check", "--report", args, i).to_string());
            }
            other => {
                eprintln!("phase4-bundle-check: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let bundle_path = bundle_path.unwrap_or_else(|| {
        eprintln!("phase4-bundle-check: --bundle is required");
        process::exit(1);
    });

    let report = check_phase4_bundle(&bundle_path);
    if let Some(path) = &report_path {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            eprintln!("phase4-bundle-check: report serialization failed: {}", e);
            process::exit(1);
        });
        if let Err(e) = std::fs::write(path, json) {
            eprintln!(
                "phase4-bundle-check: cannot write report to {}: {}",
                path, e
            );
            process::exit(1);
        }
    }

    if report.passed() {
        println!(
            "phase4-bundle-check: PASS — captures={} trajectories={}",
            report.capture_count,
            report.trajectory_files.len()
        );
    } else {
        eprintln!(
            "phase4-bundle-check: FAIL — {} issue(s)",
            report.errors.len()
        );
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

// ─── phase4-checksum-manifest ────────────────────────────────────────────────

fn cmd_phase4_checksum_manifest(args: &[String]) {
    let mut bundle_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut verify_path: Option<PathBuf> = None;
    let mut set_payload_root = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle_path = Some(require_arg("phase4-checksum-manifest", "--bundle", args, i));
            }
            "--out" => {
                i += 1;
                out_path = Some(require_arg("phase4-checksum-manifest", "--out", args, i));
            }
            "--verify" => {
                i += 1;
                verify_path = Some(require_arg("phase4-checksum-manifest", "--verify", args, i));
            }
            "--set-payload-root" => set_payload_root = true,
            other => {
                eprintln!("phase4-checksum-manifest: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let bundle =
        bundle_path.unwrap_or_else(|| missing_required("phase4-checksum-manifest", "--bundle"));
    if usize::from(out_path.is_some())
        + usize::from(verify_path.is_some())
        + usize::from(set_payload_root)
        != 1
    {
        eprintln!("phase4-checksum-manifest: exactly one of --out, --verify, or --set-payload-root is required");
        process::exit(1);
    }
    let (report, output) = if set_payload_root {
        (set_phase4_payload_root(&bundle), None)
    } else if let Some(manifest) = verify_path {
        (verify_phase4_checksum_manifest(&bundle, &manifest), None)
    } else {
        let out = out_path.unwrap();
        let opts = ChecksumManifestOptions {
            bundle,
            out: out.clone(),
        };
        (write_phase4_checksum_manifest(&opts), Some(out))
    };
    if report.passed() {
        println!(
            "phase4-checksum-manifest: PASS — files={} bytes={}{}",
            report.file_count,
            report.total_bytes,
            output
                .as_ref()
                .map(|p| format!(" out={}", p.display()))
                .unwrap_or_default()
        );
    } else {
        eprintln!(
            "phase4-checksum-manifest: FAIL — {} issue(s)",
            report.errors.len()
        );
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

// ─── phase4-context-check ────────────────────────────────────────────────────

fn cmd_phase4_context_check(args: &[String]) {
    let mut bundle_path: Option<PathBuf> = None;
    let mut report_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle_path = Some(require_arg("phase4-context-check", "--bundle", args, i));
            }
            "--report" => {
                i += 1;
                report_path =
                    Some(require_arg_str("phase4-context-check", "--report", args, i).to_string());
            }
            other => {
                eprintln!("phase4-context-check: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let bundle_path = bundle_path.unwrap_or_else(|| {
        eprintln!("phase4-context-check: --bundle is required");
        process::exit(1);
    });

    let report = check_phase4_context_bundle(&bundle_path);
    if let Some(path) = &report_path {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| {
            eprintln!("phase4-context-check: report serialization failed: {}", e);
            process::exit(1);
        });
        if let Err(e) = std::fs::write(path, json) {
            eprintln!(
                "phase4-context-check: cannot write report to {}: {}",
                path, e
            );
            process::exit(1);
        }
    }

    if report.passed() {
        println!(
            "phase4-context-check: PASS — contexts={} evidence_type={}",
            report.context_count,
            report.evidence_type.as_deref().unwrap_or("unknown")
        );
    } else {
        eprintln!(
            "phase4-context-check: FAIL — {} issue(s)",
            report.errors.len()
        );
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

fn cmd_phase4_context_export(args: &[String]) {
    let mut values = std::collections::BTreeMap::new();
    let mut ids = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].clone();
        if ![
            "--corpus",
            "--out",
            "--capture-id",
            "--context-artifact-id",
            "--access-requirement",
            "--retention",
            "--pad-table-hash",
            "--recent-input",
            "--evidence-type",
        ]
        .contains(&key.as_str())
        {
            eprintln!("phase4-context-export: unknown option '{key}'");
            process::exit(1);
        }
        i += 1;
        let value = require_arg_str("phase4-context-export", &key, args, i).to_owned();
        i += 1;
        if key == "--capture-id" {
            ids.push(value);
        } else {
            values.insert(key, value);
        }
    }
    let take = |name: &str| {
        values.get(name).cloned().unwrap_or_else(|| {
            eprintln!("phase4-context-export: {name} is required");
            process::exit(1)
        })
    };
    let opts = ContextExportOptions {
        corpus: take("--corpus").into(),
        out: take("--out").into(),
        capture_ids: ids,
        context_artifact_id: take("--context-artifact-id"),
        access_requirement: take("--access-requirement"),
        retention: take("--retention"),
        pad_table_hash: take("--pad-table-hash"),
        recent_input: values.get("--recent-input").map(PathBuf::from),
        evidence_type: values
            .get("--evidence-type")
            .cloned()
            .unwrap_or_else(|| "live".into()),
    };
    let report = export_phase4_context(&opts);
    if report.passed() {
        println!(
            "phase4-context-export: PASS — contexts={}",
            report.capture_count
        )
    } else {
        eprintln!(
            "phase4-context-export: FAIL — {} issue(s)",
            report.errors.len()
        );
        for e in report.errors {
            eprintln!("  - {e}")
        }
        process::exit(1)
    }
}

fn cmd_phase4_fallback_check(args: &[String]) {
    let mut bundle = None;
    let mut report_path = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bundle" => {
                i += 1;
                bundle = Some(require_arg("phase4-fallback-check", "--bundle", args, i))
            }
            "--report" => {
                i += 1;
                report_path = Some(require_arg("phase4-fallback-check", "--report", args, i))
            }
            other => {
                eprintln!("phase4-fallback-check: unknown option '{other}'");
                process::exit(1)
            }
        }
        i += 1
    }
    let bundle = bundle.unwrap_or_else(|| missing_required("phase4-fallback-check", "--bundle"));
    let report = check_phase4_fallback(&bundle);
    if let Some(path) = report_path {
        if let Err(e) = std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()) {
            eprintln!("phase4-fallback-check: cannot write report: {e}");
            process::exit(1)
        }
    }
    if report.passed() {
        println!(
            "phase4-fallback-check: PASS — captures={}",
            report.capture_count
        )
    } else {
        eprintln!(
            "phase4-fallback-check: FAIL — {} issue(s)",
            report.errors.len()
        );
        for e in report.errors {
            eprintln!("  - {e}")
        }
        process::exit(1)
    }
}

// ─── phase4-layout ───────────────────────────────────────────────────────────

fn cmd_phase4_layout(args: &[String]) {
    let mut map: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut capture_spec_hash: Option<String> = None;
    let mut layout_version = 1u64;
    let mut compiler_or_exporter_commit: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" => {
                i += 1;
                map = Some(require_arg("phase4-layout", "--map", args, i));
            }
            "--out" => {
                i += 1;
                out = Some(require_arg("phase4-layout", "--out", args, i));
            }
            "--capture-spec-hash" => {
                i += 1;
                capture_spec_hash = Some(
                    require_arg_str("phase4-layout", "--capture-spec-hash", args, i).to_owned(),
                );
            }
            "--layout-version" => {
                i += 1;
                layout_version = require_arg_str("phase4-layout", "--layout-version", args, i)
                    .parse()
                    .unwrap_or_else(|_| {
                        eprintln!("phase4-layout: --layout-version must be an unsigned integer");
                        process::exit(1);
                    });
            }
            "--compiler-or-exporter-commit" => {
                i += 1;
                compiler_or_exporter_commit = Some(
                    require_arg_str("phase4-layout", "--compiler-or-exporter-commit", args, i)
                        .to_owned(),
                );
            }
            other => {
                eprintln!("phase4-layout: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = LayoutOptions {
        map: map.unwrap_or_else(|| missing_required("phase4-layout", "--map")),
        out: out.unwrap_or_else(|| missing_required("phase4-layout", "--out")),
        capture_spec_hash: capture_spec_hash
            .unwrap_or_else(|| missing_required_str("phase4-layout", "--capture-spec-hash")),
        layout_version,
        compiler_or_exporter_commit: compiler_or_exporter_commit
            .unwrap_or_else(default_compiler_or_exporter_commit),
    };
    let report = write_phase4_layout(&opts);
    if report.passed() {
        println!(
            "phase4-layout: PASS — ranges={} total_len={}",
            report.range_count, report.total_len
        );
    } else {
        eprintln!("phase4-layout: FAIL — {} issue(s)", report.errors.len());
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

// ─── phase4-private-intake ───────────────────────────────────────────────────

fn cmd_phase4_private_intake(args: &[String]) {
    let mut rom_dir: Option<PathBuf> = None;
    let mut private_root: Option<PathBuf> = None;
    let mut operator_approved = false;
    let mut operator_metadata_policy =
        "operator ROM metadata available only inside private bundle".to_owned();
    let mut operator_label: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom-dir" => {
                i += 1;
                rom_dir = Some(require_arg("phase4-private-intake", "--rom-dir", args, i));
            }
            "--private-root" => {
                i += 1;
                private_root = Some(require_arg(
                    "phase4-private-intake",
                    "--private-root",
                    args,
                    i,
                ));
            }
            "--operator-approved" => {
                operator_approved = true;
            }
            "--operator-metadata-policy" => {
                i += 1;
                operator_metadata_policy = require_arg_str(
                    "phase4-private-intake",
                    "--operator-metadata-policy",
                    args,
                    i,
                )
                .to_owned();
            }
            "--operator-label" => {
                i += 1;
                operator_label = Some(
                    require_arg_str("phase4-private-intake", "--operator-label", args, i)
                        .to_owned(),
                );
            }
            other => {
                eprintln!("phase4-private-intake: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = PrivateIntakeOptions {
        rom_dir: rom_dir.unwrap_or_else(default_rom_dir),
        private_root: private_root
            .unwrap_or_else(|| missing_required("phase4-private-intake", "--private-root")),
        operator_approved,
        operator_metadata_policy,
        operator_label,
    };
    let report = prepare_phase4_private_intake(&opts);
    if report.passed() {
        println!(
            "phase4-private-intake: PASS — rom_files={} private_root_shape=prepared",
            report.rom_regular_file_count
        );
    } else {
        eprintln!(
            "phase4-private-intake: FAIL — {} issue(s)",
            report.errors.len()
        );
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

// ─── phase4-score-plan ───────────────────────────────────────────────────────

fn cmd_phase4_score_plan(args: &[String]) {
    let mut captures: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut client_batch_prefix = "phase4-k32".to_owned();
    let mut first_boss = Vec::new();
    let mut goal_positive = Vec::new();
    let mut goal_negative = Vec::new();
    let mut checkpoint_after_batch: Option<String> = None;
    let mut restore_control_batch_ids = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--captures" => {
                i += 1;
                captures = Some(require_arg("phase4-score-plan", "--captures", args, i));
            }
            "--out" => {
                i += 1;
                out = Some(require_arg("phase4-score-plan", "--out", args, i));
            }
            "--client-batch-prefix" => {
                i += 1;
                client_batch_prefix =
                    require_arg_str("phase4-score-plan", "--client-batch-prefix", args, i)
                        .to_owned();
            }
            "--first-boss" => {
                i += 1;
                first_boss
                    .push(require_arg_str("phase4-score-plan", "--first-boss", args, i).to_owned());
            }
            "--goal-positive" => {
                i += 1;
                goal_positive.push(
                    require_arg_str("phase4-score-plan", "--goal-positive", args, i).to_owned(),
                );
            }
            "--goal-negative" => {
                i += 1;
                goal_negative.push(
                    require_arg_str("phase4-score-plan", "--goal-negative", args, i).to_owned(),
                );
            }
            "--checkpoint-after-batch" => {
                i += 1;
                checkpoint_after_batch = Some(
                    require_arg_str("phase4-score-plan", "--checkpoint-after-batch", args, i)
                        .to_owned(),
                );
            }
            "--restore-control-batch" => {
                i += 1;
                restore_control_batch_ids.push(
                    require_arg_str("phase4-score-plan", "--restore-control-batch", args, i)
                        .to_owned(),
                );
            }
            other => {
                eprintln!("phase4-score-plan: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = ScorePlanOptions {
        captures: captures.unwrap_or_else(|| missing_required("phase4-score-plan", "--captures")),
        out: out.unwrap_or_else(|| missing_required("phase4-score-plan", "--out")),
        client_batch_prefix,
        first_boss,
        goal_positive,
        goal_negative,
        checkpoint_after_batch,
        restore_control_batch_ids,
    };
    let report = write_phase4_score_plan(&opts);
    if report.passed() {
        println!(
            "phase4-score-plan: PASS — captures={} batches={} emitted={} out={}",
            report.capture_count,
            report.full_batch_count,
            report.emitted_capture_count,
            opts.out.display()
        );
    } else {
        eprintln!("phase4-score-plan: FAIL — {} issue(s)", report.errors.len());
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        process::exit(1);
    }
}

// ─── redaction-scan ──────────────────────────────────────────────────────────

fn cmd_redaction_scan(args: &[String]) {
    let mut input: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;
    let mut forbidden_literals = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--input" => {
                i += 1;
                input = Some(require_arg("redaction-scan", "--input", args, i));
            }
            "--report" => {
                i += 1;
                report = Some(require_arg("redaction-scan", "--report", args, i));
            }
            "--forbid" => {
                i += 1;
                let literal = require_arg_str("redaction-scan", "--forbid", args, i);
                if literal.is_empty() {
                    eprintln!("redaction-scan: --forbid literal must not be empty");
                    process::exit(1);
                }
                forbidden_literals.push(literal.to_owned());
            }
            "--forbid-file" => {
                i += 1;
                let path = require_arg("redaction-scan", "--forbid-file", args, i);
                match load_forbidden_literals(&path) {
                    Ok(mut literals) => forbidden_literals.append(&mut literals),
                    Err(err) => {
                        eprintln!("redaction-scan: {err}");
                        process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("redaction-scan: unknown option '{}'", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    let opts = RedactionScanOptions {
        input: input.unwrap_or_else(|| missing_required("redaction-scan", "--input")),
        report,
        forbidden_literals,
    };
    let report = scan_redactions(&opts);
    if report.passed() {
        println!(
            "redaction-scan: PASS — bytes={} lines={}",
            report.bytes, report.lines
        );
    } else {
        eprintln!(
            "redaction-scan: FAIL — findings={} errors={}",
            report.finding_count,
            report.errors.len()
        );
        for err in &report.errors {
            eprintln!("  - {err}");
        }
        for finding in &report.findings {
            eprintln!(
                "  - {} at line {}, column {}",
                finding.kind, finding.line, finding.column
            );
        }
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

fn missing_required(cmd: &str, flag: &str) -> PathBuf {
    eprintln!("{cmd}: {flag} is required");
    process::exit(1);
}

fn missing_required_str(cmd: &str, flag: &str) -> String {
    eprintln!("{cmd}: {flag} is required");
    process::exit(1);
}

fn default_rom_dir() -> PathBuf {
    let Some(home) = std::env::var_os("HOME") else {
        eprintln!("phase4-private-intake: --rom-dir is required when HOME is unset");
        process::exit(1);
    };
    PathBuf::from(home).join("ROMs/SNES")
}

fn default_compiler_or_exporter_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "refwork-verify:unknown".to_owned())
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
