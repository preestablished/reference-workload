//! `refwork-featuremap` — feature-map serde types, validator, and JSON Schema generator.
//!
//! Implements API.md §1 (feature-map schema) and §2 (scoring-program DSL).
//!
//! # Forward compatibility
//!
//! Unknown **optional** fields are silently ignored — do NOT add `deny_unknown_fields`
//! anywhere in this model. This is our interpretation of API.md preamble's
//! "reject unknown required-context fields" clause: serde cannot distinguish
//! unknown-optional from unknown-required-context without a convention the spec does
//! not define, so `schema_version` major gating + `kind` gating carry that duty.
//!
//! # Feature-name pattern (doc-reconciliation note)
//!
//! refwork API.md §1.2 allows `[a-z0-9_]+`; state-scorer §3/§4 requires
//! `^[a-z][a-z0-9_]*$` with a 64-char cap. We validate the stricter intersection
//! `^[a-z][a-z0-9_]{0,63}$` so a refwork-valid map can never fail scorer load on
//! naming. See overview doc-reconciliation item 3.

#![forbid(unsafe_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

// ─── Int-or-hex newtype ───────────────────────────────────────────────────────

/// A field that accepts either a decimal integer or a `0x`/`0X` hex string.
///
/// Used for `offset` (feature entry) and `value` (predicate leaf). The JSON Schema
/// emits `anyOf: [{type: integer}, {type: string, pattern: "^0[xX][0-9a-fA-F]+$"}]`
/// so both forms are schema-valid too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct IntOrHex(pub i64);

impl<'de> Deserialize<'de> for IntOrHex {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = IntOrHex;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "an integer or a hex string like \"0x0AF6\"")
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<IntOrHex, E> {
                Ok(IntOrHex(v))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<IntOrHex, E> {
                Ok(IntOrHex(v as i64))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<IntOrHex, E> {
                let hex = if v.starts_with("0x") || v.starts_with("0X") {
                    &v[2..]
                } else {
                    return Err(E::invalid_value(
                        serde::de::Unexpected::Str(v),
                        &"a hex string starting with 0x or 0X",
                    ));
                };
                i64::from_str_radix(hex, 16).map(IntOrHex).map_err(|_| {
                    E::invalid_value(serde::de::Unexpected::Str(v), &"a valid hex integer")
                })
            }
        }
        d.deserialize_any(Visitor)
    }
}

impl schemars::JsonSchema for IntOrHex {
    fn schema_name() -> String {
        "IntOrHex".to_string()
    }
    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec, StringValidation};
        // anyOf: [ {type: integer}, {type: string, pattern: "^0[xX][0-9a-fA-F]+$"} ]
        let int_schema = Schema::Object(SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Integer))),
            ..Default::default()
        });
        let str_schema = Schema::Object(SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            string: Some(Box::new(StringValidation {
                pattern: Some("^0[xX][0-9a-fA-F]+$".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        });
        Schema::Object(SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                any_of: Some(vec![int_schema, str_schema]),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

// ─── Feature-map types (API.md §1.1 / §1.2) ──────────────────────────────────

/// Top-level feature-map document (API.md §1.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureMap {
    pub schema_version: u32,
    pub kind: String,
    pub meta: FeatureMapMeta,
    pub regions: Vec<RegionDecl>,
    pub features: Vec<Feature>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureMapMeta {
    pub name: String,
    pub workload: String,
    pub game_revision: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegionDecl {
    pub name: String,
    pub size: u64,
}

/// A feature entry (API.md §1.2).
///
/// Feature-name pattern: `^[a-z][a-z0-9_]{0,63}$` — the strict intersection of
/// refwork §1.2 `[a-z0-9_]+` and state-scorer §3/§4 `^[a-z][a-z0-9_]*$` + 64-char cap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Feature {
    pub name: String,
    pub region: String,
    pub offset: IntOrHex,
    #[serde(rename = "type")]
    pub feature_type: FeatureType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    pub semantics: Semantics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub stability: Stability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discretize: Option<Discretize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_when: Option<ValidWhen>,
}

/// Feature data type (API.md §1.2). Endianness is explicit — no platform default (rule 6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FeatureType {
    U8,
    #[serde(rename = "u16le")]
    U16le,
    #[serde(rename = "u16be")]
    U16be,
    #[serde(rename = "u32le")]
    U32le,
    #[serde(rename = "u32be")]
    U32be,
    I8,
    #[serde(rename = "i16le")]
    I16le,
    #[serde(rename = "i16be")]
    I16be,
    #[serde(rename = "i32le")]
    I32le,
    #[serde(rename = "i32be")]
    I32be,
    #[serde(rename = "bitflags8")]
    Bitflags8,
    #[serde(rename = "bitflags16le")]
    Bitflags16le,
    #[serde(rename = "bitflags32le")]
    Bitflags32le,
    #[serde(rename = "bcd8")]
    Bcd8,
    #[serde(rename = "bcd16le")]
    Bcd16le,
    Bytes,
}

impl FeatureType {
    /// Returns the derived byte width for non-`bytes` types. `None` for `bytes`
    /// (width is supplied explicitly).
    pub fn derived_width(&self) -> Option<u32> {
        match self {
            FeatureType::U8 | FeatureType::I8 | FeatureType::Bitflags8 | FeatureType::Bcd8 => {
                Some(1)
            }
            FeatureType::U16le
            | FeatureType::U16be
            | FeatureType::I16le
            | FeatureType::I16be
            | FeatureType::Bitflags16le
            | FeatureType::Bcd16le => Some(2),
            FeatureType::U32le
            | FeatureType::U32be
            | FeatureType::I32le
            | FeatureType::I32be
            | FeatureType::Bitflags32le => Some(4),
            FeatureType::Bytes => None,
        }
    }

    /// Returns true for the ten integer scalar types (u*/i* but not bitflags/bcd/bytes).
    pub fn is_integer_scalar(&self) -> bool {
        matches!(
            self,
            FeatureType::U8
                | FeatureType::U16le
                | FeatureType::U16be
                | FeatureType::U32le
                | FeatureType::U32be
                | FeatureType::I8
                | FeatureType::I16le
                | FeatureType::I16be
                | FeatureType::I32le
                | FeatureType::I32be
        )
    }

    /// Returns true for bitflags* types.
    pub fn is_bitflags(&self) -> bool {
        matches!(
            self,
            FeatureType::Bitflags8 | FeatureType::Bitflags16le | FeatureType::Bitflags32le
        )
    }

    /// Returns the max valid bit index for a bitflags type.
    pub fn max_bit(&self) -> Option<u8> {
        match self {
            FeatureType::Bitflags8 => Some(7),
            FeatureType::Bitflags16le => Some(15),
            FeatureType::Bitflags32le => Some(31),
            _ => None,
        }
    }
}

/// Feature semantics — open enum; unknown values parse as `Opaque`.
///
/// API.md §1.2: "consumers treat unknown values as `opaque`".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Semantics {
    Counter,
    PositionX,
    PositionY,
    RoomId,
    Health,
    Resource,
    Flags,
    Mode,
    ProgressFlag,
    Timer,
    Opaque,
    /// Unknown variant — treated as opaque per forward-compat spec.
    #[serde(other)]
    #[schemars(skip)]
    Unknown,
}

/// Feature stability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Stability {
    Stable,
    Volatile,
}

/// Discretization hint (API.md §1.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Discretize {
    Identity,
    Bucket {
        size: u64,
    },
    Bits,
    Threshold {
        edges: Vec<i64>,
    },
    Grid {
        x: String,
        y: String,
        room: String,
        cell_w: u64,
        cell_h: u64,
    },
    None,
}

