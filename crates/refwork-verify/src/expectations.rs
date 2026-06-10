//! `expectations.yaml` schema — the minimal assertion language for
//! `refwork-verify map-check`.
//!
//! # Schema
//!
//! ```yaml
//! # Optional: fail map-check/double-run if this artifact was produced with
//! # --continue-past-faults (or the flag is passed on the CLI).
//! continue_past_faults: false   # default; presence in a report artifact
//!                                # causes map-check/double-run to exit 1.
//!
//! # Ordered list of point-in-time assertions.
//! assertions:
//!   - feature: room_id          # name from the feature-map
//!     at_frame: 60              # exact frame  (mutually exclusive with by_frame)
//!     equals: 1                 # expected decoded value (i64)
//!
//!   - feature: frame_ctr
//!     by_frame: 120             # true at *some* frame ≤ N
//!     changes_to: 30
//!
//!   - feature: score
//!     at_frame: 200
//!     delta: 10                 # value - previous_value == delta
//!
//! # Optional: invariants that must never hold throughout the whole run.
//! never:
//!   - feature: game_mode
//!     equals: 0xFF
//! ```
//!
//! Exactly one of `at_frame` / `by_frame` must be present.
//! Exactly one of `equals` / `changes_to` / `delta` must be present.

use serde::{Deserialize, Serialize};

/// Top-level expectations document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expectations {
    /// Ordered list of point-in-time assertions.
    #[serde(default)]
    pub assertions: Vec<Assertion>,

    /// Never-hold invariants (checked every frame).
    #[serde(default)]
    pub never: Vec<NeverClause>,
}

/// A single point-in-time assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    /// Feature name (must exist in the feature map).
    pub feature: String,

    /// Assert at exactly this frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_frame: Option<u64>,

    /// Assert true at some frame ≤ this value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_frame: Option<u64>,

    /// Decoded value must equal this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<i64>,

    /// Decoded value must have changed to this since the previous assertion
    /// for this feature (or from the initial WRAM-init value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_to: Option<i64>,

    /// Decoded value minus the previous decoded value must equal this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<i64>,
}

impl Assertion {
    /// Validate that exactly one timing and exactly one condition is specified.
    pub fn validate(&self) -> Result<(), String> {
        let timing_count = self.at_frame.is_some() as u8 + self.by_frame.is_some() as u8;
        if timing_count != 1 {
            return Err(format!(
                "assertion for feature {:?}: exactly one of at_frame/by_frame is required \
                 (got {})",
                self.feature, timing_count
            ));
        }
        let cond_count = self.equals.is_some() as u8
            + self.changes_to.is_some() as u8
            + self.delta.is_some() as u8;
        if cond_count != 1 {
            return Err(format!(
                "assertion for feature {:?}: exactly one of equals/changes_to/delta is \
                 required (got {})",
                self.feature, cond_count
            ));
        }
        Ok(())
    }

    /// The latest frame at which this assertion must fire.
    pub fn deadline(&self) -> u64 {
        self.at_frame.or(self.by_frame).unwrap_or(0)
    }
}

/// An invariant that must never hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeverClause {
    /// Feature name.
    pub feature: String,
    /// Value that must never be observed for this feature.
    pub equals: i64,
}

/// Parse `expectations.yaml` from a string.
pub fn parse_expectations(yaml: &str) -> Result<Expectations, String> {
    let exp: Expectations =
        serde_yaml::from_str(yaml).map_err(|e| format!("expectations parse error: {}", e))?;
    for a in &exp.assertions {
        a.validate()?;
    }
    Ok(exp)
}
