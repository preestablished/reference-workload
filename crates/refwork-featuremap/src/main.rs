//! `refwork-featuremap` CLI — validate and schema subcommands.
//!
//! Usage:
//!   refwork-featuremap validate <map.yaml> [--scoring <scoring.yaml>]
//!   refwork-featuremap schema
//!
//! Arg parsing is hand-rolled to match the xtask style (no clap dependency).

#![forbid(unsafe_code)]

use refwork_featuremap::{
    generate_schema, parse_feature_map, parse_scoring_program, validate_pair,
};
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "validate" => cmd_validate(&args[2..]),
        "schema" => cmd_schema(),
        other => {
            eprintln!("error: unknown subcommand {:?}", other);
            usage();
            process::exit(1);
        }
    }
}

fn usage() {
    eprintln!("Usage:");
    eprintln!("  refwork-featuremap validate <map.yaml> [--scoring <scoring.yaml>]");
    eprintln!("  refwork-featuremap schema");
}

fn cmd_validate(args: &[String]) {
    if args.is_empty() {
        eprintln!("error: validate requires <map.yaml>");
        process::exit(1);
    }

    let map_path = &args[0];
    let mut scoring_path: Option<&str> = None;

    // Parse optional --scoring flag
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scoring" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --scoring requires a path argument");
                    process::exit(1);
                }
                scoring_path = Some(&args[i]);
            }
            other => {
                eprintln!("error: unexpected argument {:?}", other);
                process::exit(1);
            }
        }
        i += 1;
    }

    // Read and parse feature map
    let map_yaml = match std::fs::read_to_string(map_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {:?}: {}", map_path, e);
            process::exit(1);
        }
    };

    let (map, map_errors) = match parse_feature_map(&map_yaml) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    };

    if let Some(sp_path) = scoring_path {
        // Cross-file validation
        let sp_yaml = match std::fs::read_to_string(sp_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {:?}: {}", sp_path, e);
                process::exit(1);
            }
        };

        let (sp, _sp_standalone_errors) = match parse_scoring_program(&sp_yaml) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        };

        // validate_pair runs map validation + scoring standalone + cross checks
        let errors = validate_pair(&map, &sp);
        if errors.is_empty() {
            process::exit(0);
        } else {
            for e in &errors {
                eprintln!("{}", e);
            }
            process::exit(1);
        }
    } else {
        // Map-only validation
        if map_errors.is_empty() {
            process::exit(0);
        } else {
            for e in &map_errors {
                eprintln!("{}", e);
            }
            process::exit(1);
        }
    }
}

fn cmd_schema() {
    print!("{}", generate_schema());
}