/// `valid_when` guard (API.md §1.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ValidWhen {
    pub feature: String,
    pub op: CompareOp,
    pub value: IntOrHex,
}

/// Comparison operator for leaf predicates (equality/ordering operations).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

// ─── Scoring-program types (API.md §2) ────────────────────────────────────────

/// Top-level scoring-program document (API.md §2.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoringProgram {
    pub schema_version: u32,
    pub kind: String,
    pub meta: ScoringMeta,
    pub stages: Stages,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaping: Option<Vec<Shaping>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalties: Option<Vec<Penalty>>,
    pub goal: Goal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoringMeta {
    pub name: String,
    pub feature_map: String,
    pub version: u32,
}

/// Stages block with `monotone` flag and ordered list.
///
/// `monotone` defaults to `true` per state-scorer's JSON Schema (optional, default true).
/// `monotone: false` is reserved and rejected in v1 (doc-reconciliation: state-scorer §4
/// makes this a compile warning; refwork §2.2 doesn't mention it. We treat it as a hard
/// error per the owner-doc's "monotone milestones" semantics.)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stages {
    #[serde(default = "default_true")]
    pub monotone: bool,
    pub list: Vec<Stage>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub points: i64,
    pub when: Pred,
    /// Stage dependencies — names earlier stages that must be reached first.
    ///
    /// Present in state-scorer's normative JSON Schema (maxItems: 8, "names only earlier
    /// stages") but absent from refwork API.md §2. We model + validate it here; see
    /// overview doc-reconciliation item 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,
}

/// Shaping term (API.md §2.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shaping {
    pub name: String,
    pub weight: i64,
    pub expr: ShapingExpr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapingExpr {
    pub feature: String,
    pub reduce: Reduce,
}

/// Reduce function for shaping expressions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reduce {
    Identity,
    Popcount,
}

/// Penalty entry (API.md §2.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Penalty {
    pub name: String,
    pub when: Pred,
    pub action: PenaltyAction,
}

/// Penalty action — currently only `prune`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PenaltyAction {
    Prune,
}

/// Goal block (API.md §2.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub name: String,
    pub predicate: Pred,
}

// ─── Predicate grammar (API.md §2.3) ─────────────────────────────────────────

/// Recursive predicate grammar (API.md §2.3).
///
/// `Pred := Leaf | { all: [Pred,…] } | { any: [Pred,…] } | { not: Pred }`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pred {
    All { all: Vec<Pred> },
    Any { any: Vec<Pred> },
    Not { not: Box<Pred> },
    Leaf(PredLeaf),
}

/// Leaf predicate — either a comparison or a bit-test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PredLeaf {
    Compare {
        feature: String,
        op: CompareOp,
        value: IntOrHex,
    },
    BitTest {
        feature: String,
        op: BitOp,
        bit: u8,
    },
}

impl PredLeaf {
    pub fn feature_name(&self) -> &str {
        match self {
            PredLeaf::Compare { feature, .. } => feature,
            PredLeaf::BitTest { feature, .. } => feature,
        }
    }
}

/// Bit-test operators for leaf predicates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BitOp {
    BitSet,
    BitClear,
}

// ─── Permissive envelope (for pre-parse version/kind checks) ─────────────────

/// Minimal envelope parsed first to extract `schema_version` and `kind` before
/// the full model is deserialized. This enables collecting preamble errors with
/// stable rule ids (`preamble/version`, `preamble/kind`) rather than serde parse
/// noise — important for the fixture manifest.
#[derive(Debug, Deserialize)]
struct Envelope {
    schema_version: Option<serde_json::Value>,
    kind: Option<String>,
}

