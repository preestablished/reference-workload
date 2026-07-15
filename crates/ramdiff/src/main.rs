//! `ramdiff` — RAM-address discovery tool.
//!
//! # Subcommands
//!
//! ```text
//! ramdiff record --rom <file.rom> --script <run.padlog> --session <dir>
//!                [--mark <frame>=<label>] [--dump-every N] [--frames N]
//!                [--interactive]   (only when compiled with --features interactive)
//!                [--resume] [--gamepad /dev/input/eventN]
//!
//! ramdiff search --session <dir>
//!                [--width u8|u16le]
//!                [--changed A B] [--unchanged A B]
//!                [--inc A B] [--dec A B]
//!                [--value N --in A] [--delta D A B]
//!
//! ramdiff candidates --session <dir> [--context N] [--limit N]
//!
//! ramdiff watch --addr <region>:<offset> --rom <file.rom> --script <run.padlog>
//!               [--width u8|u16le]
//!
//! ramdiff emit --map <feature-map.yaml> --name <name> --offset <hex-or-dec>
//!              --type <type> --stability stable|volatile
//!              [--region wram] [--semantics <s>] [--description <text>]
//!              [--discretize identity|none|bits]
//!              [--force]
//! ```
//!
//! # Interactive mode keyboard mapping (feature `interactive`)
//!
//! API.md §3.4 pad bitmask (bit 0..11): A B X Y L R Up Down Left Right Start Select
//!
//! | Key | Button | Bit |
//! |-----|--------|-----|
//! | X key | A button | 0 |
//! | Z key | B button | 1 |
//! | A key | X button | 2 |
//! | S key | Y button | 3 |
//! | Q key | L | 4 |
//! | W key | R | 5 |
//! | Up arrow | Up | 6 |
//! | Down arrow | Down | 7 |
//! | Left arrow | Left | 8 |
//! | Right arrow | Right | 9 |
//! | Enter | Start | 10 |
//! | Right Shift | Select | 11 |
//! | F5 | Dump WRAM (prompts for label) | — |
//! | Esc | Quit | — |
//!
//! A Logitech F310 (or compatible) gamepad is auto-detected on Linux and
//! merged with the keyboard. F5 and Esc remain keyboard-only.

#![forbid(unsafe_code)]

use std::str::FromStr;

use ramdiff::candidates::CandidatesOpts;
use ramdiff::emit::{parse_feature_type, parse_semantics, parse_stability, EmitOpts};
use ramdiff::filter::{run_search, FilterOp};
use ramdiff::record::{parse_mark, parse_watch_addr, InteractiveOpts, RecordOpts, WatchOpts};
use ramdiff::session::SearchWidth;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        usage();
        std::process::exit(1);
    }

    let result = match args[0].as_str() {
        "record" => cmd_record(&args[1..]),
        "search" => cmd_search(&args[1..]),
        "candidates" => cmd_candidates(&args[1..]),
        "watch" => cmd_watch(&args[1..]),
        "emit" => cmd_emit(&args[1..]),
        "--help" | "-h" | "help" => {
            usage();
            return;
        }
        other => {
            eprintln!("ramdiff: unknown subcommand {:?}", other);
            usage();
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("ramdiff: error: {}", e);
        std::process::exit(1);
    }
}

