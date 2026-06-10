//! `ramdiff` library surface — exposes modules for integration tests.
//!
//! The binary is the primary consumer; this lib target exists so integration
//! tests (in `tests/`) can call the filter/session/emit APIs directly without
//! going through the CLI.

#![forbid(unsafe_code)]

pub mod candidates;
pub mod emit;
pub mod filter;
pub mod record;
pub mod session;