// ─── Validation errors ────────────────────────────────────────────────────────

/// A single validation finding. All errors are collected (not fail-fast).
///
/// `rule` ids:
/// - `"1.3/1"` … `"1.3/7"` — literal §1.3 spec rules
/// - `"1.2/width"` — §1.2 bytes width required
/// - `"1.2/width-mismatch"` — §1.2 width != derived
/// - `"1.2/guard"` — §1.2 valid_when references volatile/missing feature
/// - `"2/monotone"` — monotone: false rejected
/// - `"2/points"` — points < 0
/// - `"2/stable-only"` — volatile feature in predicate/shaping (§1.3/3)
/// - `"preamble/version"` — schema_version major != 1
/// - `"preamble/kind"` — wrong kind value
/// - `"x/feature-exists"` — scoring references feature not in map
/// - `"x/bitop-target"` — bit_set/bit_clear/popcount on non-bitflags feature
/// - `"x/bit-width"` — bit index exceeds type width
/// - `"x/featmap-ref"` — scoring.meta.feature_map != map.meta.name
/// - `"x/requires"` — stage.requires names non-earlier or missing stage
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub rule: &'static str,
    pub path: String,
    pub msg: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.rule, self.path, self.msg)
    }
}

fn err(rule: &'static str, path: impl Into<String>, msg: impl Into<String>) -> ValidationError {
    ValidationError {
        rule,
        path: path.into(),
        msg: msg.into(),
    }
}

// ─── Feature-name pattern ─────────────────────────────────────────────────────

// Inline pattern check to avoid adding a regex dep for a simple anchored pattern.
// Validates `^[a-z][a-z0-9_]{0,63}$`.
fn valid_feature_name(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return false;
    }
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

// ─── Validate feature map ─────────────────────────────────────────────────────

/// Validate a feature map in isolation. Returns all collected errors.
pub fn validate_map(map: &FeatureMap) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Envelope checks already done by the caller (parse_feature_map_with_checks).

    // Build region index
    let region_map: HashMap<&str, u64> = map
        .regions
        .iter()
        .map(|r| (r.name.as_str(), r.size))
        .collect();

    // Build feature index (name → Feature)
    let mut feature_index: HashMap<&str, &Feature> = HashMap::new();

    // §1.3/1 — names unique; regions exist; offsets in bounds
    for (i, feat) in map.features.iter().enumerate() {
        let path = format!("features[{}].{}", i, feat.name);

        // Name pattern check (strict intersection, see module doc)
        if !valid_feature_name(&feat.name) {
            errors.push(err(
                "1.3/1",
                format!("features[{}].name", i),
                format!(
                    "feature name {:?} does not match ^[a-z][a-z0-9_]{{0,63}}$",
                    feat.name
                ),
            ));
        }

        // Uniqueness
        if feature_index.contains_key(feat.name.as_str()) {
            errors.push(err(
                "1.3/1",
                format!("features[{}].name", i),
                format!("duplicate feature name {:?}", feat.name),
            ));
        } else {
            feature_index.insert(&feat.name, feat);
        }

        // Region declared
        let region_size = if let Some(&sz) = region_map.get(feat.region.as_str()) {
            sz
        } else {
            errors.push(err(
                "1.3/1",
                format!("features[{}].region", i),
                format!("region {:?} not declared in regions", feat.region),
            ));
            continue; // can't check bounds without size
        };

        // Width resolution and §1.2 width rules
        let width = match resolve_width(feat, &path, &mut errors) {
            Some(w) => w,
            None => continue,
        };

        // Offset + width in bounds
        let offset = feat.offset.0 as u64;
        if offset.saturating_add(width as u64) > region_size {
            errors.push(err(
                "1.3/1",
                format!("{}.offset", path),
                format!(
                    "offset 0x{:X} + width {} = {} exceeds region {:?} size {}",
                    offset,
                    width,
                    offset + width as u64,
                    feat.region,
                    region_size
                ),
            ));
        }
    }

    // §1.3/2 — bitflags* only with discretize: bits or none
    for (i, feat) in map.features.iter().enumerate() {
        if feat.feature_type.is_bitflags() {
            let ok = matches!(
                &feat.discretize,
                None | Some(Discretize::Bits) | Some(Discretize::None)
            );
            if !ok {
                errors.push(err(
                    "1.3/2",
                    format!("features[{}].discretize", i),
                    format!(
                        "bitflags feature {:?} must use discretize: bits or none",
                        feat.name
                    ),
                ));
            }
        }
    }

    // §1.3/7 — grid discretize rules
    for (i, feat) in map.features.iter().enumerate() {
        if let Some(Discretize::Grid {
            x,
            y,
            room,
            cell_w,
            cell_h,
        }) = &feat.discretize
        {
            validate_grid(
                i,
                feat,
                x,
                y,
                room,
                *cell_w,
                *cell_h,
                &feature_index,
                &mut errors,
            );
        }
    }

    // §1.2 guard — valid_when references a stable, existing feature
    for (i, feat) in map.features.iter().enumerate() {
        if let Some(vw) = &feat.valid_when {
            let path = format!("features[{}].valid_when.feature", i);
            match feature_index.get(vw.feature.as_str()) {
                None => {
                    errors.push(err(
                        "1.2/guard",
                        path,
                        format!("valid_when references unknown feature {:?}", vw.feature),
                    ));
                }
                Some(ref_feat) if ref_feat.stability != Stability::Stable => {
                    errors.push(err(
                        "1.2/guard",
                        path,
                        format!(
                            "valid_when references volatile feature {:?} (must be stable)",
                            vw.feature
                        ),
                    ));
                }
                _ => {}
            }
        }
    }

    errors
}

