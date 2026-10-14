# 02 — Validator Variant Split (WP 2)

Target: `xtask/src/image.rs`. Public entry point `validate_manifest(&Path)`
(line 264) keeps its exact signature `Result<(), ImageError>` and its
`ImageError::Validation(Vec<String>)` error shape (seven callers, F16).
`cmd_image_validate` in `xtask/src/main.rs` (lines 220–233) needs no change
except the optional success-line variant suffix (D11).

## Goal and prerequisite state

Goal: `validate_manifest` applies the §4.2 table. A `generic` bundle that
matches the table passes. A `game` bundle is checked exactly as today. Error
strings for existing game checks keep their current substrings so the
existing negative tests (`00-overview.md` F9) stay valid.

Prerequisite: WP 1 complete (gate G1). The implementer codes from the §4.2
table in `01-owner-doc-amendment.md`.

## Repository evidence and patterns

- Checks are free functions taking `root: Option<&Mapping>` and
  `errors: &mut Vec<String>`; helpers `child_map`, `seq_field`,
  `string_field`, `u64_field`, `expect_string`, `expect_u64`, `expect_bool`,
  `has_key` (lines 1794–1930) push one error per defect and return `None`
  on a missing field so later checks continue.
- `validate_machine` (1227–1266) uses `expect_device` with `DeviceSpec`
  (1268–1317). `expect_device` pushes `machine.devices missing <kind>` when
  absent and `machine.devices.<kind>.<field>` on value drift.
- `validate_handoff_files` (1492–1535) checks file presence, the sidecar
  pair, then parses the three TOML files with string-scanning helpers
  (`table_block`, `region_block`, `region_array_block`, `field_*`,
  1965–2007). There is no TOML crate dependency; keep that.
- `validate_green_stamp_sidecar` (1537–1638) pins
  `workload_image` to `{WORKLOAD_NAME}@{VERSION}`.
- Constants `WORKLOAD_NAME`, `VERSION`, `FPS_NUM`, `FPS_DEN`,
  `REQUIRED_REGIONS`, `PAD_BUTTONS`, `PAD_LAYOUT_ID` (lines 10–55).

## Exact change surface

All symbols below are `proposed` unless marked existing. Insert new items
next to the existing function they refine. Names are recommendations; keep
them private to the module except where noted.

### Variant resolution

- `pub enum ManifestVariant { Game, Generic }` (`proposed`, near
  `DeviceSpec`), with
  `fn resolve_variant(root: Option<&Mapping>, errors) -> Option<ManifestVariant>`
  (`proposed`): `root == None` (document is not a mapping; `mapping()` has
  already pushed its error) → `Some(Game)` so the existing error list for
  that input is unchanged; key absent → `Some(Game)`; `"game"` →
  `Some(Game)`; `"generic"` → `Some(Generic)`; `null`, empty string, any
  other string, or any non-string → push exactly one error
  `variant must be game or generic, got <yaml-debug-of-value>` and return
  `None`. `validate_manifest` returns `Err(Validation(errors))` immediately
  on `None`, so an unknown variant yields exactly one error and no cascade.
- `pub fn manifest_variant(manifest: &Path) -> Result<ManifestVariant, ImageError>`
  (`proposed`, beside `validate_manifest`): parses the file and resolves the
  variant only. Used by the CLI suffix (D11). `validate_manifest`'s return
  type does not change.

### `validate_manifest` (existing, line 264) — new order

```text
read, parse, hash                     (unchanged)
root = mapping(...)
variant = resolve_variant(root, errors)?  # None → return Err with the single variant error
validate_schema_common(root, variant, errors)   # proposed: schema_version/kind for both; generic meta rules
if Game { validate_game_meta(root, errors) }    # proposed: the meta pins, at their CURRENT position (before artifacts)
validate_artifacts(base, root, errors)          # existing, unchanged
validate_boot(root, errors)                     # existing, unchanged
validate_machine(root, variant, errors)         # existing, now takes variant
validate_regions(root, variant, errors)         # existing, now takes variant
validate_fps(root, variant, errors)             # existing, now takes variant
validate_pad_layout(root, errors)               # existing, unchanged
validate_defaults(root, variant, errors)        # existing, now takes variant
validate_determinism_block(root, variant, errors) # proposed: generic only (see below)
match variant {
  Game    => validate_handoff_files(base, root, hash, errors)          # existing, unchanged
  Generic => validate_generic_handoff_files(base, root, hash, errors)  # proposed
}
validate_no_game_content(base, errors)          # existing, unchanged
```

### `validate_schema_common(root, variant, errors)` (`proposed`)

