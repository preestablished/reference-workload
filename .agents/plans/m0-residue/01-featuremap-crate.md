# 01 — `refwork-featuremap`: types, validator, CLI, JSON Schema

**Replaces** the stub at `crates/refwork-featuremap/src/lib.rs` (the current
`Feature {name, region, offset, width}` struct and its `validate()` are
placeholders — delete them; nothing else in the workspace depends on them
except its own unit test).

## Deliverables

1. Serde data model for the **feature map** (API.md §1.1–§1.2) and the
   **scoring program** (API.md §2.1–§2.3) — this crate owns both surfaces
   (README: "This repo owns the scoring-DSL surface").
2. Validator implementing API.md §1.3 rules 1–7 plus the §2 cross-file rules.
3. A `refwork-featuremap` **binary** (same crate, `src/main.rs`) with:
   - `validate <map.yaml> [--scoring <scoring.yaml>]` — exit 0/1 with
     file:line-ish diagnostics (serde_yaml location where available).
   - `schema` — print the feature-map JSON Schema to stdout (used to generate
     the committed `schema/feature-map.schema.json`).
4. `schema/feature-map.schema.json` committed at the repo root (README layout).

## Data model (field-for-field from API.md — do not improvise)

Top level (§1.1): `schema_version: u32` (reject major ≠ 1), `kind:
"feature-map"` (fixed string, reject others), `meta { name, workload,
game_revision, version: u32, authors: Option<Vec<String>> }`,
`regions: Vec<RegionDecl { name, size: u64 }>`, `features: Vec<Feature>`.

`Feature` (§1.2):
- `name` — `[a-z0-9_]+`, unique within the map.
- `region` — must appear in `regions`.
- `offset` — int **or hex string** (`0x...`): implement a serde helper that
  accepts both YAML integer and string forms.
- `type` — enum, exact spelling: `u8 u16le u16be u32le u32be i8 i16le i16be
  i32le i32be bitflags8 bitflags16le bitflags32le bcd8 bcd16le bytes`.
- `width` — required iff `type: bytes`; otherwise optional but MUST equal the
  type-derived width if present (derived widths: 1/2/4 by type suffix;
  `bcd16le` = 2 etc.).
- `semantics` — **open enum** (unknown values parse as `opaque`; do not
  reject): `counter position_x position_y room_id health resource flags mode
  progress_flag timer opaque`.
- `description` — optional string.
- `stability` — `stable | volatile`, required.
- `discretize` — optional, default `none`. Kinds: `identity`, `bucket {size}`,
  `bits`, `threshold {edges: Vec<i64>}`, `grid {x, y, room, cell_w, cell_h}`,
  `none`.
- `valid_when` — optional `{ feature, op, value }` guard; op/value grammar per
  §2.3 leaf; the referenced feature must exist and be `stability: stable`
  (§1.2 text: "another (stable) feature").

Scoring program (§2.1):
- `schema_version: u32`, `kind: "scoring-program"`, `meta { name, feature_map,
  version }`.
- `stages { monotone: bool, list: Vec<Stage { name, points: i64, when: Pred }> }`.
- `shaping: Option<Vec<Shaping { name, weight: i64, expr: { feature, reduce } }>>`
  with `reduce: identity | popcount` (§2.3).
- `penalties: Option<Vec<Penalty { name, when: Pred, action: "prune" }>>`.
- `goal { name, predicate: Pred }`.
- `Pred` grammar (§2.3, recursive): leaf `{feature, op: eq|ne|lt|le|gt|ge,
  value: i64}` | `{feature, op: bit_set|bit_clear, bit: u8 (0..=31)}` |
  `{all: [Pred]}` | `{any: [Pred]}` | `{not: Pred}`. Integers decimal or `0x`
  hex (same serde helper as `offset`). **No floats anywhere in the model.**

Forward compatibility (API.md preamble, normative): unknown **optional** fields
are ignored (serde default behavior — do NOT use `deny_unknown_fields`
globally); unknown `schema_version` major and wrong `kind` are hard errors.

## Validation rules (§1.3, numbered for diagnostics)

Map-local:
1. Feature names unique; every `feature.region` declared in `regions`;
   `offset + width(type) <= region.size`.
2. `bitflags*` features only with `discretize.kind: bits` or `none`.
6. Endianness explicit — enforced by the closed `type` enum (no naked `u16`).
7. `grid`: `x`/`y`/`room` name features in the same map (`x` may be the
   carrier); `x`/`y` integer scalar types only (the ten `u*/i*` types — not
   bitflags/bcd/bytes); `room` must be `stability: stable`; `cell_w`/`cell_h`
   ≥ 1; features referenced as `x`/`y` must not carry their own non-`none`
   discretize other than the grid itself.
   Also (§1.2): `valid_when.feature` exists and is stable.

Cross-file (needs both documents; CLI `--scoring`):
3. Every feature referenced by any stage/penalty/goal predicate, shaping expr,
   or `valid_when` must be `stability: stable` — **except** `valid_when` is
   already map-local; predicates/shaping come from the scoring program.
4. `bytes` features excluded from all predicates and shaping exprs.
   Plus: every scoring-referenced feature must exist in the map;
   `bit_set/bit_clear.bit` must fit the feature width (bit ≤ 7 for
   8-bit types, ≤ 15 for 16-bit, ≤ 31 for 32-bit);
   `scoring.meta.feature_map == map.meta.name`.

(Rule 5 — region published by the workload — is a scorer-load-time check, not
this validator's; note that in a doc comment.)

Error type: a `ValidationError { rule: &'static str, path: String, msg: String }`
list (collect all errors, don't stop at the first) so fixtures can assert on
`rule` identifiers like `"1.3/1"`, `"1.3/7"`, `"2/stable-only"`.

## JSON Schema generation

- Add `schemars` derive alongside serde on the feature-map types only (the
  committed schema file covers the feature map; the scoring program's
  normative JSON Schema is owned by state-scorer per §2 preamble — do NOT
  emit one here).
- `refwork-featuremap schema` prints `schemars::schema_for!(FeatureMap)`
  pretty-printed with **stable key order** (serde_json `preserve_order` off,
  BTreeMap-backed — output must be byte-deterministic for the CI drift gate).
- Commit the output as `schema/feature-map.schema.json`.

## Dependencies

`serde` (derive), `serde_yaml`, `serde_json`, `schemars`. Pin normally in the
crate's Cargo.toml. Note: `serde_yaml` is archived upstream but is the MAP.md-
named convention; pin `0.9.x` and record the (known, accepted) deprecation in
a Cargo.toml comment rather than switching to a fork unilaterally.

## Tests (crate-local; fixtures are package 03)

- Round-trip: parse §1.4's worked example verbatim → re-serialize → parse →
  equal models.
- Offset helper: `0x0AF6` string and `2806` int parse equal.
- Open-enum semantics: unknown `semantics: warp_gate` parses as `opaque`.
- Unknown optional field ignored; unknown `kind` rejected; `schema_version: 2`
  rejected.
- One unit test per §1.3 rule (the YAML fixtures in 03 cover the CLI path;
  these cover the library path).
- Predicate grammar: nested `{all/any/not}` parse; `bit: 32` rejected.
