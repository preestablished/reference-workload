#![forbid(unsafe_code)]

fn main() {
    let mut args = std::env::args();
    let _program = args.next();
    match args.next().as_deref() {
        Some("--help") | Some("-h") | None => print_help(),
        Some(other) => {
            eprintln!("refwork-harness: unknown argument `{other}`");
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!("refwork-harness");
    println!();
    println!("Usage:");
    println!("  refwork-harness --help");
    println!();
    println!("The production fd-3 control loop lands in the next package-02 bead.");
    println!("This binary currently exposes the package-02 region/meta foundation.");
}
