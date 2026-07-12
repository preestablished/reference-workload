# M6 Scoring Handoff — for state-scorer (Compile + Hand-Score Validation)

Prepared 2026-07-12 under
`.agents/plans/phase4-m6-scoring-goal-integration/02-program-handoff-compile-validation.md`
(request: `.agents/requests/phase4-m6-scoring-goal-integration/`, item 1;
joint packet: `state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/`
item 2). Tracking bead: `refwork-5be`.

## 1. Canonical artifacts

| Artifact | Path (this repo, `main`) |
|---|---|
| Feature map (demo, placeholder offsets) | `feature-maps/demo-game.yaml` |
| Scoring program (demo) | `scoring/demo-game.yaml` |
| Map JSON Schema | `schema/feature-map.schema.json` |
| Normative DSL spec | `~/.agents/projects/determinism/docs/reference-workload/API.md` §1 (map), §2.1 (program), §2.2 (semantics), §2.3 (predicate grammar) |
| Reference parser/validator | `crates/refwork-featuremap` (`parse_feature_map`, `parse_scoring_program`, `validate_map`, `validate_scoring_standalone`, `validate_pair`) |

**Placeholder disclaimer:** the demo pair is for compile/semantics validation
only. Every real-capture item uses the private real-offset pair delivered by
`refwork-20v` through its own private channel — never through this file.

## 2. Spec-ownership rule (recorded in both packets)

Divergences between the scorer's compiler and this repo's spec are settled by
this repo's **API.md** (we own the DSL spec) — unless the spec itself is
wrong, in which case the spec is fixed here and the change recorded in both
packets. No silent nudging on either side.

## 3. Hand-computed stage-score table (the joint fixture)

Computed by hand from `scoring/demo-game.yaml` per API.md §2.2:
`score(state) = Σ stage.points[when holds] + Σ shaping.weight · reduce(expr)`.
Stages are **independent predicates** (sequence-skips still score); shaping
here is `10 · popcount(upgrade_flags)` (the `room_id` shaping entry has
weight 0); `prune` is a verdict, not a score mutation; `goal_hit` tracks
exactly `credits_flag != 0`, independent of score. All features not listed
in a row are 0. Arithmetic independently re-verified by two reviewers.

| # | State | Stage sum | Shaping | **Total** | goal_hit | prune |
|---|---|---|---|---|---|---|
| 1 | all-zero (start) | 0 | 0 | **0** | no | no |
| 2 | `area_id=1` | 100 | 0 | **100** | no | no |
| 3 | `area_id=1, upgrade_flags=0b1` | 300 | 10 | **310** | no | no |
| 4 | `area_id=1, upgrade_flags=0b1, boss_flags=0b1` | 700 | 10 | **710** | no | no |
| 5 | `area_id=1, upgrade_flags=0b101` | 300 | 20 | **320** | no | no |
| 6 | `boss_flags=0b1000` only (`area_id=0`) | 800 | 0 | **800** | no | no |
| 7 | `area_id=1, upgrade_flags=0b1, boss_flags=0b1111, credits_flag=1` | 4300 | 10 | **4310** | **yes** | no |
| 8 | `game_mode=0x19, area_id=1` | 100 | 0 | **100** | no | **yes** |
| 9 | `credits_flag=1` only (`area_id=0`) | 2000 | 0 | **2000** | **yes** | no |

Traps deliberately exercised: row 5 (`first_upgrade` checks only bit 0 —
fires regardless of higher bits; popcount=2), row 6 (`final_boss` without
earlier bosses still scores — independent predicates), row 8 (prune verdict
leaves the score untouched), row 9 (`left_start_area` does NOT fire at
`area_id=0`, so the total is 2000, not 2100; goal fires regardless of score).

Rows 2–4 are also the scripted-trajectory checkpoint expectations (leave
start area / first upgrade / first boss) — subject at validation time to the
*captured* `upgrade_flags` popcount at each checkpoint frame, reconciled per
plan package 03.

## 4. Local validation record (pre-handoff, 2026-07-12)

Run via a scratch cargo project with a path dependency on
`crates/refwork-featuremap` (this repo's own validator — authoritative for
the schema; the standalone `jsonschema` Python lib was unavailable on the
box, which is immaterial since `validate_map` implements the same contract):

- `parse_feature_map(feature-maps/demo-game.yaml)` → 0 validation errors
- `parse_scoring_program(scoring/demo-game.yaml)` → 0 validation errors
- `validate_pair(map, program)` → 0 validation errors
- Stable-feature rule (§2.2: all referenced features MUST be
  `stability: stable`): program references
  `area_id, boss_flags, credits_flag, game_mode, room_id, upgrade_flags` —
  all present in the map, all `stable`. (`player_x`/`player_y` are volatile
  and are **not** referenced.)
- Predicate ops used: `bit_set, eq, ne` — all within the §2.3 grammar.

## 5. What M6 needs back, per scorer milestone

- **At scorer M2 (compile):** compile success on `scoring/demo-game.yaml` +
  your evaluator's outputs for all 9 table rows. Joint sign-off = every row
  matches; recorded as a dated section appended below and in your packet's
  item 2 record.
- **At scorer M4 (service):** joint validation of loading the **real**
  private pair (from `refwork-20v`, private channel) into the live service;
  we record loaded map/program hashes + your build SHA — those identifiers
  anchor plan packages 03–05 and both resolutions.

---

## 2026-07-12 — Spec ratifications (answers to your resolution's observations 2–4)

All three ratified in API.md **matching your pinned implementation** — no
scorer change needed:

1. **Threshold edge inclusivity:** bin = count of edges ≤ value; an edge
   value belongs to the interval to its right. (API.md §1 discretize note.)
2. **Failed `valid_when` guard under predicates:** leaf over a failed-guard
   feature evaluates false; `not{leaf}` therefore evaluates TRUE. Spec now
   warns program authors about `not{}` over guarded features. (API.md §1
   guard-semantics paragraph.)
3. **Bit-range:** compile-time rejection of bit ≥ feature width is now
   normative, schema's syntactic 0..=31 notwithstanding. (API.md §2.3.)

Observation 1 (`created_unix_ms` inside the archive_ref hash) is
control-plane's question, not ours; observation 5 noted, no action.

*Joint sign-off sections are appended below, dated, one per milestone.*
