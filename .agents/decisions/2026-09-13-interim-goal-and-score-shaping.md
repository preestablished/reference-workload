# Decision: Interim Goal = World-1 Clear; Score Shaping Objective

Date: 2026-09-13. Decided by Matt (operator) as the recorded default of
shared contract D4 (`SHARED-CONTRACT.md` of the discovery-02 plan suite),
in response to the plan
`~/.agents/projects/reference-workload/plans/discovery-02-processing-and-interim-scoring/`
(package `01-decision-gate-and-spec-amendment.md`, gate G0).

**Status: DEFAULT DECISION RECORDED 2026-09-13; operator wording
confirmation pending.** This file was drafted by the coding agent in an
autonomous session under the contract's default branch (D4: "interim goal
= W1 cleared / W2 reached, operator confirms at the plan-2 gate"). The
operator's explicit acceptance is requested in the WP6 push ask; until
then every gate-3 record produced under this decision carries the
"interim form" wording below and none claims the credits form.

## What is accepted

- The private scoring program v3 goal predicate is the world-1-clear latch
  (feature `stage_clear`, the same feature as stage `world1_clear`). The
  credits goal is deferred to Phase 8 under a new program version (v4).
- "fires-on-credits" is recorded UNDECLARABLE in every gate-3 record
  (GATE3-CLAIMS.md, GATE-RECORD.md, `04-resolution.md`, handoffs).
- Shaping adds a bounded score term per plan decision D-B (a byte-slice of
  the score counter, weighted so it can never outrank a stage); prune
  fires on death and game-over. Timer expiry in this game costs a life
  (`timer_expiry: life-lost`, recorded in the `discovery-02-gameover-timer`
  session notes), so it is a death variant covered by the death/game-over
  prune — no separate timer-expiry prune unless a distinct timer game-over
  state is found.
- `score` is isolated from the `discovery-02-score-currency` session's
  `score-only-*` brackets (not `discovery-02-main`'s mixed brackets);
  `currency` (coins) is a separate, provably independent counter (no
  co-moving event; `both_event: NOT-AVAILABLE`) mapped per plan decision
  D-G — mapped by default, shaped only if a currency gradient is wanted.
- discovery-01 is superseded by discovery-02
  (`.agents/decisions/2026-09-03-discovery-02-supersedes-discovery-01.md`);
  bead `refwork-1n8`'s migration step is replaced by re-confirmation of the
  11 known offsets on discovery-02.

## Standing consequences

- Phase 4 exit gate 3 is declared in its interim form only; the phases
  track records the reduction verbatim (dated amendment under exit-gate
  item 3 of `~/.agents/projects/determinism/phases/phase-4-scoring-and-inputs.md`,
  plus one-line notes in `docs/reference-workload/IMPLEMENTATION-PLAN.md`
  §M6 Accept and `docs/reference-workload/README.md` goal-predicate row).
- Any later credits-reaching trajectory re-opens goal authoring as program
  v4 and re-runs the gate-3 trajectory package (plan WP5).
- The prior recorded stage id `first_boss` (the W1-S2 midboss) is kept;
  the user's "first boss" (W1-S4) is expressed by the stages `world1_boss`
  and `world1_clear` (shared contract fact 9).

Precedent: `.agents/decisions/2026-09-03-discovery-02-supersedes-discovery-01.md`,
`.agents/decisions/2026-07-16-apu-clock-epoch-cut.md`.