fn resolve_width(feat: &Feature, path: &str, errors: &mut Vec<ValidationError>) -> Option<u32> {
    match &feat.feature_type {
        FeatureType::Bytes => match feat.width {
            None => {
                errors.push(err(
                    "1.2/width",
                    format!("{}.width", path),
                    format!("feature {:?} has type: bytes but no width", feat.name),
                ));
                None
            }
            Some(w) => Some(w),
        },
        t => {
            let derived = t.derived_width().expect("non-bytes type has derived width");
            if let Some(declared) = feat.width {
                if declared != derived {
                    errors.push(err(
                        "1.2/width-mismatch",
                        format!("{}.width", path),
                        format!(
                            "feature {:?} has type derived width {} but declared width {}",
                            feat.name, derived, declared
                        ),
                    ));
                    return None;
                }
            }
            Some(derived)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_grid(
    feat_idx: usize,
    feat: &Feature,
    x: &str,
    y: &str,
    room: &str,
    cell_w: u64,
    cell_h: u64,
    feature_index: &HashMap<&str, &Feature>,
    errors: &mut Vec<ValidationError>,
) {
    let base = format!("features[{}].discretize", feat_idx);

    // cell_w/cell_h >= 1
    if cell_w < 1 {
        errors.push(err(
            "1.3/7",
            format!("{}.cell_w", base),
            "cell_w must be >= 1".to_string(),
        ));
    }
    if cell_h < 1 {
        errors.push(err(
            "1.3/7",
            format!("{}.cell_h", base),
            "cell_h must be >= 1".to_string(),
        ));
    }

    // x, y, room must name features in the same map
    for (field, name) in [("x", x), ("y", y), ("room", room)] {
        match feature_index.get(name) {
            None => {
                errors.push(err(
                    "1.3/7",
                    format!("{}.{}", base, field),
                    format!("grid {field} references unknown feature {name:?}"),
                ));
            }
            Some(ref_feat) => {
                // x/y must be integer scalar types
                if field == "x" || field == "y" {
                    if !ref_feat.feature_type.is_integer_scalar() {
                        errors.push(err(
                            "1.3/7",
                            format!("{}.{}", base, field),
                            format!(
                                "grid {field} feature {:?} must be an integer scalar type",
                                name
                            ),
                        ));
                    }
                    // x/y must not carry their own non-none discretize except this grid
                    let is_carrier = ref_feat.name == feat.name;
                    if !is_carrier {
                        let bad = !matches!(&ref_feat.discretize, None | Some(Discretize::None));
                        if bad {
                            errors.push(err(
                                "1.3/7",
                                format!("{}.{}", base, field),
                                format!(
                                    "grid {field} feature {:?} has non-none discretize (double-counting)",
                                    name
                                ),
                            ));
                        }
                    }
                }
                // room must be stable
                if field == "room" && ref_feat.stability != Stability::Stable {
                    errors.push(err(
                        "1.3/7",
                        format!("{}.room", base),
                        format!("grid room feature {:?} must be stability: stable", name),
                    ));
                }
            }
        }
    }
}

// ─── Validate scoring program (map-independent rules) ─────────────────────────

/// Validate scoring-program structure without the paired map.
pub fn validate_scoring_standalone(sp: &ScoringProgram) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // monotone: false rejected (reserved in v1)
    if !sp.stages.monotone {
        errors.push(err(
            "2/monotone",
            "stages.monotone".to_string(),
            "monotone: false is reserved and rejected in v1".to_string(),
        ));
    }

    // points >= 0
    for (i, stage) in sp.stages.list.iter().enumerate() {
        if stage.points < 0 {
            errors.push(err(
                "2/points",
                format!("stages.list[{}].points", i),
                format!(
                    "stage {:?} has negative points {}",
                    stage.name, stage.points
                ),
            ));
        }
    }

    // bit: 32 rejected (§2.3 grammar: bit 0..=31)
    collect_pred_errors_scoring(sp, &mut errors);

    errors
}

fn collect_pred_errors_scoring(sp: &ScoringProgram, errors: &mut Vec<ValidationError>) {
    for (i, stage) in sp.stages.list.iter().enumerate() {
        check_pred_bit_range(
            &stage.when,
            &format!("stages.list[{}].when", i),
            errors,
            None,
        );
    }
    if let Some(penalties) = &sp.penalties {
        for (i, p) in penalties.iter().enumerate() {
            check_pred_bit_range(&p.when, &format!("penalties[{}].when", i), errors, None);
        }
    }
    check_pred_bit_range(&sp.goal.predicate, "goal.predicate", errors, None);
}

/// Check that `bit` values in leaf predicates are within 0..=31.
fn check_pred_bit_range(
    pred: &Pred,
    path: &str,
    errors: &mut Vec<ValidationError>,
    feature_type: Option<&FeatureType>,
) {
    match pred {
        Pred::All { all } => {
            for (i, p) in all.iter().enumerate() {
                check_pred_bit_range(p, &format!("{}.all[{}]", path, i), errors, None);
            }
        }
        Pred::Any { any } => {
            for (i, p) in any.iter().enumerate() {
                check_pred_bit_range(p, &format!("{}.any[{}]", path, i), errors, None);
            }
        }
        Pred::Not { not } => {
            check_pred_bit_range(not, &format!("{}.not", path), errors, None);
        }
        Pred::Leaf(PredLeaf::BitTest { bit, .. }) => {
            let _ = feature_type; // only the map-cross-check uses this
            if *bit > 31 {
                errors.push(err(
                    "x/bit-width",
                    path.to_string(),
                    format!("bit {} exceeds maximum 31 (§2.3)", bit),
                ));
            }
        }
        Pred::Leaf(PredLeaf::Compare { .. }) => {}
    }
}

// ─── Cross-file validation ────────────────────────────────────────────────────

/// Validate the scoring program against the paired feature map.
/// Returns all errors from both sources combined.
pub fn validate_pair(map: &FeatureMap, sp: &ScoringProgram) -> Vec<ValidationError> {
    let mut errors = validate_map(map);
    errors.extend(validate_scoring_standalone(sp));

    let feature_index: HashMap<&str, &Feature> =
        map.features.iter().map(|f| (f.name.as_str(), f)).collect();

    // x/featmap-ref — scoring.meta.feature_map == map.meta.name
    if sp.meta.feature_map != map.meta.name {
        errors.push(err(
            "x/featmap-ref",
            "meta.feature_map".to_string(),
            format!(
                "scoring.meta.feature_map {:?} != map.meta.name {:?}",
                sp.meta.feature_map, map.meta.name
            ),
        ));
    }

    // x/requires — stage requires entries name earlier stages, max 8
    let stage_names: Vec<&str> = sp.stages.list.iter().map(|s| s.name.as_str()).collect();
    for (i, stage) in sp.stages.list.iter().enumerate() {
        if let Some(reqs) = &stage.requires {
            if reqs.len() > 8 {
                errors.push(err(
                    "x/requires",
                    format!("stages.list[{}].requires", i),
                    format!(
                        "stage {:?} has {} requires entries (max 8)",
                        stage.name,
                        reqs.len()
                    ),
                ));
            }
            for req in reqs {
                // Must be an earlier stage (index < i)
                if stage_names[..i]
                    .iter()
                    .position(|&n| n == req.as_str())
                    .is_none()
                {
                    errors.push(err(
                        "x/requires",
                        format!("stages.list[{}].requires", i),
                        format!(
                            "stage {:?} requires {:?} which is not an earlier stage",
                            stage.name, req
                        ),
                    ));
                }
            }
        }
    }

    // Collect all feature references from the scoring program
    for (i, stage) in sp.stages.list.iter().enumerate() {
        let path = format!("stages.list[{}].when", i);
        check_pred_cross(&stage.when, &path, &feature_index, &mut errors);
    }
    if let Some(penalties) = &sp.penalties {
        for (i, p) in penalties.iter().enumerate() {
            let path = format!("penalties[{}].when", i);
            check_pred_cross(&p.when, &path, &feature_index, &mut errors);
        }
    }
    check_pred_cross(
        &sp.goal.predicate,
        "goal.predicate",
        &feature_index,
        &mut errors,
    );

    // Shaping: feature must exist, be stable (hard error per §2.2 "MUST be stable";
    // state-scorer §4 treats shaping as a warning — doc-reconciliation item 2,
    // owner doc wins), not bytes, popcount only on bitflags.
    if let Some(shaping) = &sp.shaping {
        for (i, s) in shaping.iter().enumerate() {
            let fname = &s.expr.feature;
            let path = format!("shaping[{}].expr.feature", i);
            match feature_index.get(fname.as_str()) {
                None => {
                    errors.push(err(
                        "x/feature-exists",
                        path,
                        format!("shaping references unknown feature {:?}", fname),
                    ));
                }
                Some(feat) => {
                    if feat.stability != Stability::Stable {
                        errors.push(err(
                            "2/stable-only",
                            path.clone(),
                            format!(
                                "shaping references volatile feature {:?} (§2.2 MUST be stable)",
                                fname
                            ),
                        ));
                    }
                    if feat.feature_type == FeatureType::Bytes {
                        errors.push(err(
                            "1.3/4",
                            path.clone(),
                            format!("shaping references bytes feature {:?}", fname),
                        ));
                    }
                    if s.expr.reduce == Reduce::Popcount && !feat.feature_type.is_bitflags() {
                        errors.push(err(
                            "x/bitop-target",
                            path,
                            format!(
                                "popcount reduce on {:?} requires a bitflags* feature type",
                                fname
                            ),
                        ));
                    }
                }
            }
        }
    }

    errors
}

/// Walk a predicate and check cross-file rules.
fn check_pred_cross(
    pred: &Pred,
    path: &str,
    feature_index: &HashMap<&str, &Feature>,
    errors: &mut Vec<ValidationError>,
) {
    match pred {
        Pred::All { all } => {
            for (i, p) in all.iter().enumerate() {
                check_pred_cross(p, &format!("{}.all[{}]", path, i), feature_index, errors);
            }
        }
        Pred::Any { any } => {
            for (i, p) in any.iter().enumerate() {
                check_pred_cross(p, &format!("{}.any[{}]", path, i), feature_index, errors);
            }
        }
        Pred::Not { not } => {
            check_pred_cross(not, &format!("{}.not", path), feature_index, errors);
        }
        Pred::Leaf(leaf) => {
            let fname = leaf.feature_name();
            match feature_index.get(fname) {
                None => {
                    errors.push(err(
                        "x/feature-exists",
                        path.to_string(),
                        format!("predicate references unknown feature {:?}", fname),
                    ));
                }
                Some(feat) => {
                    // §1.3/3 — must be stable
                    if feat.stability != Stability::Stable {
                        errors.push(err(
                            "2/stable-only",
                            path.to_string(),
                            format!("predicate references volatile feature {:?} (§1.3/3)", fname),
                        ));
                    }
                    // §1.3/4 — bytes excluded
                    if feat.feature_type == FeatureType::Bytes {
                        errors.push(err(
                            "1.3/4",
                            path.to_string(),
                            format!("predicate references bytes feature {:?}", fname),
                        ));
                    }
                    // x/bitop-target and x/bit-width for bit tests
                    if let PredLeaf::BitTest {
                        feature: _,
                        op: _,
                        bit,
                    } = leaf
                    {
                        if !feat.feature_type.is_bitflags() {
                            errors.push(err(
                                "x/bitop-target",
                                path.to_string(),
                                format!(
                                    "bit_set/bit_clear on {:?} requires a bitflags* feature type",
                                    fname
                                ),
                            ));
                        } else if let Some(max) = feat.feature_type.max_bit() {
                            if *bit > max {
                                errors.push(err(
                                    "x/bit-width",
                                    path.to_string(),
                                    format!(
                                        "bit {} exceeds max bit {} for {:?} ({:?})",
                                        bit, max, fname, feat.feature_type
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─── Parse with envelope pre-check ───────────────────────────────────────────

/// Parse a feature-map YAML document. Checks `schema_version` and `kind` via a
/// permissive envelope first, then deserializes the full model.
pub fn parse_feature_map(yaml: &str) -> Result<(FeatureMap, Vec<ValidationError>), String> {
    // Step 1: parse the envelope
    let env: Envelope =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {}", e))?;

    let mut preamble_errors = Vec::new();

    match &env.schema_version {
        None => {
            preamble_errors.push(err(
                "preamble/version",
                "schema_version".to_string(),
                "schema_version is required".to_string(),
            ));
        }
        Some(v) => {
            let major = match v {
                serde_json::Value::Number(n) => n.as_u64(),
                _ => None,
            };
            match major {
                Some(1) => {}
                Some(n) => {
                    preamble_errors.push(err(
                        "preamble/version",
                        "schema_version".to_string(),
                        format!("unsupported schema_version major {}, expected 1", n),
                    ));
                }
                None => {
                    preamble_errors.push(err(
                        "preamble/version",
                        "schema_version".to_string(),
                        "schema_version must be an integer".to_string(),
                    ));
                }
            }
        }
    }

    match &env.kind {
        None => {
            preamble_errors.push(err(
                "preamble/kind",
                "kind".to_string(),
                "kind is required".to_string(),
            ));
        }
        Some(k) if k != "feature-map" => {
            preamble_errors.push(err(
                "preamble/kind",
                "kind".to_string(),
                format!("expected kind \"feature-map\", got {:?}", k),
            ));
        }
        _ => {}
    }

    if !preamble_errors.is_empty() {
        return Ok((
            // Return a dummy map when the envelope is bad so callers can still
            // collect the preamble errors; the caller should check for errors first.
            FeatureMap {
                schema_version: 0,
                kind: String::new(),
                meta: FeatureMapMeta {
                    name: String::new(),
                    workload: String::new(),
                    game_revision: String::new(),
                    version: 0,
                    authors: None,
                },
                regions: Vec::new(),
                features: Vec::new(),
            },
            preamble_errors,
        ));
    }

    // Step 2: deserialize the full model
    let map: FeatureMap =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {}", e))?;

    let validation_errors = validate_map(&map);
    Ok((map, validation_errors))
}

/// Parse a scoring-program YAML document. Checks `schema_version` and `kind` first.
pub fn parse_scoring_program(yaml: &str) -> Result<(ScoringProgram, Vec<ValidationError>), String> {
    let env: Envelope =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {}", e))?;

    let mut preamble_errors = Vec::new();

    match &env.schema_version {
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(1) => {}
        Some(other) => {
            preamble_errors.push(err(
                "preamble/version",
                "schema_version".to_string(),
                format!("unsupported schema_version {:?}, expected 1", other),
            ));
        }
        None => {
            preamble_errors.push(err(
                "preamble/version",
                "schema_version".to_string(),
                "schema_version is required".to_string(),
            ));
        }
    }

    match &env.kind {
        Some(k) if k == "scoring-program" => {}
        Some(k) => {
            preamble_errors.push(err(
                "preamble/kind",
                "kind".to_string(),
                format!("expected kind \"scoring-program\", got {:?}", k),
            ));
        }
        None => {
            preamble_errors.push(err(
                "preamble/kind",
                "kind".to_string(),
                "kind is required".to_string(),
            ));
        }
    }

    if !preamble_errors.is_empty() {
        return Err(preamble_errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"));
    }

    let sp: ScoringProgram =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {}", e))?;

    let errors = validate_scoring_standalone(&sp);
    Ok((sp, errors))
}

// ─── Schema generation ────────────────────────────────────────────────────────

/// Generate the feature-map JSON Schema as a pretty-printed string (deterministic
/// key order via serde_json's default BTreeMap-backed output).
pub fn generate_schema() -> String {
    let schema = schemars::schema_for!(FeatureMap);
    let mut s = serde_json::to_string_pretty(&schema).expect("schema serialization failed");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// §1.4 worked example YAML (verbatim from API.md).
    const DEMO_MAP_YAML: &str = r#"
schema_version: 1
kind: feature-map
meta:
  name: demo-game
  workload: refwork-demo
  game_revision: "operator-set"
  version: 1
regions:
  - { name: wram, size: 131072 }
features:
  - name: room_id
    region: wram
    offset: 0x079B
    type: u16le
    semantics: room_id
    description: "Current room/area identifier"
    stability: stable
    discretize: { kind: identity }

  - name: area_id
    region: wram
    offset: 0x079F
    type: u8
    semantics: room_id
    description: "Macro area/zone index"
    stability: stable
    discretize: { kind: identity }

  - name: player_x
    region: wram
    offset: 0x0AF6
    type: u16le
    semantics: position_x
    description: "Player X position in room, pixels"
    stability: volatile
    discretize:
      { kind: grid, x: player_x, y: player_y, room: room_id, cell_w: 32, cell_h: 32 }

  - name: player_y
    region: wram
    offset: 0x0AFA
    type: u16le
    semantics: position_y
    description: "Player Y position in room, pixels"
    stability: volatile
    discretize: { kind: none }

  - name: health
    region: wram
    offset: 0x09C2
    type: u16le
    semantics: health
    description: "Current health points"
    stability: stable
    discretize: { kind: threshold, edges: [1, 30, 100, 300] }

  - name: upgrade_flags
    region: wram
    offset: 0x09A4
    type: bitflags16le
    semantics: flags
    description: "Collected-upgrade bitmask (bit0=mobility-1, bit1=weapon-1, ...)"
    stability: stable
    discretize: { kind: bits }

  - name: boss_flags
    region: wram
    offset: 0x7829
    type: bitflags8
    semantics: progress_flag
    description: "Boss-defeated bitmask (bit0=boss1 ... bit3=final boss)"
    stability: stable
    discretize: { kind: bits }

  - name: game_mode
    region: wram
    offset: 0x0998
    type: u8
    semantics: mode
    description: "Main state machine: 0x07=gameplay, 0x26=credits, others=menu/load/cutscene"
    stability: stable
    discretize: { kind: identity }

  - name: credits_flag
    region: wram
    offset: 0x09DA
    type: u8
    semantics: progress_flag
    description: "Nonzero once the end-credits sequence has started"
    stability: stable
    discretize: { kind: identity }
"#;

    #[test]
    fn round_trip_demo_map() {
        let (map, errors) = parse_feature_map(DEMO_MAP_YAML).expect("parse should succeed");
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(map.meta.name, "demo-game");
        assert_eq!(map.features.len(), 9);

        // re-serialize → re-parse
        let yaml2 = serde_yaml::to_string(&map).expect("serialize");
        let (map2, errors2) = parse_feature_map(&yaml2).expect("re-parse");
        assert!(errors2.is_empty(), "re-parse errors: {:?}", errors2);
        assert_eq!(map, map2);
    }

    #[test]
    fn offset_int_and_hex_equivalent() {
        let yaml_int = r#"
schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: [{ name: wram, size: 131072 }]
features:
  - name: x_int
    region: wram
    offset: 2806
    type: u8
    semantics: counter
    stability: stable
"#;
        let yaml_hex = r#"
schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: [{ name: wram, size: 131072 }]
features:
  - name: x_hex
    region: wram
    offset: "0x0AF6"
    type: u8
    semantics: counter
    stability: stable
"#;
        let (m_int, _) = parse_feature_map(yaml_int).unwrap();
        let (m_hex, _) = parse_feature_map(yaml_hex).unwrap();
        // 0x0AF6 == 2806
        assert_eq!(m_int.features[0].offset, m_hex.features[0].offset);
        assert_eq!(m_int.features[0].offset.0, 2806);
    }

    #[test]
    fn open_enum_unknown_semantics_parses_as_opaque() {
        let yaml = r#"
schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: [{ name: wram, size: 131072 }]
features:
  - name: mystery
    region: wram
    offset: 0
    type: u8
    semantics: warp_gate
    stability: stable
"#;
        let (map, errors) = parse_feature_map(yaml).unwrap();
        assert!(errors.is_empty(), "errors: {:?}", errors);
        assert_eq!(map.features[0].semantics, Semantics::Unknown);
    }

    #[test]
    fn unknown_optional_field_ignored() {
        let yaml = r#"
schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: [{ name: wram, size: 131072 }]
features:
  - name: x
    region: wram
    offset: 0
    type: u8
    semantics: counter
    stability: stable
    some_future_field: yes_really
"#;
        let result = parse_feature_map(yaml);
        assert!(result.is_ok(), "should not reject unknown fields");
    }

    #[test]
    fn unknown_kind_rejected() {
        let yaml = r#"
schema_version: 1
kind: not-a-feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: []
features: []
"#;
        let (_, errors) = parse_feature_map(yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "preamble/kind"),
            "expected preamble/kind error, got: {:?}",
            errors
        );
    }

    #[test]
    fn schema_version_2_rejected() {
        let yaml = r#"
schema_version: 2
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions: []
features: []
"#;
        let (_, errors) = parse_feature_map(yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "preamble/version"),
            "expected preamble/version error, got: {:?}",
            errors
        );
    }

    // ── §1.3 rule tests ──────────────────────────────────────────────────────

    fn minimal_map_yaml(features_block: &str) -> String {
        format!(
            r#"schema_version: 1
kind: feature-map
meta: {{ name: t, workload: w, game_revision: r, version: 1 }}
regions:
  - {{ name: wram, size: 131072 }}
features:
{}
"#,
            features_block
        )
    }

    #[test]
    fn rule_1_3_1_duplicate_name() {
        let yaml = minimal_map_yaml(
            "  - { name: x, region: wram, offset: 0, type: u8, semantics: counter, stability: stable }\n  - { name: x, region: wram, offset: 1, type: u8, semantics: counter, stability: stable }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/1"),
            "duplicate: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_3_1_undeclared_region() {
        let yaml = minimal_map_yaml(
            "  - { name: x, region: sram, offset: 0, type: u8, semantics: counter, stability: stable }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/1"),
            "undeclared region: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_3_1_offset_out_of_bounds() {
        let yaml = minimal_map_yaml(
            "  - { name: x, region: wram, offset: 131072, type: u8, semantics: counter, stability: stable }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/1"),
            "oob offset: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_3_2_bitflags_bad_discretize() {
        let yaml = minimal_map_yaml(
            "  - name: bf\n    region: wram\n    offset: 0\n    type: bitflags8\n    semantics: flags\n    stability: stable\n    discretize: { kind: bucket, size: 4 }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/2"),
            "bitflags: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_2_width_bytes_missing() {
        let yaml = minimal_map_yaml(
            "  - { name: x, region: wram, offset: 0, type: bytes, semantics: opaque, stability: stable }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.2/width"),
            "bytes no width: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_2_width_mismatch() {
        let yaml = minimal_map_yaml(
            "  - { name: x, region: wram, offset: 0, type: u16le, width: 4, semantics: counter, stability: stable }",
        );
        let (_, errors) = parse_feature_map(&yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.2/width-mismatch"),
            "width mismatch: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_3_7_grid_x_not_scalar() {
        let yaml = r#"schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions:
  - { name: wram, size: 131072 }
features:
  - name: bf
    region: wram
    offset: 0
    type: bitflags16le
    semantics: flags
    stability: stable
    discretize: { kind: bits }
  - name: py
    region: wram
    offset: 2
    type: u16le
    semantics: position_y
    stability: volatile
    discretize: { kind: none }
  - name: room
    region: wram
    offset: 4
    type: u8
    semantics: room_id
    stability: stable
  - name: carrier
    region: wram
    offset: 6
    type: u16le
    semantics: position_x
    stability: volatile
    discretize: { kind: grid, x: bf, y: py, room: room, cell_w: 32, cell_h: 32 }
"#;
        let (_, errors) = parse_feature_map(yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/7"),
            "grid x not scalar: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_3_7_grid_room_volatile() {
        let yaml = r#"schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions:
  - { name: wram, size: 131072 }
features:
  - name: px
    region: wram
    offset: 0
    type: u16le
    semantics: position_x
    stability: volatile
    discretize: { kind: grid, x: px, y: py, room: vroom, cell_w: 32, cell_h: 32 }
  - name: py
    region: wram
    offset: 2
    type: u16le
    semantics: position_y
    stability: volatile
    discretize: { kind: none }
  - name: vroom
    region: wram
    offset: 4
    type: u8
    semantics: room_id
    stability: volatile
"#;
        let (_, errors) = parse_feature_map(yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.3/7"),
            "grid room volatile: {:?}",
            errors
        );
    }

    #[test]
    fn rule_1_2_guard_volatile() {
        let yaml = r#"schema_version: 1
kind: feature-map
meta: { name: t, workload: w, game_revision: r, version: 1 }
regions:
  - { name: wram, size: 131072 }
features:
  - name: vmode
    region: wram
    offset: 0
    type: u8
    semantics: mode
    stability: volatile
  - name: x
    region: wram
    offset: 1
    type: u8
    semantics: counter
    stability: stable
    valid_when: { feature: vmode, op: eq, value: 1 }
"#;
        let (_, errors) = parse_feature_map(yaml).unwrap();
        assert!(
            errors.iter().any(|e| e.rule == "1.2/guard"),
            "guard volatile: {:?}",
            errors
        );
    }

    // ── Predicate grammar tests ───────────────────────────────────────────────

    #[test]
    fn nested_predicate_parses() {
        let yaml = r#"
schema_version: 1
kind: scoring-program
meta: { name: t, feature_map: t, version: 1 }
stages:
  monotone: true
  list:
    - name: s1
      points: 10
      when:
        all:
          - { feature: a, op: eq, value: 0 }
          - any:
              - { feature: b, op: ne, value: 1 }
              - not: { feature: c, op: gt, value: 2 }
goal:
  name: g
  predicate: { feature: a, op: eq, value: 0 }
"#;
        let sp: ScoringProgram = serde_yaml::from_str(yaml).expect("parse");
        match &sp.stages.list[0].when {
            Pred::All { all } => {
                assert_eq!(all.len(), 2);
            }
            _ => panic!("expected All"),
        }
    }

    #[test]
    fn bit_32_rejected() {
        let yaml = r#"
schema_version: 1
kind: scoring-program
meta: { name: t, feature_map: t, version: 1 }
stages:
  monotone: true
  list:
    - name: s1
      points: 10
      when: { feature: bf, op: bit_set, bit: 32 }
goal:
  name: g
  predicate: { feature: bf, op: bit_set, bit: 32 }
"#;
        let sp: ScoringProgram = serde_yaml::from_str(yaml).expect("parse");
        let errors = validate_scoring_standalone(&sp);
        assert!(
            errors.iter().any(|e| e.rule == "x/bit-width"),
            "bit 32 rejected: {:?}",
            errors
        );
    }

    #[test]
    fn schema_anyof_int_or_hex() {
        // Assert the generated schema's offset field has anyOf [integer, string pattern]
        // rather than adding a full jsonschema-validation dep.
        let schema_str = generate_schema();
        let v: serde_json::Value = serde_json::from_str(&schema_str).unwrap();

        // Find the IntOrHex definition
        let def = &v["definitions"]["IntOrHex"];
        let any_of = def["anyOf"].as_array().expect("anyOf array");
        assert_eq!(any_of.len(), 2, "anyOf should have 2 variants");

        let integer = any_of.iter().find(|s| s["type"] == "integer");
        let string_pat = any_of.iter().find(|s| s["type"] == "string");
        assert!(integer.is_some(), "integer variant missing");
        let sp = string_pat.expect("string variant missing");
        assert_eq!(sp["pattern"], "^0[xX][0-9a-fA-F]+$");
    }
}
