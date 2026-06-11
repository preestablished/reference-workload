//! `refwork-verify` — host-side emulator verification library.
//!
//! Provides three verification operations as a library so integration tests
//! can call them directly without shelling out:
//!
//! - [`play`] — run a `.padlog` script against a ROM, collecting hashes,
//!   feature events, fault reports and optional framebuffer snapshots.
//! - [`map_check`] — run a script and assert an [`Expectations`] file.
//! - [`double_run`] — run a script twice from fresh cores and compare
//!   chained hashes (determinism gate).
//!
//! **Seam discipline**: all core access goes through the public
//! `refwork-emu` API surface (`Core::new`, `run_one_frame`,
//! `blit_completed_frame`, `frame_counter`, `wram`, `debug_peek`,
//! `fault`).  Nothing in this crate reaches around that facade.

#![forbid(unsafe_code)]

pub(crate) mod decode;
pub mod double_run;
pub mod expectations;
pub mod map_check;
pub mod play;

pub use expectations::{Assertion, Expectations, NeverClause};
pub use play::{PlayOptions, PlayReport};
