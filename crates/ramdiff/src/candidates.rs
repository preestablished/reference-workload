//! `ramdiff candidates` — list surviving candidate offsets with hexdump context.
//!
//! For each surviving offset in the candidate set, prints:
//! - The offset (hex) and current value in each dump
//! - ±8 bytes of surrounding context (hexdump style) from the first dump

use crate::session::Session;

/// Options for the `candidates` subcommand.
pub struct CandidatesOpts {
    /// Number of context bytes on each side.
    pub context: usize,
    /// If set, only print at most this many candidates.
    pub limit: Option<usize>,
}

impl Default for CandidatesOpts {
    fn default() -> Self {
        CandidatesOpts {
            context: 8,
            limit: None,
        }
    }
}

/// Print candidate offsets for the session at `dir`.
pub fn run_candidates(dir: &std::path::Path, opts: &CandidatesOpts) -> Result<(), String> {
    let session = Session::load(dir)?;

    if session.candidates.offsets.is_empty() {
        println!("No candidates (run `ramdiff search` first).");
        return Ok(());
    }

    // Load dump bytes for each dump in the session.
    let all_dumps: Vec<_> = session
        .dumps
        .iter()
        .map(|meta| {
            let bytes = session.load_dump_bytes(&meta.label)?;
            Ok((meta.label.clone(), bytes))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let width = session.candidates.width;
    let byte_size = width.byte_size();

    let count = session.candidates.offsets.len();
    let limit = opts.limit.unwrap_or(count);
    println!("Candidates: {} (showing up to {})", count, limit);
    println!();

    for (i, &off) in session.candidates.offsets.iter().take(limit).enumerate() {
        println!("  [{}] offset 0x{:05X} ({}u)", i, off, off);

        // Print value in each dump.
        for (label, bytes) in &all_dumps {
            let val = width.read_value(bytes, off);
            println!("    {:>12} : {}", label, val);
        }

        // Hexdump context from the first dump (if any).
        if let Some((_, first_bytes)) = all_dumps.first() {
            let start = (off as usize).saturating_sub(opts.context);
            let end = ((off as usize) + byte_size + opts.context).min(first_bytes.len());
            let slice = &first_bytes[start..end];
            print!("    context [{:#07x}..{:#07x}]:", start, end);
            for (j, &b) in slice.iter().enumerate() {
                let abs = start + j;
                if abs >= off as usize && abs < off as usize + byte_size {
                    print!(" [{:02x}]", b); // highlight the target bytes
                } else {
                    print!("  {:02x} ", b);
                }
            }
            println!();
        }
        println!();
    }

    if count > limit {
        println!("  ... {} more (use --limit to see more)", count - limit);
    }

    Ok(())
}