fn usage() {
    println!("Usage: ramdiff <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("Subcommands:");
    println!("  record --rom <file.rom> --script <run.padlog> --session <dir>");
    println!("         [--mark <frame>=<label>] [--dump-every N] [--frames N]");
    #[cfg(feature = "interactive")]
    println!("         [--interactive] [--resume] [--output-log <file.padlog>]");
    println!("         [--gamepad /dev/input/eventN]   (default: auto-detect)");
    println!();
    println!("  search --session <dir>");
    println!("         [--width u8|u16le]");
    println!("         [--changed A B] [--unchanged A B]");
    println!("         [--inc A B] [--dec A B]");
    println!("         [--value N --in A] [--delta D A B]");
    println!();
    println!("  candidates --session <dir> [--context N] [--limit N]");
    println!();
    println!("  watch --addr <region>:<offset> --rom <file.rom> --script <run.padlog>");
    println!("        [--width u8|u16le]");
    println!();
    println!("  emit --map <feature-map.yaml> --name <name> --offset <hex-or-dec>");
    println!("       --type <type> --stability stable|volatile");
    println!("       [--region wram] [--semantics <s>] [--description <text>]");
    println!("       [--discretize identity|none|bits] [--force]");
}

// ─── record ──────────────────────────────────────────────────────────────────

fn cmd_record(args: &[String]) -> Result<(), String> {
    let mut rom: Option<std::path::PathBuf> = None;
    let mut script: Option<std::path::PathBuf> = None;
    let mut session_dir: Option<std::path::PathBuf> = None;
    let mut marks: Vec<(u64, String)> = Vec::new();
    let mut dump_every: Option<u64> = None;
    let mut total_frames: Option<u64> = None;
    let mut interactive = false;
    let mut resume = false;
    let mut output_log: Option<std::path::PathBuf> = None;
    let mut gamepad: Option<std::path::PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom = Some(need_path("record", "--rom", args, i)?);
            }
            "--script" => {
                i += 1;
                script = Some(need_path("record", "--script", args, i)?);
            }
            "--session" => {
                i += 1;
                session_dir = Some(need_path("record", "--session", args, i)?);
            }
            "--mark" => {
                i += 1;
                let s = need_str("record", "--mark", args, i)?;
                marks.push(parse_mark(&s)?);
            }
            "--dump-every" => {
                i += 1;
                let s = need_str("record", "--dump-every", args, i)?;
                dump_every = Some(
                    s.parse::<u64>()
                        .map_err(|_| format!("--dump-every: expected integer, got {:?}", s))?,
                );
            }
            "--frames" => {
                i += 1;
                let s = need_str("record", "--frames", args, i)?;
                total_frames = Some(
                    s.parse::<u64>()
                        .map_err(|_| format!("--frames: expected integer, got {:?}", s))?,
                );
            }
            "--interactive" => {
                interactive = true;
            }
            "--resume" => {
                resume = true;
            }
            "--output-log" => {
                i += 1;
                output_log = Some(need_path("record", "--output-log", args, i)?);
            }
            "--gamepad" => {
                i += 1;
                gamepad = Some(need_path("record", "--gamepad", args, i)?);
            }
            other => {
                return Err(format!("record: unknown option {:?}", other));
            }
        }
        i += 1;
    }

    let session_dir = session_dir.ok_or_else(|| "record: --session is required".to_owned())?;

    if interactive {
        let rom = rom.ok_or_else(|| "record --interactive: --rom is required".to_owned())?;
        let out_log = output_log.unwrap_or_else(|| session_dir.join("interactive.padlog"));
        return ramdiff::record::run_interactive(&InteractiveOpts {
            rom,
            session_dir,
            output_log: out_log,
            resume,
            gamepad,
        });
    }

    if resume {
        return Err("record: --resume requires --interactive".to_owned());
    }
    if gamepad.is_some() {
        return Err("record: --gamepad requires --interactive".to_owned());
    }

    let rom = rom.ok_or_else(|| "record: --rom is required".to_owned())?;
    let script = script.ok_or_else(|| "record: --script is required".to_owned())?;

    ramdiff::record::run_record(&RecordOpts {
        rom,
        script,
        session_dir,
        marks,
        dump_every,
        total_frames,
        quiet: false,
    })
}

// ─── search ──────────────────────────────────────────────────────────────────

