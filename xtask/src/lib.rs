//! xtask — developer tooling for the deterministic emulator workspace.
//!
//! Subcommands:
//! - `cargo xtask audit-syms --bin PATH` — audit a release binary for banned
//!   clock, sleep, and scheduler entry points.
//! - `cargo xtask build-rom [--out PATH]` — assemble the synthetic test ROM.
//! - `cargo xtask deny` — determinism deny gate (banned token scan).
//! - `cargo xtask fetch-test-roms` — download and verify test ROM archives.
//! - `cargo xtask cpu-tests [--dir DIR] [--filter SUBSTR] [--max-fail N]` —
//!   run the single-step CPU test corpus.
//! - `cargo xtask spc-tests [--dir DIR] [--filter SUBSTR]` — validate the
//!   pinned SPC700 single-step corpus (execution gate arrives with M2).
//! - `cargo xtask hash-chain [--frames N]` — print the chained synthetic-ROM
//!   frame hash (cross-architecture determinism probe).

pub mod asm;
pub mod audit_syms;
pub mod cpu_tests;
pub mod deny;
pub mod fetch;
pub mod hash_chain;
pub mod image;
pub mod spc_tests;
pub mod synth_rom;

pub use synth_rom::build_synth_rom;
