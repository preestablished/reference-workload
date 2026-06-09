# 03 — Demo YAML artifacts + negative fixtures

Depends on: 01 (parser/validator must exist).

## Positive artifacts (checked in at repo root, per README layout)

1. **`feature-maps/demo-game.yaml`** — copy the worked example from API.md
   §1.4 **verbatim** (schema_version 1, meta name `demo-game`, workload
   `refwork-demo`, `game_revision: "operator-set"`, region `wram` size
   131072, and the nine features: `room_id, area_id, player_x, player_y,
   health, upgrade_flags, boss_flags, game_mode, credits_flag` with exactly
   the §1.4 offsets/types/discretize blocks). These offsets are explicitly
   placeholders — the file header comment must say so and point at the
   `ramdiff`/`map-check` workflow (§1.5). Do not invent or "improve" values.

2. **`scoring/demo-game.yaml`** — copy API.md §2.1 verbatim: the seven stages
   (`left_start_area` 100 → `credits` 2000), `shaping` (incl. the
   `weight: 0` documentation-entry style), `penalties` (`dead` → `prune`,
   `game_mode eq 0x19`), `goal credits_reached` on `credits_flag ne 0`.

Both must pass:
```
cargo run -p refwork-featuremap -- validate feature-maps/demo-game.yaml
cargo run -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml
```

Consistency note (grounded): every feature referenced by §2.1 predicates
(`area_id, upgrade_flags, boss_flags, credits_flag, game_mode`) AND by its
shaping exprs (`upgrade_flags`, plus `room_id` in the `weight: 0` entry) is
declared `stability: stable` in §1.4; `player_x/player_y` (volatile) appear
only in discretize grids; the bit-op targets (`upgrade_flags`, `boss_flags`)
are `bitflags*` types, satisfying `x/bitop-target`. So the verbatim pair
validates cleanly under rules 3/4 and the `x/` cross-checks. If
implementation finds otherwise, the validator is wrong, not the docs.

## Negative fixtures (≥10; M0 acceptance names "bad offset,
volatile-in-predicate, etc.")

Location: `crates/refwork-featuremap/tests/fixtures/invalid/NN-<slug>.yaml`
(+ paired `NN-<slug>.scoring.yaml` for the cross-file cases, #12–#16). Each
fixture is the demo map minimally mutated — one rule violation per file. An
integration test (`crates/refwork-featuremap/tests/fixtures.rs`) drives the
sweep FROM THE MANIFEST (`fixtures/invalid/expected.json` mapping file →
expected rule id), asserting each entry FAILS with that rule id, and asserts
manifest↔directory **bijection** (every `*.yaml` except `*.scoring.yaml`
suffixes appears in the manifest and vice versa) so a stale fixture can never
silently stop being exercised. Cross-file entries name their paired scoring
file in the manifest; `*.scoring.yaml` files are never validated standalone.
Parse-time rejections (#10 `bad-schema-version`, wrong-`kind`) must still
surface as rule ids: parse into a permissive envelope (`schema_version` +
`kind` first), check those post-parse, and only then deserialize the full
model — the manifest then uses ordinary rule ids (`preamble/version`,
`preamble/kind`) rather than a special parse-error marker. The CLI
exit path is covered by running the binary in at least one test
(`assert_cmd`-style via `std::process::Command` on the built binary — no new
dev-deps needed; use `env!("CARGO_BIN_EXE_refwork-featuremap")`).

| # | Fixture | Violates |
|---|---|---|
| 01 | `offset-out-of-bounds` — `health` offset `0x1FFFF` (offset+2 > 131072) | §1.3/1 |
| 02 | `duplicate-name` — two `room_id` features | §1.3/1 |
| 03 | `undeclared-region` — feature with `region: sram` not in `regions` | §1.3/1 |
| 04 | `bitflags-bad-discretize` — `upgrade_flags` with `discretize: {kind: bucket, size: 4}` | §1.3/2 |
| 05 | `bytes-missing-width` — a `type: bytes` feature without `width` | §1.2 width rule |
| 06 | `width-mismatch` — `health` (`u16le`) with `width: 4` | §1.2 width rule |
| 07 | `grid-x-not-scalar` — grid whose `x` names a `bitflags16le` feature | §1.3/7 |
| 08 | `grid-room-volatile` — grid whose `room` names a volatile feature | §1.3/7 |
| 09 | `grid-double-count` — grid `y` feature also carrying `{kind: identity}` | §1.3/7 |
| 10 | `bad-schema-version` — `schema_version: 2` | preamble |
| 11 | `valid-when-volatile` — `valid_when.feature: player_x` (volatile) | §1.2 guard rule |
| 12 | `scoring-volatile-in-predicate` — scoring stage `when` on `player_x` (map valid, scoring invalid ⇒ pair fails) | §1.3/3 (“volatile-in-predicate”, named in M0 accept) |
| 13 | `scoring-bytes-in-predicate` — map gains a valid `bytes` feature; scoring goal references it | §1.3/4 |
| 14 | `scoring-unknown-feature` — stage `when` on `warp_progress` (not in map) | `x/feature-exists` |
| 15 | `scoring-bit-out-of-range` — `bit_set` with `bit: 9` on `bitflags8` | `x/bit-width` |
| 16 | `scoring-bitop-on-non-bitflags` — `bit_set` with `bit: 3` on `game_mode` (`u8`) | `x/bitop-target` (state-scorer §4 alignment) |

(16 listed so the 10-minimum survives any cut during review; #12–#16 ship a
paired `*.scoring.yaml`. "Violates" ids in the `x/` namespace are
implementation-defined cross-checks per plan 01 — not literal spec clauses.)

## Also

- Add `feature-maps/` and `scoring/` to the repo (README "Repository layout
  (target)" already names them).
- `xtask/asm`, `schema/`, `feature-maps/`, `scoring/` contain no game content
  — re-run `cargo xtask deny` is unaffected (host-side files, not scanned),
  but the **clean-room check** applies: no commercial names in comments.