Checks that today live inline in `validate_manifest` lines 283–286:
`schema_version == 1`, `kind == workload-image`. For `Game`, nothing else
(the game pins move to `validate_game_meta`; game behavior is unchanged).
For `Generic` add: `meta.name` matches `^[a-z0-9][a-z0-9-]*$` (error
`meta.name must match ...`); `meta.version` non-empty with no `@` or
whitespace (error `meta.version`); `meta.built_from` present with non-empty
`repo`, 40-lowercase-hex `git_rev`, 40-lowercase-hex `guest_sdk_rev` (error
paths `meta.built_from.<key>`). Implement the hex check with a small
`is_git_rev_hex` next to `is_blake3_hex` (line 1957).

Note: the existing test `missing_built_from_is_reported_once_even_when_stamped`
(line 2372) asserts the `meta.built_from` error is reported once. Keep a
single `child_map(..., "meta.built_from", ...)` call per validation run;
pass the resolved mapping to the game-layer function instead of re-resolving.

### `validate_game_meta` (`proposed`)

Exactly the pins moved out of `validate_manifest` lines 287–304, called at
the same point in the sequence they occupy today (immediately after
`schema_version`/`kind`, before `validate_artifacts`) so the game error
**order** is unchanged as well as its content: `meta.name ==
WORKLOAD_NAME`, `meta.version == VERSION`, `meta.built_from.emu_version`
non-empty. Error paths unchanged (`meta.name`, `meta.version`,
`meta.built_from.emu_version`).

### `validate_machine(root, variant, errors)` (existing)

- Both: `vcpus == 1`.
- `Game`: `mem_mib == 128`; the three `expect_device` calls unchanged
  (extra kinds stay unrejected, as today).
- `Generic`: `mem_mib` is an integer ≥ 1 (error `machine.mem_mib must be an
  integer >= 1`); `expect_device` for `detguest-channel` and `pv-pad`
  unchanged; then walk the list once more and push
  `machine.devices unsupported kind <k> for variant generic` for every
  entry whose kind is not one of the two, and
  `machine.devices duplicate kind <k>` for any kind seen twice.

### `validate_regions(root, variant, errors)` (existing)

- `Game`: the existing `REQUIRED_REGIONS` loop unchanged, nothing added.
- `Generic`: `regions` is a non-empty sequence (error `regions must be a
  non-empty list`); each entry is a mapping with a non-empty `name` and
  integer `size`; names unique (error `regions duplicate name <n>`);
  `size % 4096 == 0`, or `size == 0` only when `optional: true` (error
  `regions.<n>.size must be a multiple of 4096`); `optional`, if present,
  is a boolean; no entry carries `layout_version` (reuse the existing error
  text); if an entry named `framebuffer` exists it must have a non-empty
  `format` (error `regions.framebuffer.format`).

### `validate_fps(root, variant, errors)` (existing)

- `Game`: unchanged pinned values.
- `Generic`: `num` and `den` are integers ≥ 1. Exact error texts:
  `fps.num must be an integer >= 1` and `fps.den must be an integer >= 1`
  (a float, a string, or zero all produce them; `u64_field` returns `None`
  for non-integers, so check `Some(v) if v >= 1`).

### `validate_defaults(root, variant, errors)` (existing)

- `Game`: unchanged.
- `Generic`: if `defaults` is absent, no error. If present, `feature_map`
  and `scoring_program` must be non-empty strings containing one `@`.

### `validate_determinism_block(root, variant, errors)` (`proposed`)

`Generic` only: `determinism` mapping present; `suite` non-empty string.
Error path `determinism.suite`. For `Game` the function returns without
checking (today the game path does not inspect `determinism` in the
manifest, and this plan does not change game behavior).

### `validate_generic_handoff_files` (`proposed`)

Mirror of `validate_handoff_files` for the generic bundle (§4.4):

1. Required files: `boot.toml`, `expected-regions.toml`, `README.md`.
   Error `missing dist handoff file <rel>` (same text as game).
2. Sidecar pair: identical logic to lines 1508–1520. On an unstamped
   sidecar, parse it as YAML and check `kind == determinism-unstamped`,
   `workload_image == <meta.name>@<meta.version>`, and `git_rev ==
   meta.built_from.git_rev` (error paths `determinism.unstamped.<key>`).
   On a green stamp call the shared stamp validator with generic
   expectations (below). Read `meta.built_from.git_rev` through the
   non-pushing accessor `manifest_built_from` (line 1932), not
   `manifest_git_rev` (line 1938), so a missing revision is reported once
   by `validate_schema_common` and not again here.
