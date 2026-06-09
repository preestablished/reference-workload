//! xtask binary — hand-rolled argument parser (no clap).

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        usage();
        std::process::exit(1);
    }

    match args[0].as_str() {
        "build-rom" => cmd_build_rom(&args[1..]),
        "deny" => cmd_deny(&args[1..]),
        "fetch-test-roms" => cmd_fetch_test_roms(&args[1..]),
        "cpu-tests" => cmd_cpu_tests(&args[1..]),
        "--help" | "-h" | "help" => {
            usage();
        }
        other => {
            eprintln!("xtask: unknown subcommand '{}'", other);
            usage();
            std::process::exit(1);
        }
    }
}

fn usage() {
    println!("Usage: cargo xtask <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("Subcommands:");
    println!("  build-rom [--out PATH]");
    println!("      Assemble the synthetic test ROM. Default output: target/synth-rom.rom");
    println!();
    println!("  deny");
    println!("      Scan crates/refwork-emu and crates/refwork-harness for banned");
    println!("      determinism-breaking tokens. Exits non-zero on any finding.");
    println!();
    println!("  fetch-test-roms");
    println!("      Download and verify test ROM archives from xtask/test-roms.lock.");
    println!();
    println!("  cpu-tests [--dir DIR] [--filter SUBSTR] [--max-fail N]");
    println!("      Run the 65816 single-step JSON test corpus against the emulator CPU.");
}

// ─── build-rom ───────────────────────────────────────────────────────────────

fn cmd_build_rom(args: &[String]) {
    let mut out_path = PathBuf::from("target/synth-rom.rom");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("build-rom: --out requires a path");
                    std::process::exit(1);
                }
                out_path = PathBuf::from(&args[i]);
            }
            other => {
                eprintln!("build-rom: unknown option '{}'", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    println!("build-rom: assembling synth ROM ...");
    let rom = xtask::build_synth_rom();
    println!("build-rom: {} bytes assembled", rom.len());

    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                eprintln!("build-rom: cannot create output directory: {}", e);
                std::process::exit(1);
            });
        }
    }

    std::fs::write(&out_path, &rom).unwrap_or_else(|e| {
        eprintln!("build-rom: cannot write '{}': {}", out_path.display(), e);
        std::process::exit(1);
    });

    let hash = blake3::hash(&rom);
    println!("build-rom: written to {}", out_path.display());
    println!("build-rom: blake3 = {}", hash.to_hex());
}

// ─── deny ────────────────────────────────────────────────────────────────────

fn cmd_deny(_args: &[String]) {
    let workspace_root = find_workspace_root();
    match xtask::deny::run_deny(&workspace_root) {
        Ok(()) => {}
        Err(_count) => std::process::exit(1),
    }
}

// ─── fetch-test-roms ─────────────────────────────────────────────────────────

fn cmd_fetch_test_roms(_args: &[String]) {
    let workspace_root = find_workspace_root();
    match xtask::fetch::run_fetch(&workspace_root) {
        Ok(()) => println!("fetch-test-roms: done."),
        Err(e) => {
            eprintln!("fetch-test-roms: {}", e);
            std::process::exit(1);
        }
    }
}

// ─── cpu-tests ───────────────────────────────────────────────────────────────

fn cmd_cpu_tests(args: &[String]) {
    let mut opts = xtask::cpu_tests::CpuTestOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("cpu-tests: --dir requires a path");
                    std::process::exit(1);
                }
                opts.dir = PathBuf::from(&args[i]);
            }
            "--filter" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("cpu-tests: --filter requires a substring");
                    std::process::exit(1);
                }
                opts.filter = Some(args[i].clone());
            }
            "--max-fail" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("cpu-tests: --max-fail requires a number");
                    std::process::exit(1);
                }
                opts.max_fail = args[i].parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("cpu-tests: --max-fail must be a non-negative integer");
                    std::process::exit(1);
                });
            }
            other => {
                eprintln!("cpu-tests: unknown option '{}'", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    match xtask::cpu_tests::run_cpu_tests(&opts) {
        Ok(()) => {}
        Err(_) => std::process::exit(1),
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Find the workspace root by walking up from the current directory looking
/// for a `Cargo.toml` containing `[workspace]`. Falls back to the current dir.
fn find_workspace_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = std::fs::read_to_string(&candidate) {
                if content.contains("[workspace]") {
                    return dir;
                }
            }
        }
        match dir.parent() {
            Some(p) => dir = p.to_owned(),
            None => break,
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}
