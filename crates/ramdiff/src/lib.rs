//! `ramdiff` library surface — exposes modules for integration tests.
//!
//! The binary is the primary consumer; this lib target exists so integration
//! tests (in `tests/`) can call the filter/session/emit APIs directly without
//! going through the CLI.

#![forbid(unsafe_code)]

#[cfg(feature = "interactive")]
pub mod audio;
pub mod candidates;
pub mod emit;
pub mod filter;
#[cfg(all(feature = "interactive", target_os = "linux"))]
pub mod gamepad;
#[cfg(all(feature = "interactive", target_os = "macos"))]
pub mod gamepad_macos;
pub mod record;
pub mod session;