fn cmd_search(args: &[String]) -> Result<(), String> {
    let mut session_dir: Option<std::path::PathBuf> = None;
    let mut ops: Vec<FilterOp> = Vec::new();
    let mut pending_value: Option<u32> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--session" => {
                i += 1;
                session_dir = Some(need_path("search", "--session", args, i)?);
            }
            "--width" => {
                i += 1;
                let s = need_str("search", "--width", args, i)?;
                ops.push(FilterOp::SetWidth(SearchWidth::from_str(&s)?));
            }
            "--changed" => {
                let a = positional("search", "--changed A", args, i + 1)?;
                let b = positional("search", "--changed B", args, i + 2)?;
                ops.push(FilterOp::Changed { a, b });
                i += 2;
            }
            "--unchanged" => {
                let a = positional("search", "--unchanged A", args, i + 1)?;
                let b = positional("search", "--unchanged B", args, i + 2)?;
                ops.push(FilterOp::Unchanged { a, b });
                i += 2;
            }
            "--inc" => {
                let a = positional("search", "--inc A", args, i + 1)?;
                let b = positional("search", "--inc B", args, i + 2)?;
                ops.push(FilterOp::Increased { a, b });
                i += 2;
            }
            "--dec" => {
                let a = positional("search", "--dec A", args, i + 1)?;
                let b = positional("search", "--dec B", args, i + 2)?;
                ops.push(FilterOp::Decreased { a, b });
                i += 2;
            }
            "--value" => {
                i += 1;
                let s = need_str("search", "--value", args, i)?;
                pending_value = Some(parse_u32(&s, "--value")?);
            }
            "--in" => {
                i += 1;
                let label = need_str("search", "--in", args, i)?;
                let value = pending_value
                    .take()
                    .ok_or_else(|| "search: --in must follow --value".to_owned())?;
                ops.push(FilterOp::ValueIn { value, label });
            }
            "--delta" => {
                let d_s = positional("search", "--delta D", args, i + 1)?;
                let a = positional("search", "--delta A", args, i + 2)?;
                let b = positional("search", "--delta B", args, i + 3)?;
                let d = parse_u32(&d_s, "--delta")?;
                ops.push(FilterOp::Delta { delta: d, a, b });
                i += 3;
            }
            other => {
                return Err(format!("search: unknown option {:?}", other));
            }
        }
        i += 1;
    }

    if let Some(v) = pending_value {
        return Err(format!(
            "search: --value {} has no following --in <label>",
            v
        ));
    }

    let dir = session_dir.ok_or_else(|| "search: --session is required".to_owned())?;
    run_search(&dir, &ops)
}

// ─── candidates ──────────────────────────────────────────────────────────────

fn cmd_candidates(args: &[String]) -> Result<(), String> {
    let mut session_dir: Option<std::path::PathBuf> = None;
    let mut opts = CandidatesOpts::default();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--session" => {
                i += 1;
                session_dir = Some(need_path("candidates", "--session", args, i)?);
            }
            "--context" => {
                i += 1;
                let s = need_str("candidates", "--context", args, i)?;
                opts.context = s
                    .parse::<usize>()
                    .map_err(|_| format!("--context: expected integer, got {:?}", s))?;
            }
            "--limit" => {
                i += 1;
                let s = need_str("candidates", "--limit", args, i)?;
                opts.limit = Some(
                    s.parse::<usize>()
                        .map_err(|_| format!("--limit: expected integer, got {:?}", s))?,
                );
            }
            other => {
                return Err(format!("candidates: unknown option {:?}", other));
            }
        }
        i += 1;
    }

    let dir = session_dir.ok_or_else(|| "candidates: --session is required".to_owned())?;
    ramdiff::candidates::run_candidates(&dir, &opts)
}

// ─── watch ───────────────────────────────────────────────────────────────────

fn cmd_watch(args: &[String]) -> Result<(), String> {
    let mut rom: Option<std::path::PathBuf> = None;
    let mut script: Option<std::path::PathBuf> = None;
    let mut addr_str: Option<String> = None;
    let mut width = SearchWidth::U8;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rom" => {
                i += 1;
                rom = Some(need_path("watch", "--rom", args, i)?);
            }
            "--script" => {
                i += 1;
                script = Some(need_path("watch", "--script", args, i)?);
            }
            "--addr" => {
                i += 1;
                addr_str = Some(need_str("watch", "--addr", args, i)?);
            }
            "--width" => {
                i += 1;
                let s = need_str("watch", "--width", args, i)?;
                width = SearchWidth::from_str(&s)?;
            }
            other => {
                return Err(format!("watch: unknown option {:?}", other));
            }
        }
        i += 1;
    }

    let rom = rom.ok_or_else(|| "watch: --rom is required".to_owned())?;
    let script = script.ok_or_else(|| "watch: --script is required".to_owned())?;
    let addr_s = addr_str.ok_or_else(|| "watch: --addr is required".to_owned())?;
    let addr = parse_watch_addr(&addr_s)?;

    ramdiff::record::run_watch(&WatchOpts {
        rom,
        script,
        addr,
        width,
    })
}

// ─── emit ─────────────────────────────────────────────────────────────────────

