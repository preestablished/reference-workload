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
   - `validate <map.yaml> [--scoring <scoring.yaml>]` — exit 0/1. Diagnostics
     carry serde_yaml locations where available; NOTE the honest limit:
     errors inside the untagged `Pred` / internally-tagged `Discretize`
     grammars degrade to variant-mismatch messages without locations
     (serde_yaml buffers via `Content`). Wrap those parse sites with
     feature-name/stage-name context in the error path; a full post-parse
     `serde_yaml::Value` diagnostic pass is a non-goal for M0.
   - `schema` — print the feature-map JSON Schema to stdout (used to generate
     the committed `schema/feature-map.schema.json`).
4. `schema/feature-map.schema.json` committed at the repo root (README layout).

## Data model (field-for-field from API.md — do not improvise)

Top level (§1.1): `schema_version: u32` (reject major ≠ 1), `kind:
"feature-map"` (fixed string, reject others), `meta { name, workload,
game_revision, version: u32, authors: Option<Vec<String>> }`,
`regions: Vec<RegionDecl { name, size: u64 }>`, `features: Vec<Feature>`.

`Feature` (§1.2):
- `name` — validated as `^[a-z][a-z0-9_]{0,63}$` (the strict intersection of
  refwork §1.2 `[a-z0-9_]+` and state-scorer §3/§4's `^[a-z][a-z0-9_]*$` +
  64-char cap, so a refwork-valid map can never fail scorer load on naming);
  unique within the map. Doc-comment the intersection (overview §doc-recon).
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
- `stages { monotone: bool (serde default TRUE — state-scorer's schema makes
  it optional with default true; a program legally omits it), list:
  Vec<Stage { name, points, when: Pred, requires: Option<Vec<String>> }> }`.
  Validate: `monotone: false` rejected ("reserved, rejected in v1" per
  state-scorer §4); `points >= 0` (scorer schema `minimum: 0`); `requires`
  ≤ 8 entries, each naming an EARLIER stage in `list` (scorer compiler rule).
  `requires` is absent from refwork §2 — see overview §doc-recon item 1.
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
Record in a doc comment that this IS our interpretation of the preamble's
"reject unknown *required-context* fields" clause: serde cannot distinguish
unknown-optional from unknown-required-context without a convention the spec
does not define, so version-major + `kind` gating carries that duty.

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
   (Shaping-as-hard-error is the strict reading of §2.2's "All features
   referenced MUST be stable"; state-scorer treats shaping as a warning —
   overview §doc-recon item 2; doc-comment the choice.)
4. `bytes` features excluded from all predicates and shaping exprs.

Implementation-defined cross-checks (not literal §-numbered spec rules; give
them their own rule-id namespace `x/...` so fixtures and diagnostics never
cite nonexistent spec clauses):
- `x/feature-exists` — every scoring-referenced feature exists in the map.
- `x/bitop-target` — `bit_set`/`bit_clear` and `reduce: popcount` only on
  `bitflags8|bitflags16le|bitflags32le` features (state-scorer §4: these
  "require a bitflags* field" — without this, a refwork-valid pair fails at
  scorer load, defeating the pre-gate).
- `x/bit-width` — `bit` fits the bitflags width (≤7 / ≤15 / ≤31), within the
  §2.3 global `0..=31` bound.
- `x/featmap-ref` — `scoring.meta.feature_map == map.meta.name`.
- `x/requires` — stage `requires` entries name earlier stages (≤ 8).

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
- **Int-or-hex fields need hand-written schemas:** `#[derive(JsonSchema)]`
  ignores `deserialize_with`, so a plain derive would publish a schema that
  REJECTS `offset: "0x0AF6"` while our validator accepts it. Model int-or-hex
  as a newtype (used by `offset` and `valid_when.value`) with a manual
  `JsonSchema` impl / `#[schemars(schema_with = ...)]` emitting
  `anyOf: [ {type: integer}, {type: string, pattern: "^0[xX][0-9a-fA-F]+$"} ]`.
  Note: `valid_when` drags the §2.3 leaf-predicate type into the feature-map
  schema, so that shared type needs `JsonSchema` even though scoring-program
  types as a whole are excluded.
- `refwork-featuremap schema` prints `schemars::schema_for!(FeatureMap)`
  pretty-printed with **stable key order** (serde_json `preserve_order` off,
  BTreeMap-backed — output must be byte-deterministic for the CI drift gate)
  and a trailing newline (pin the convention; `diff -u` footgun).
- Commit the output as `schema/feature-map.schema.json`.
- Test: validate both §1.4 offset forms (int and `0x` string) against the
  GENERATED schema with a JSON-Schema checker in tests (`jsonschema` dev-dep
  or a targeted assertion on the emitted `anyOf` node — prefer the latter to
  keep the dep tree small).

## Dependencies

`serde` (derive), `serde_yaml`, `serde_json`, `schemars`. Pin normally in the
crate's Cargo.toml. Note: `serde_yaml` is archived upstream but is the
convention named by the spec README's "Conventions honored (MAP.md)" section
(MAP.md itself doesn't name it); pin `0.9.x` and record the (known, accepted)
deprecation in a Cargo.toml comment rather than switching to a fork
unilaterally. Pin `schemars`/`serde_json` minor versions — the committed
schema's byte-stability is a CI gate (see 04).

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
