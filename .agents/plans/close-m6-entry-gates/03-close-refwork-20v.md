# Package 03 — Close `refwork-20v`: Real Map/Scoring/Layout (Mixed)

**Gate:** GATE-RECORD-ASK1.md names full-corpus or fallback. Runbook:
fast-follow `03-private-map-layout-and-scoring.md` — read it fully first;
this package only assigns the work and adds what's changed since.

## Division of labor

**Operator does** (needs hands on the game):
- Controlled hand-play segments for `ramdiff` offset discovery — short,
  targeted state changes (leave start area, pick up first upgrade, take
  damage, defeat first boss, trigger death state, reach/load
  credits/late-game state) so each candidate offset's behavior is
  isolated. The agent scripts *what* state changes are needed per feature
  (from the demo map's feature list: room_id, area_id, player x/y,
  health, upgrade_flags, boss_flags, game_mode, credits_flag) and the
  operator performs them.
- Confirms ROM/revision identity for `meta.game_revision` (private).

**Agent does** (everything else):
- Drives the `ramdiff` pipeline. Concrete seam: the human-driven capture
  is `cargo run -p ramdiff --features interactive -- record --interactive
  --rom <private> --session <private-dir>` — the **operator** is at the
  controls for exactly those invocations; everything downstream
  (`search`, `candidates`, `watch`, `emit`, and any `record --script`
  replay) is agent-only and non-interactive. The agent proposes
  offsets/types/stability from the sessions and validates stability with
  repeated/restored states (fast-follow 03 step 2 — never mark a field
  stable off one trace). Verify the exact CLI flags against
  `crates/ramdiff` at execution time.
- Authors the private `feature-map.yaml` + `scoring-program.yaml` beneath
  the private root (structure from the checked-in demo pair, offsets from
  discovery — never demo offsets).
- Runs the validation chain (fast-follow 03 steps 3–7): featuremap
  validate, real `map-check` with durable report, `phase4-layout`
  generation citing the **package-01 exporter commit SHA**, independent
  layout review, small capture probe vs `ReadGuestMemory` cross-check.

## Additions since the fast-follow was planned

- The scoring program should be authored to the **same stage/goal shape**
  the demo program and API.md §2.1 define — state-scorer M2's compiler is
  live and their M4 service will load this exact pair (M6 plan package 02
  joint half); gratuitous divergence from the demo shape creates joint
  re-validation work.
- state-scorer's resolution flagged 3 spec questions we own (threshold
  edge inclusivity; `not{}` over failed `valid_when` guard ⇒ TRUE;
  bit-range stricter than schema). **Settle them in API.md before
  authoring the real program** — the real pair should be written against
  ratified semantics, not re-audited after. API.md is not in this repo:
  `~/.agents/projects/determinism/docs/reference-workload/API.md`
  (§1 map, §2.1–2.3 scoring DSL).

## Exit signal

Fast-follow 03 exit criteria all hold; evidence stored under the private
root's `validation/`; `bd close refwork-20v -r "<evidence summary,
opaque refs only>"`; gate checker shows gate 3 PASS. Public record:
opaque hashes and pass/fail only.
