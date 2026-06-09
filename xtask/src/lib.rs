//! xtask — developer tooling for the deterministic emulator workspace.
//!
//! Subcommands:
//! - `cargo xtask build-rom [--out PATH]` — assemble the synthetic test ROM.
//! - `cargo xtask deny` — determinism deny gate (banned token scan).
//! - `cargo xtask fetch-test-roms` — download and verify test ROM archives.
//! - `cargo xtask cpu-tests [--dir DIR] [--filter SUBSTR] [--max-fail N]` —
//!   run the single-step CPU test corpus.

pub mod asm;
pub mod cpu_tests;
pub mod deny;
pub mod fetch;
pub mod synth_rom;

pub use synth_rom::build_synth_rom;
