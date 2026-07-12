# Phase 4 M6 — Scoring/Goal Integration + Exploration Readiness (Plan)

## Outcome

Close reference-workload M6 and supply **Phase 4 exit gate 3**: the staged
scoring program scores a hand-played trajectory monotonically through its
stages, and the goal predicate fires only on the credits flag — evaluated by a
live `state-scorer` build, not a reimplementation. Along the way: joint
compile/hand-score validation of the scoring DSL, scripted-trajectory stage
validation, the 1,000-state fixture corpus with expected scores under the
scorer's <1 ms/state budget, the exploration-readiness smoke (1,000 bursts,
zero `Fault`s, zero determinism-hash mismatches on spot-replays), and the
manifest + `dist/` handoff surface.

This plan does **not** duplicate the fast-follow
(`.agents/plans/phase4-real-capture-corpus-fast-follow/`): the exporter, the
real-offset private map/scoring pair, the hand-play session, and the frozen
corpus are its deliverables and this plan's *inputs*.

## Gate Reality — Read Before Executing Anything

The request (`.agents/requests/phase4-m6-scoring-goal-integration/`) is
**explicitly gated** and was filed for choreography. As of planning
(2026-07-12) the entry gate does NOT hold:

- scorer M3: state-scorer is a Phase 0 skeleton; its M1→M4 chain was requested
  the same day. **Not closed.**
- `refwork-czi`: in progress (implementation done, held open until committed +
  clean-checkout gate recorded). **Not closed.**
- `refwork-20v`: open. **Not closed.**
- Hand-play session artifact: none; `refwork-5tk` launch decision is no-go
  pending an approved operator session. **Does not exist.**

Therefore this plan has two strata:

1. **Startable now (pre-gate):** package 01 (tracking bead + gate-check
   tooling) and package 02's pre-gate half (demo program/map handoff package
   for state-scorer M2, with the hand-computed stage-score table). The request
   explicitly marks item 1 "startable at scorer M2, before the full gate", and
   the handoff *preparation* is startable immediately.
2. **Gated runbooks (packages 03–08):** written so a cold agent can execute
   them the day the gate opens, with zero re-derivation. Executing them now is
   an error — each opens with its own gate check.

## Work Packages

| File | Work package | Stratum | Exit signal |
|---|---|---|---|
| `01-entry-gate-and-tracking.md` | M6 bead + dep edges, mechanical entry-gate checker, branch decision procedure | now | Bead exists with edges; `tools/m6-gate-check.sh` reports gate state truthfully |
| `02-program-handoff-compile-validation.md` | Demo (later real) map/program handoff to state-scorer; joint compile + hand-score validation | pre-gate half now | Handoff doc + hand-score table published; joint sign-off recorded at scorer M2/M4 |
| `03-scripted-trajectory-validation.md` | Stage scores on the scripted trajectory via the live scorer | gated | Three checkpoints score exactly per the hand-score table, scorer-evaluated |
| `04-gate3-labeled-trajectory.md` | Hand-played labeled trajectory: monotonicity + goal-only-on-credits (the gate-3 fixture) | gated | Frozen labeled JSONL with recorded hashes; both properties reproducible from it |
| `05-fixture-corpus-and-budget.md` | 1,000-state fixture corpus with expected scores; <1 ms/state confirmation | gated | Corpus id consistent with fast-follow records; budget evidence recorded |
| `06-exploration-readiness-smoke.md` | Orchestrator dev loop: 1,000 bursts, zero Faults, zero spot-replay hash mismatches | gated (scorer M4 + stack) | Smoke counts recorded with window-coordination note |
| `07-handoff-surface.md` | `WorkloadImage` registration or manifest + `dist/` layout handoff | gated | Disposition recorded (registered vs manifest+dist + follow-on note) |
| `08-resolution-and-verification.md` | `04-resolution.md` authoring template + self-verification matrix | last | Resolution appended; every phases-track check pre-passed locally |

## Dependency Shape

1. Package 01 first (bead + gate checker) — it is the switchboard everything
   else consults.
2. Package 02's pre-gate half immediately after; its joint half fires when
   scorer M2 closes (compile) and again at scorer M4 (real-pair load).
3. Packages 03 and 04 share the operator-session artifacts; run 03 before 04
   (scripted before hand-played) unless the session delivers everything at
   once, in which case 03–05 collapse into one validation pass — record it as
   one pass, not three fabricated ones.
4. Package 05 needs scorer M4 timing harness; package 06 needs scorer M4 +
   confirmed-up stack + window coordination (hypervisor `l1w` still open).
5. Package 07 any time after 04; package 08 strictly last.

## Fallback Branch (Entry Condition 4)

If only the first-room fallback exists: package 03 runs against the scripted
trajectory's first-room prefix only, **package 04 cannot run**, and Phase 4
exit gate 3 is **NOT declared** — the resolution names it blocked on the
hand-play session. Never soften this. Record which branch held in the bead and
in `04-resolution.md`.

## Standing Constraints

- The working tree currently carries `refwork-czi`'s uncommitted
  implementation. Do not commit, revert, stage, or "clean up" that diff; M6
  work adds only its own files until czi lands. Never use `git add -A`/`.`.
  The hands-off set as of planning (`git status --short`):
  modified — `Cargo.lock`, `crates/refwork-dh-client/src/mock.rs`,
  `crates/refwork-verify/Cargo.toml`, `crates/refwork-verify/src/lib.rs`,
  `crates/refwork-verify/src/main.rs`,
  `crates/refwork-verify/src/phase4_bundle_check.rs`,
  `crates/refwork-verify/src/phase4_checksum_manifest.rs`,
  `crates/refwork-verify/src/phase4_context_check.rs`,
  `crates/refwork-verify/tests/integration.rs`,
  `docs/phase4-corpus-guide/{03-capture-export,05-validate-handoff,index}.html`;
  untracked — `crates/refwork-verify/src/phase4_artifact_check.rs`,
  `phase4_capture_export.rs`, `phase4_context_export.rs`,
  `phase4_fallback_check.rs`, and `data/`. Re-run `git status` at execution
  time; the rule is the czi diff, the list is its snapshot.
- The checked-in `feature-maps/demo-game.yaml` + `scoring/demo-game.yaml` are
  placeholder-offset artifacts: valid for scorer M2 compile validation and
  DSL semantics, **contractually disqualified** for every real-capture item.
- Spec ownership: this repo's API.md governs DSL semantics. Scorer-compiler
  divergence is settled by API.md unless API.md itself is wrong — then fix
  API.md here and record the change in both packets.
- One corpus id everywhere: never mint a second id or re-hash an artifact the
  fast-follow already froze.