3. `boot.toml`: `boot_toml_version == 1`; `[autostart]` present;
   `[[unit]]` present with non-empty `exec`; collect every
   `[[expected_region]]` block as `(name, layout_version)`; require at
   least one; require each name to be in the manifest region names. Error
   texts: `boot.toml boot_toml_version must be 1`, `boot.toml missing
   [autostart]`, `boot.toml missing [[unit]]`, `boot.toml unit.exec must be
   non-empty`, `boot.toml must declare at least one [[expected_region]]`,
   `boot.toml expected_region <n> is not declared in regions`.
4. `expected-regions.toml`: `schema_version == 1`; for each `[[regions]]`
   block, `name` in manifest regions, `size` equal to the manifest size, and
   when `boot.toml` has an `[[expected_region]]` of the same name, equal
   `layout_version`. Error texts: `expected-regions.toml <n> is not declared
   in regions`, `expected-regions.toml <n> size <a> != manifest size <b>`,
   `expected-regions.toml <n> layout_version <a> != boot.toml <b>`.

The existing string-scanning helpers find blocks by exact header text.
`region_array_block(content, "expected_region", name)` locates one named
block; to enumerate all blocks, add a small helper
`fn array_blocks(content: &str, array: &str) -> Vec<(String, Option<u64>)>`
(`proposed`, next to `region_array_block` at line 1970) that scans for
`[[<array>]]` headers and returns each block's `name` and `layout_version`.
The generic path first strips comment lines and trailing `#` comments
(`fn strip_toml_comments(content: &str) -> String`, `proposed`, same
place) before any block scan, and treats a block as ending at the next line
that starts with `[`, so a comment containing `[autostart]` or a table
following an `[[expected_region]]` entry cannot be misattributed. The game
path keeps using the helpers exactly as today.

### `validate_green_stamp_sidecar_generic` (`proposed`)

Same as `validate_green_stamp_sidecar` (1537–1638) except:

- `workload_image` must equal `format!("{name}@{version}")` from the
  manifest's `meta` (read via `string_field`; if absent, the schema layer
  already reported it, so skip the comparison).
- revision key is `git_rev`, compared to `meta.built_from.git_rev`
  (error path `determinism.last_green.git_rev`).
- no `emu_version` requirement.
- `image_manifest_hash`, `suite_version`, `timestamp`, `suite_report_blake3`
  rules unchanged.

Prefer one shared private function with a small `StampExpectations` struct
(`proposed`) carrying `workload_image`, `rev_key`, `rev_value`,
`emu_version: Option<String>` over two copies, so the two variants cannot
drift on the shared keys. The existing game-path tests at lines 2414–2470
and 2755–2776 must keep passing with the same error substrings.

### CLI (optional, recommended)

`cmd_image_validate` success line becomes
`image validate: OK - <path> (variant <game|generic>)`, obtained by calling
`manifest_variant` after `validate_manifest` succeeds. `validate_manifest`'s
signature does not change (D11).

## Intended behavior, invariants, error paths

- Invariant: a manifest without `variant`, or with `variant: game`,
  produces the same error list as before this change for every input. No
  new rule applies to `Game`; the characterization test in
  `03-fixture-and-tests.md` proves it.
- Invariant: `image register` is not modified; it now accepts a valid
  generic bundle because it calls `validate_manifest`. That is stated in
  `00-overview.md` Non-goals and is not tested here.
- Invariant: unknown top-level keys are ignored (API.md forward-compat
  rule). Do not add a deny-list.
- Invariant: the generic layer never reads `harness.toml`.
- Error aggregation stays "collect all, fail once" as today.
- `build_image`, `write_workload_manifest`, `write_unstamped_sidecar`,
  `double_build`, `register_image` are not modified beyond the optional
  return-type ripple above.

## Tests and acceptance

Tests are specified in `03-fixture-and-tests.md`. Acceptance for this
package:

1. `cargo test --locked -p xtask` passes, including every pre-existing test
   in `xtask/src/image.rs`.
2. `cargo fmt --all -- --check` and
   `cargo clippy --workspace --all-targets -- -D warnings` pass. CI runs both
   (`.github/workflows/ci.yaml` lines 38–40).
3. The characterization test for game error vectors (WP 3) passes.

## Dependencies, risks, edge cases, exclusions

- Depends on WP 1. WP 3 depends on this.
- Edge: `variant: Game` (capitalized) is invalid; do not case-fold.
- Edge: `variant: generic` with `regions` containing `wram` is valid; the
  generic layer does not forbid game region names.
- Edge: `mem_mib: 0` is invalid for generic.
- Edge: a generic bundle that also contains `harness.toml` is valid; the
  file is ignored.
- Exclusion: no serde struct for the manifest; keep `serde_yaml::Value`
  scanning so unknown fields stay ignorable without `deny_unknown_fields`
  decisions.
