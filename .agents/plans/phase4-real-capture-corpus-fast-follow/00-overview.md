# Phase 4 Real-Capture Corpus Fast-Follow

## Outcome

Produce and freeze a validated lab-private Phase 4 corpus from the real in-VM
capture path, then publish only sanitized handoff evidence. The normal outcome
is a corpus of at least 1,000 frame-coherent captures with real feature offsets,
framebuffers, dedup labels, a first-boss trajectory, goal-positive and
goal-negative labels, and deterministic K=32 score batches. The same frozen
corpus id must close both project-level evidence floors and become the input to
state-scorer M1 and input-synthesizer M2.

This plan does not implement reference-workload M6, rebuild the hypervisor
capture engine, or publish private game-derived material.

## Current Gate State

As of the 2026-07-10 request update and the repository state inspected while
planning:

- `refwork-gp9` and `refwork-d7t.11` through `.15` are closed.
- The capture engine is proven on the real image. Use a worker containing
  determinism-hypervisor `c0337ab` or later; otherwise retain bounded Runs.
- `refwork-d7t.1` remains blocked on durable operator-approved M2 floor
  evidence.
- The operator-private hand-play/corpus session and metadata/publication
  disposition are not recorded.
- Exporter implementation and synthetic tests may start immediately. Real map
  discovery may use approved early captures, but private capture, labeling,
  approval, and handoff cannot finish before the operator session.

Re-check these facts at execution time; do not infer operator approval from the
technical gates being green.

## Work Packages

| File | Work package | Exit signal |
|---|---|---|
| `01-preflight-gates-and-privacy.md` | Revalidate prerequisites, open beads, and record the operator decision | A durable go/no-go record selects full corpus or approved fallback |
| `02-exporter-implementation.md` | Add the real capture exporter and synthetic contract tests | Exporter emits validator-compatible rows from mock capture-engine responses |
| `03-private-map-layout-and-scoring.md` | Discover real offsets and compile private feature/scoring/layout contracts | Map validation, real map-check, and layout evidence pass privately |
| `04-operator-capture-and-labeling.md` | Run the hand-play session and build labels/consumer aids | Full corpus has >=1,000 captures and mandatory label coverage |
| `04a-first-room-fallback.md` | Define and validate the explicitly approved limited fallback | A separately typed fallback bundle has reproducible limited claims |
| `05-bundle-validation-and-freeze.md` | Assemble, validate, checksum, redact, and freeze | Immutable private bundle and sanitized version record agree on one id |
| `06-context-and-downstream-handoffs.md` | Build the context fixture and executable cold-agent handoffs | Both downstream consumers can retrieve and identify the frozen inputs |
| `07-fulfillment-and-closeout.md` | Update fulfillment records, beads, and request resolution | Both floors reflect their true completion state with evidence |
| `08-verification-matrix.md` | Final clean-checkout and privacy verification | All applicable gates pass and no private payload is tracked |

## Dependency Shape

1. Complete package 01 before any operator-private production.
2. Packages 02 and the non-private preparation in 03 may proceed in parallel.
3. Finish the real map/layout contract before the production export so every
   row is packed against the final layout.
4. Run package 04 only in the approved operator session. If the full session is
   unavailable but fallback is approved, execute package 04a instead.
5. Execute packages 05, 06, and 07 in order using the selected bundle kind;
   package 08 is the final audit.

Do not silently mutate a completed bundle to repair a downstream issue. Any
change to captures, labels, maps, scoring, layout, or manifests creates a new
corpus version and repeats packages 05 through 08.

## Durable Outputs

Source repository:

- exporter source and synthetic tests;
- sanitized corpus version record and downstream smoke instructions;
- request resolution at
  `.agents/requests/phase4-real-capture-corpus-fast-follow/04-resolution.md`;
- bead ids and close reasons for exporter and real-map work.

Lab-private storage only:

- ROM metadata, real feature offsets, scoring labels, capture ids and decoded
  values, padlogs, raw/compressed capture artifacts, trajectories, retrieval
  secrets, and private validation reports containing those values;
- complete scorer corpus and live context fixture.

External project records:

- `$HOME/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`;
- `$HOME/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`.

## Definition Of Done

The full path is done only when the private bundle passes every validator, its
version is frozen, both fulfillment records name the same opaque corpus id,
operator publication/private-only disposition is recorded, retrieval/access/
retention/regeneration notes are complete, downstream handoffs are executable
by a cold agent, and public evidence passes redaction scanning.

If the operator explicitly approves the first-room-only fallback, completion
means the fallback is frozen and the scorer request remains **partially
fulfilled** with a concrete follow-on for trajectory, first-boss, and goal
coverage. Never describe that fallback as satisfying the full Phase 4 scorer
golden-artifact gate.
