#![forbid(unsafe_code)]

use refwork_harness::ctl::{ControlChannel, SeqpacketFd};
use refwork_harness::frame::{run_frame_loop, SdkPlatform};
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
    // Under the agent this binds the detchannel; standalone it is a no-op
    // and later region publication reports standalone mode.
    refwork_harness::agent::init_sdk();
    let transport = match SeqpacketFd::from_inherited_control_fd() {
        Ok(transport) => transport,
        Err(err) => {
            eprintln!("refwork-harness: fd-3 control socket unavailable: {err}");
            std::process::exit(1);
        }
    };
    let mut channel = ControlChannel::new(transport);
    let mut loader = FilesystemGameLoader;

    let setup = match run_setup(&mut channel, &mut loader, SetupConfig::default()) {
        Ok(setup) => setup,
        Err(err) => {
            eprintln!("refwork-harness: setup failed: {err}");
            std::process::exit(1);
        }
    };

    // Under the agent this drives the real detchannel: pv-pad input, ring-W
    // FrameMark, and the pv-pad FRAME_COUNTER boundary the hypervisor's
    // Run{frame_budget} stops on. Standalone it no-ops (see SdkPlatform).
    let mut platform = SdkPlatform;
    if let Err(err) = run_frame_loop(&mut channel, setup, &mut platform) {
        eprintln!("refwork-harness: frame loop failed: {err}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!("refwork-harness");
    println!();
    println!("Usage:");
    println!("  refwork-harness --help");
    println!("  refwork-harness [--fd3]");
    println!();
    println!("Default mode uses inherited fd 3 as an AF_UNIX SOCK_SEQPACKET control channel.");
    println!("After Start, the harness free-runs frames and polls control at frame boundaries.");
}