fn cmd_emit(args: &[String]) -> Result<(), String> {
    let mut map: Option<std::path::PathBuf> = None;
    let mut name: Option<String> = None;
    let mut offset: Option<u32> = None;
    let mut feature_type: Option<refwork_featuremap::FeatureType> = None;
    let mut stability: Option<refwork_featuremap::Stability> = None;
    let mut discretize: Option<refwork_featuremap::Discretize> = None;
    let mut region = "wram".to_owned();
    let mut semantics = refwork_featuremap::Semantics::Opaque;
    let mut description: Option<String> = None;
    let mut force = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" => {
                i += 1;
                map = Some(need_path("emit", "--map", args, i)?);
            }
            "--name" => {
                i += 1;
                name = Some(need_str("emit", "--name", args, i)?);
            }
            "--offset" => {
                i += 1;
                let s = need_str("emit", "--offset", args, i)?;
                offset = Some(parse_u32_or_hex(&s, "--offset")?);
            }
            "--type" => {
                i += 1;
                let s = need_str("emit", "--type", args, i)?;
                feature_type = Some(parse_feature_type(&s)?);
            }
            "--stability" => {
                i += 1;
                let s = need_str("emit", "--stability", args, i)?;
                stability = Some(parse_stability(&s)?);
            }
            "--discretize" => {
                i += 1;
                let s = need_str("emit", "--discretize", args, i)?;
                discretize = Some(parse_discretize_str(&s)?);
            }
            "--region" => {
                i += 1;
                region = need_str("emit", "--region", args, i)?;
            }
            "--semantics" => {
                i += 1;
                let s = need_str("emit", "--semantics", args, i)?;
                semantics = parse_semantics(&s)?;
            }
            "--description" => {
                i += 1;
                description = Some(need_str("emit", "--description", args, i)?);
            }
            "--force" => {
                force = true;
            }
            other => {
                return Err(format!("emit: unknown option {:?}", other));
            }
        }
        i += 1;
    }

    let map = map.ok_or_else(|| "emit: --map is required".to_owned())?;
    let name = name.ok_or_else(|| "emit: --name is required".to_owned())?;
    let offset = offset.ok_or_else(|| "emit: --offset is required".to_owned())?;
    let feature_type = feature_type.ok_or_else(|| "emit: --type is required".to_owned())?;
    let stability = stability.ok_or_else(|| "emit: --stability is required".to_owned())?;

    ramdiff::emit::run_emit(&EmitOpts {
        map,
        name,
        offset,
        feature_type,
        stability,
        discretize,
        region,
        semantics,
        description,
        force,
    })
}

// ─── Argument helpers ─────────────────────────────────────────────────────────

fn need_path(
    cmd: &str,
    flag: &str,
    args: &[String],
    idx: usize,
) -> Result<std::path::PathBuf, String> {
    if idx >= args.len() {
        return Err(format!("{}: {} requires an argument", cmd, flag));
    }
    Ok(std::path::PathBuf::from(&args[idx]))
}

fn need_str(cmd: &str, flag: &str, args: &[String], idx: usize) -> Result<String, String> {
    if idx >= args.len() {
        return Err(format!("{}: {} requires an argument", cmd, flag));
    }
    Ok(args[idx].clone())
}

fn positional(cmd: &str, name: &str, args: &[String], idx: usize) -> Result<String, String> {
    if idx >= args.len() {
        return Err(format!("{}: {} requires an argument", cmd, name));
    }
    if args[idx].starts_with("--") {
        return Err(format!(
            "{}: {} requires an argument but got flag {:?}",
            cmd, name, args[idx]
        ));
    }
    Ok(args[idx].clone())
}

fn parse_u32(s: &str, flag: &str) -> Result<u32, String> {
    if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16).map_err(|_| format!("{}: bad hex value {:?}", flag, s))
    } else {
        s.parse::<u32>()
            .map_err(|_| format!("{}: expected integer, got {:?}", flag, s))
    }
}

fn parse_u32_or_hex(s: &str, flag: &str) -> Result<u32, String> {
    parse_u32(s, flag)
}

fn parse_discretize_str(s: &str) -> Result<refwork_featuremap::Discretize, String> {
    match s {
        "identity" => Ok(refwork_featuremap::Discretize::Identity),
        "none" => Ok(refwork_featuremap::Discretize::None),
        "bits" => Ok(refwork_featuremap::Discretize::Bits),
        other => Err(format!(
            "unknown discretize {:?}, expected identity|none|bits",
            other
        )),
    }
}
