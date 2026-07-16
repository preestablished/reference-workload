# Package 07 — Gate-3 Labeled Trajectory, Scorer Evaluation, Handoff

## Goal

Emit the labeled feature trajectory from the frozen corpus, evaluate it
through the **live scorer** (never a local reimplementation — scoring-goal
packages 03/04 govern), record exactly what Phase 4 exit gate 3 can claim
from this trajectory, and deliver the corpus handoff state-scorer is waiting
on (`state-scorer-v8n`).

## Steps

### 1. Trace (agent)

The trace consumes the **composed index** derived in package 06 step 5
(`$PR/bundle/derived/index-composed.jsonl` — main + credits sibling bundle,
identical map/layout hashes asserted, capture-id sets disjoint). `trace`
takes one fully-labeled index; the two bundles never feed it separately.

Labels file first: `refwork-verify trace` requires a label row for **every**
capture id in the index (verified, `phase4_trace.rs`), schema
`kind: phase4-trace-labels` / `schema_version: 1`. Generate it
programmatically from the COMPOSED index + the frame→event table:
anchors get `expected_highest_stage` / `first_boss_coverage` / `prune` /
`goal`; `goal: true` ONLY on credits-bundle captures; everything else gets
an explicit `goal: false`-style truthful row. Spot-check ≥10 rows by hand
against the decoded values before running:

```sh
# The pipeline stage must run over the composed index — use its explicit
# --captures override if it has one; else invoke refwork-verify trace
# directly with the pipeline's report-path conventions:
tools/m6-session-pipeline.sh trace --private-root "$PR" \
  --captures "$PR/bundle/derived/index-composed.jsonl" \
  --labels "$PR/bundle/labels/trace-labels.yaml"
# → $PR/bundle/trajectory/first-boss.jsonl + validation/trace-report.json
```

Freeze the trajectory under the SAME corpus id conventions (scoring-goal
package 04 "artifact lineage rule": this is the only derivation; no second
independently-hashed copy ever).

### 2. Scorer-side evaluation (joint; needs the live M4 service)

> **Coordination (not a full STOP) — and the agent does the work:** the
> executing agent builds and starts the state-scorer M4 service ITSELF per
> their `docs/joint-smoke-runbook.md` §1 (build `scorer-service`, run
> `state-scorerd`, healthz check) — this is agent-doable; do not wait on the
> operator for it. Escalate to a STOP only if the service turns out to need
> resources or credentials the agent lacks (ports, keys, a deployed host).
> Then load the REAL pair per its §2 call order (`LoadFeatureMap` with the
> layout compiled from the 20v map — cross-check `feature_bytes_len` — then
> `LoadScoringProgram`; record both returned hashes + build SHA). This is
> scoring-goal package 02's "at scorer M4" joint half — record sign-off in
> the handoff doc as that package specifies.

Then run scoring-goal packages 03 and 04's runbooks over the frozen
artifacts:

- package 03: the three checkpoint states score exactly per the
  pre-committed expected totals (compute expected values from the REAL
  program's stage points and the captured feature values — not the demo
  table's idealized rows);
- package 04: full-trajectory monotonicity of the **stage component** and
  goal_hit ⇔ credits-feature ≠ 0, both directions, with per-frame evidence
  file + hashes + exact command lines.

### 3. The gate-3 claim record (write it whichever way the evidence lands)

Append `GATE3-CLAIMS.md` to this plan dir (public-safe):

- **Claimable from discovery-01 alone (trajectory ends at world-2 stage-1):**
  monotonic staged scoring through every stage the trajectory reaches
  (left-start → first upgrade → midboss → world-1 boss → world-2 reached),
  and the goal predicate **never fires** on any of its ~1,005 non-credits
  states (the "only" direction's negative half).
- **Claimable only with the Run C credits fixture in the corpus:** the
  "fires" half — goal_hit true on credits-positive states. With Run C done
  (package 06 step 4), gate 3 is declarable in full.
- **If Run C was deferred:** gate 3 is NOT declared (scoring-goal package 04:
  "no credits-positive frame ⇒ the fires half is unproven — a gap, not a
  pass"); name the blocker (credits-reaching capture) and the owner. Never
  soften.

### 4. State-scorer handoff (deliverable (d))

Per fast-follow 06's "State-Scorer Handoff" section (its checklist governs:
retrieval, checksum verification, corpus id, decode-golden + K=32 gate
commands, coverage expectations, ownership boundary). Additions:

- Write the smoke document where their packet expects it, citing the frozen
  corpus id, `score-plan.json`, the trajectory + trace report, and the
  expected-scores question: the sidecar is scoring-goal package 05's
  deliverable — if not yet produced, say so explicitly (their budget run is
  scoring-goal package 05, not this package).
- Update `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md` §5's M4
  slot with the joint-load record (hashes + build SHA) from step 2.
- Note their beads DB loss: reference `state-scorer-v8n`/`state-scorer-wg0`
  by name with a pointer to their `04-resolution.md`, and leave their-side
  bead bookkeeping to their side.
- `redaction-scan` every public handoff file before it lands
  (fast-follow 06 privacy checks).

## Acceptance criteria

- `trace-report.json` status pass; trajectory JSONL frozen under the corpus
  id with recorded hash; labels file covers 100% of the COMPOSED index's
  capture ids (both bundles) and its goal column matches the credits ground
  truth.
- Scorer evaluation recorded with build SHA + loaded map/program hashes;
  monotonicity + goal analyses reproducible from frozen artifacts (this is
  scoring-goal package 08's verification matrix rows 1–2 — pre-run them now).
- `GATE3-CLAIMS.md` written, internally consistent with the Run C branch
  actually taken.
- Handoff document delivered and redaction-scanned; handoff doc M4 section
  updated.

## On failure

- Trace errors ("no label for capture_id", order mismatch): fix the label
  generator or the index reference — never hand-edit trajectory rows.
- Monotonicity violation at a non-transition frame: a stage predicate sits on
  non-latched state (package 05 design rule was violated) or a feature
  misdecoded — if the map/program must change, the fast-follow stop condition
  cascades: new corpus id, re-run package 06. This is exactly why package 05
  step 1 front-loads the latched-state rule.
- Scorer disagreement with expected totals: spec-ownership rule
  (scoring-goal package 02) — settle by API.md; never nudge expectations.
