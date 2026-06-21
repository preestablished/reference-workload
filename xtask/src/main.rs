//! xtask binary — hand-rolled argument parser (no clap).

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        usage();
        std::process::exit(1);
    }

    match args[0].as_str() {
        "audit-syms" => cmd_audit_syms(&args[1..]),
        "build-rom" => cmd_build_rom(&args[1..]),
        "deny" => cmd_deny(&args[1..]),
        "fetch-test-roms" => cmd_fetch_test_roms(&args[1..]),
        "cpu-tests" => cmd_cpu_tests(&args[1..]),
        "spc-tests" => cmd_spc_tests(&args[1..]),
        "hash-chain" => cmd_hash_chain(&args[1..]),
        "image" => cmd_image(&args[1..]),
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
    println!("  audit-syms --bin PATH");
    println!("      Audit a release binary for banned clock, sleep, and scheduler symbols.");
    println!();
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
    println!();
    println!("  spc-tests [--dir DIR] [--filter SUBSTR] [--max-fail N]");
    println!("      Run the SPC700 single-step JSON corpus against the audio CPU.");
    println!();
    println!("  hash-chain [--frames N]");
    println!("      Print the chained synthetic-ROM frame hash (default 600 frames).");
    println!("      Identical across architectures = cross-arch determinism holds.");
    println!();
    println!("  image build --agent-bin PATH");
    println!("      Build dist/workload-image-<version>/ image handoff artifacts.");
    println!();
    println!("  image validate PATH");
    println!("      Validate a workload-image.yaml and its adjacent artifacts.");
}

// ─── audit-syms ──────────────────────────────────────────────────────────────

fn cmd_audit_syms(args: &[String]) {
    let mut bin: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bin" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("audit-syms: --bin requires a path");
                    std::process::exit(2);
                }
                if bin.is_some() {
                    eprintln!("audit-syms: --bin may only be supplied once");
                    std::process::exit(2);
                }
                bin = Some(PathBuf::from(&args[i]));
            }
            other => {
                eprintln!("audit-syms: unknown option '{}'", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let Some(bin) = bin else {
        eprintln!("audit-syms: --bin is required");
        std::process::exit(2);
    };

    if let Err(err) = xtask::audit_syms::run_audit_syms(&bin) {
        eprintln!("audit-syms: {err}");
        std::process::exit(1);
    }
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

// ─── image ───────────────────────────────────────────────────────────────────

fn cmd_image(args: &[String]) {
    let Some(subcommand) = args.first() else {
        eprintln!("image: expected subcommand build or validate");
        std::process::exit(2);
    };
    match subcommand.as_str() {
        "build" => cmd_image_build(&args[1..]),
        "validate" => cmd_image_validate(&args[1..]),
        other => {
            eprintln!("image: unknown subcommand '{}'", other);
            std::process::exit(2);
        }
    }
}

fn cmd_image_build(args: &[String]) {
    let mut agent_bin: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--agent-bin" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("image build: --agent-bin requires a path");
                    std::process::exit(2);
                }
                agent_bin = Some(PathBuf::from(&args[i]));
            }
            other => {
                eprintln!("image build: unknown option '{}'", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let Some(agent_bin) = agent_bin else {
        eprintln!("image build: --agent-bin is required");
        std::process::exit(2);
    };

    let workspace_root = find_workspace_root();
    match xtask::image::build_image(&workspace_root, &agent_bin) {
        Ok(out_dir) => println!("image build: wrote {}", out_dir.display()),
        Err(err) => {
            eprintln!("image build: {err}");
            std::process::exit(1);
        }
    }
}

fn cmd_image_validate(args: &[String]) {
    if args.len() != 1 {
        eprintln!("image validate: expected exactly one manifest path");
        std::process::exit(2);
    }

    match xtask::image::validate_manifest(&PathBuf::from(&args[0])) {
        Ok(()) => println!("image validate: OK - {}", args[0]),
        Err(err) => {
            eprintln!("image validate: {err}");
            std::process::exit(1);
        }
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

// ─── spc-tests ───────────────────────────────────────────────────────────────

fn cmd_spc_tests(args: &[String]) {
    let mut opts = xtask::spc_tests::SpcTestOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("spc-tests: --dir requires a path");
                    std::process::exit(2);
                }
                opts.dir = std::path::PathBuf::from(&args[i]);
            }
            "--filter" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("spc-tests: --filter requires a substring");
                    std::process::exit(2);
                }
                opts.filter = Some(args[i].clone());
            }
            "--max-fail" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("spc-tests: --max-fail requires a number");
                    std::process::exit(2);
                }
                opts.max_fail = args[i].parse::<usize>().unwrap_or_else(|_| {
                    eprintln!("spc-tests: --max-fail must be a non-negative integer");
                    std::process::exit(2);
                });
            }
            other => {
                eprintln!("spc-tests: unknown option '{}'", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if xtask::spc_tests::run_spc_tests(&opts).is_err() {
        std::process::exit(1);
    }
}

// ─── hash-chain ──────────────────────────────────────────────────────────────

fn cmd_hash_chain(args: &[String]) {
    let mut frames = 600usize;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--frames" => {
                i += 1;
                let v = args.get(i).and_then(|a| a.parse().ok());
                match v {
                    Some(n) => frames = n,
                    None => {
                        eprintln!("hash-chain: --frames requires a positive integer");
                        std::process::exit(2);
                    }
                }
            }
            other => {
                eprintln!("hash-chain: unknown option '{}'", other);
                std::process::exit(2);
            }
        }
        i += 1;
    }
    match xtask::hash_chain::run_hash_chain(frames) {
        Ok(chain) => {
            let hex: String = chain.iter().map(|b| format!("{:02x}", b)).collect();
            println!(
                "hash-chain: frames={} arch={} chain={}",
                frames,
                std::env::consts::ARCH,
                hex
            );
        }
        Err(e) => {
            eprintln!("hash-chain: {}", e);
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
