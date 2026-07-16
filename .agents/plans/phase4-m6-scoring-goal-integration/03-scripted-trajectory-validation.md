# Package 03 — Scripted-Trajectory Stage Validation (Item 2)

**Gate:** GATE-RECORD.md exists (any branch). In the first-room-fallback
branch this package runs against the scripted trajectory's **first-room
prefix only** — which validates at most the `left_start_area` checkpoint;
record the reduction explicitly.

## What this proves

The three scripted checkpoints — leave start area → first upgrade → first
boss — score **exactly** per API.md §2.1, **evaluated by the live scorer**
(never a local reimplementation; `refwork-verify phase4-score-plan` output is
planning aid, not evidence).

## Inputs

- The scripted trajectory captures from the fast-follow's frozen bundle (real
  offsets, real pair from 20v). Identify them by the corpus id and the
  fast-follow's labeling — do not re-capture.
- Scorer M4 service (or M3 batch evaluator if the scorer packet exposes one —
  either is "the scorer"; record which). Build SHA + loaded real map/program
  hashes from package 02's M4 record.

## Runbook

1. Extract the feature vectors at the three checkpoint frames (the
   fast-follow's labels name them; if only frame ranges are labeled, the
   checkpoint frame is the first frame where the stage predicate holds).
2. Compute expected totals: stage sums are fixed (100 / 100+200 /
   100+200+400); shaping is `10 · popcount(upgrade_flags)` **at that frame**
   — read the actual captured `upgrade_flags`, don't assume the package-02
   table's idealized values. Write the three expected totals down *before*
   running the scorer.
3. Evaluate the three states through the scorer. Exact match required —
   any delta is a defect in exactly one of: capture labeling, this repo's
   spec/program, or the scorer. Settle per the spec-ownership rule; do not
   nudge expected values to match observed.
4. Record: corpus id, frame ids, feature values read, expected vs observed
   totals, scorer build SHA, map/program hashes. Append to the handoff doc
   and cite from the resolution.

## Exit Signal

Three (or, fallback branch, one) checkpoint rows with expected == observed,
recorded with full identifiers, cross-referenced by the scorer packet's
resolution (this is a two-sided item).
