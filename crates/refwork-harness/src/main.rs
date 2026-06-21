#![forbid(unsafe_code)]

use refwork_harness::ctl::{ControlChannel, SeqpacketFd};
use refwork_harness::game::FilesystemGameLoader;
use refwork_harness::runner::{run_setup, SetupConfig};

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    let mode = args.next();
    if let Some(extra) = args.next() {
        eprintln!("refwork-harness: unexpected extra argument `{extra}`");
        std::process::exit(2);
    }

    match mode.as_deref() {
        Some("--help") | Some("-h") => print_help(),
        Some("--fd3") | None => run_fd3(),
        Some(other) => {
            eprintln!("refwork-harness: unknown argument `{other}`");
            std::process::exit(2);
        }
    }
}

fn run_fd3() {
    let transport = match SeqpacketFd::from_inherited_control_fd() {
        Ok(transport) => transport,
        Err(err) => {
            eprintln!("refwork-harness: fd-3 control socket unavailable: {err}");
            std::process::exit(1);
        }
    };
    let mut channel = ControlChannel::new(transport);
    let mut loader = FilesystemGameLoader;

    let _setup = match run_setup(&mut channel, &mut loader, SetupConfig::default()) {
        Ok(setup) => setup,
        Err(err) => {
            eprintln!("refwork-harness: setup failed: {err}");
            std::process::exit(1);
        }
    };

    eprintln!("refwork-harness: frame loop is not implemented in this build");
    std::process::exit(1);
}

fn print_help() {
    println!("refwork-harness");
    println!();
    println!("Usage:");
    println!("  refwork-harness --help");
    println!("  refwork-harness [--fd3]");
    println!();
    println!("Default mode uses inherited fd 3 as an AF_UNIX SOCK_SEQPACKET control channel.");
    println!(
        "Setup performs Hello, LoadGame, GameLoaded, RegisterRegion, Ready, then waits for Start."
    );
}
