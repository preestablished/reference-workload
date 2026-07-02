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
pub mod phase4_bundle_check;
pub mod phase4_checksum_manifest;
pub mod phase4_context_check;
pub mod phase4_layout;
pub mod phase4_private_intake;
pub mod phase4_score_plan;
pub mod phase4_trace;
pub mod play;
pub mod redaction_scan;
pub mod vm_first_room;
pub mod vm_suite;

pub use expectations::{Assertion, Expectations, NeverClause};
pub use phase4_bundle_check::{check_phase4_bundle, Phase4BundleReport};
pub use phase4_checksum_manifest::{
    write_phase4_checksum_manifest, ChecksumManifestOptions, ChecksumManifestReport,
};
pub use phase4_context_check::{check_phase4_context_bundle, Phase4ContextReport};
pub use phase4_layout::{write_phase4_layout, LayoutOptions, LayoutReport};
pub use phase4_private_intake::{
    prepare_phase4_private_intake, PrivateIntakeOptions, PrivateIntakeReport,
};
pub use phase4_score_plan::{write_phase4_score_plan, ScorePlanOptions, ScorePlanReport};
pub use phase4_trace::{emit_phase4_trace, TraceOptions, TraceReport};
pub use play::{PlayOptions, PlayReport};
pub use redaction_scan::{scan_redactions, RedactionScanOptions, RedactionScanReport};
