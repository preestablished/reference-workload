# Package 04 — Hand-Played Labeled Trajectory: The Gate-3 Fixture (Item 3)

**Gate:** GATE-RECORD.md names the `full-corpus` or `raw-session` branch.
In the `first-room-fallback` branch this package **cannot run** — there is no
hand-played trajectory at all. Do not simulate one; the resolution names
gate 3 blocked on the hand-play session, and this package stays untouched.

## Artifact lineage rule (first decision, before any command)

- If the fast-follow's item 3 already emitted the traced/labeled trajectory
  (`refwork-verify trace` + `phase4-score-plan` output, frozen under the
  corpus id): **consume that frozen artifact directly.** Do not re-derive a
  second independently-hashed trajectory under a different id.
- Only if the fast-follow handed off the raw `ramdiff record` session
  *without* the trace step: run the conversion yourself —

  ```
  refwork-verify trace --captures <capture-index.jsonl> \
      --map <real feature-map from 20v> --scoring <real scoring program> \
      --labels <session labels> --out <labeled-trajectory.jsonl> \
      --report <trace-report>
  ```

  and freeze the output under the fast-follow's **existing** corpus id and
  versioning conventions (its package 05 defines them). One id everywhere.

Either way the artifact is one labeled feature-trajectory JSONL: one record
per frame, decoded features + stage annotations.

## The two gate-3 properties (scorer-evaluated)

Run the full trajectory through the live scorer (build + hashes from
package 02's M4 record) and verify:

1. **Monotonicity:** the stage-score component is non-decreasing frame over
   frame across the whole trajectory, and strictly increases at each labeled
   stage transition. Check the *stage* component, not the total — shaping
   (`10·popcount(upgrade_flags)`) is monotone for this program only because
   upgrades are never lost; if total-score is what you check, say so and
   justify it from the captured data.
2. **Goal-only-on-credits:** `goal_hit` is true on a frame **iff** that
   frame's `credits_flag != 0` — both directions. The credits-positive frames
   come from the credits save-state (scripted play or operator-provided
   late-game save RAM, whichever the fast-follow session produced). If the
   trajectory contains no credits-positive frame, the "fires" half is
   unproven — that is a gap, not a pass; get the credits fixture before
   declaring gate 3.

Record per-frame results in a compact evidence file (frame id → stage score,
goal_hit), its hash, and the exact command lines.

## Freeze

Version and freeze the labeled JSONL + evidence per the fast-follow's corpus
conventions, recording: artifact hashes, corpus id, scorer build SHA,
map/program hashes. This artifact is consumed by Phase 4 exit gate 3 and
Phase 8 campaign step 1 — the phases track will re-run the trace from a
clean checkout and diff hashes, so freeze exactly what the recorded command
produces, byte for byte.

## Exit Signal

Frozen labeled trajectory with recorded hashes; monotonicity and
goal-only-on-credits both reproducible from the frozen artifact + named
scorer build; gate-3 wording satisfiable verbatim from the record.
