# Package 02 — Program/Map Handoff + Compile Validation (Item 1)

Two halves. The **pre-gate half is startable now** (the request marks item 1
"startable at scorer M2, before the full gate" — and preparing the handoff
needs nothing from anyone). The **joint half** fires when scorer M2 closes,
and extends at scorer M4.

## Pre-gate half (do now)

### 2.1 Handoff package

Write `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md` containing:

1. Pointers to the canonical artifacts: `feature-maps/demo-game.yaml`,
   `scoring/demo-game.yaml`, `schema/feature-map.schema.json`, and the
   normative spec (`~/.agents/projects/determinism/docs/reference-workload/API.md`
   §1 map, §2 scoring DSL, §2.2 semantics, §2.3 predicate grammar).
2. The spec-ownership rule verbatim: divergences between the scorer's compiler
   and this repo's spec are settled by API.md (this repo owns the DSL spec)
   unless the spec itself is wrong — then the spec is fixed here and the
   change recorded in both packets.
3. The hand-computed stage-score table (§2.2 below).
4. What M6 will need back at each scorer milestone: at M2, compile success +
   their computed scores for the table's states; at M4, joint validation of
   loading the **real** private pair (delivered privately by `refwork-20v`,
   never through this file) into the live service.
5. The placeholder disclaimer: the demo pair is for compile/semantics
   validation only; every real-capture item uses the 20v pair.

### 2.2 Hand-score table (the joint fixture)

Compute by hand from `scoring/demo-game.yaml` + API.md §2.2
(`score = Σ stage.points[when holds] + Σ shaping.weight · reduce(expr)`).
Table rows are feature-vector states, not frames, so both sides can evaluate
without any capture data. Include at minimum:

| State (features set, all others 0) | Stage sum | Shaping | Total | goal_hit | prune |
|---|---|---|---|---|---|
| all-zero (start) | 0 | 0 | 0 | no | no |
| `area_id=1` | 100 | 0 | 100 | no | no |
| `area_id=1, upgrade_flags=0b1` | 300 | 10·popcount(1)=10 | 310 | no | no |
| `area_id=1, upgrade_flags=0b1, boss_flags=0b1` | 700 | 10 | 710 | no | no |
| `area_id=1, upgrade_flags=0b101` (skip: bit2 w/o bit1) | 300 | 20 | 320 | no | no |
| `boss_flags=0b1000` only (sequence-skip; area_id=0) | 800 | 0 | 800 | no | no |
| `area_id=1, upgrade_flags=0b1, boss_flags=0b1111, credits_flag=1` | 100+200+400+400+400+800+2000=4300 | 10 | 4310 | **yes** | no |
| `game_mode=0x19` (dead), `area_id=1` | 100 | 0 | 100 | no | **yes** |
| `credits_flag=1` only | 2000+100? — **no**: area_id=0 ⇒ 2000 | 0 | 2000 | **yes** | no |

Recompute every row yourself when authoring the handoff — the table above is
the planner's arithmetic and the whole point is two independent computations
agreeing. Key semantics the rows must exercise: stages are independent
predicates (sequence-skips still score), shaping is additive popcount,
`prune` is a verdict not a score mutation, `goal_hit` tracks exactly
`credits_flag != 0` (and is independent of score).

The three scripted-trajectory checkpoints (package 03) reuse rows 2–4:
leave-start-area = 100+shaping, first-upgrade = 310, first-boss = 710 —
**subject to** the real trajectory's actual `upgrade_flags` popcount at each
checkpoint; the runbook in package 03 says how to reconcile.

### 2.3 Local validation before handing off (do now)

- Validate `feature-maps/demo-game.yaml` against
  `schema/feature-map.schema.json`, and the map/program pair semantically.
  There is **no** `refwork-verify` CLI subcommand for this; the validation
  logic lives in `crates/refwork-featuremap/src/lib.rs`
  (`parse_feature_map`, `parse_scoring_program`, `validate_map`,
  `validate_scoring_standalone`, `validate_pair`). Invoke those via a
  standalone scratch cargo project with a path dependency on
  `refwork-featuremap` (scratchpad or an untracked throwaway — NOT committed).
  **Do not edit `crates/refwork-verify/src/main.rs` or any other
  currently-modified file** — they carry refwork-czi's uncommitted work.
- Confirm every feature referenced by `scoring/demo-game.yaml` exists in the
  map and is `stability: stable` (§2.2 rule). `player_x/y` are volatile but
  are not referenced by the program — verify, don't assume.
- Sanity-check the predicate grammar of every `when`/`goal` against §2.3.

Record results in the handoff doc. If anything fails, that is a real pre-gate
finding — fix the artifact or the spec, and say which.

## Joint half (at scorer M2, then M4)

- **At scorer M2:** their acceptance compiles `scoring/demo-game.yaml`. Run
  the joint check: their compiled program evaluates the hand-score table and
  every row matches. Record sign-off in both packets (this repo: a dated
  section appended to the handoff doc + bead comment; theirs: their packet's
  item 2 record). Divergence path: API.md wins; if API.md is wrong, fix it
  here, bump nothing silently, record in both packets.
- **At scorer M4:** load the real 20v pair into the live service jointly with
  the scorer side — their exit-gate evidence run requires it. Record the
  loaded map/program hashes + scorer build SHA; these exact identifiers are
  what packages 03–05 and the resolution cite.

## Exit Signal

Pre-gate: handoff doc exists, local validation recorded, hand-score table
published. Joint: both sign-offs recorded with build SHA + artifact hashes.
