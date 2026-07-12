# Package 01 — Entry Gate & Tracking (Startable Now)

## Goal

Give M6 a durable tracking anchor (request item 0) and a mechanical,
re-runnable entry-gate check so no later package guesses at gate state.

## Step 1: Create the M6 bead (do now)

```bash
M6=$(bd create "M6: scoring/goal integration + exploration readiness (gated)" \
  -d "Phase 4 close-out per .agents/requests/phase4-m6-scoring-goal-integration/. Supplies Phase 4 exit gate 3 (staged program scores hand-played trajectory monotonically; goal fires only on credits flag). GATED on: scorer M3 closed (see state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/), refwork-czi closed, refwork-20v closed, hand-play session artifact existing. Items 4-5 additionally need scorer M4 + confirmed-up stack. Plan: .agents/plans/phase4-m6-scoring-goal-integration/. Joint packet: state-scorer phase4-m1-m4-first-boss-scoring (items 1-2, 4, 5-window are two-sided)." \
  -p 1 -l integration,phase4 -t task --silent)
bd dep add $M6 refwork-czi
bd dep add $M6 refwork-20v
```

Notes:
- Labels: `bd create -l` accepts comma-separated labels
  (`-l integration,phase4`), matching the request's item 0.
- Do NOT add a dep edge to `refwork-5tk`: entry condition 4 is satisfiable by
  the raw hand-play session alone (fast-follow entry condition 6) even if 5tk's
  full corpus freeze is still open. A hard edge would over-block. The gate
  checker (step 2) checks the artifact, not the bead.
- scorer M3 is in another repo's tracker — it cannot be a bead edge; it lives
  in the gate checker and the bead description.
- Child beads per work item are optional; create them lazily when the gate
  opens (`bd dep add CHILD $M6`).

## Step 2: Gate-check script (do now)

Add `tools/m6-gate-check.sh` (executable, no external deps beyond `bd`,
`jq` optional). It must print one line per condition with PASS/FAIL/UNKNOWN
and exit nonzero unless all of 1–4 pass:

1. **scorer M3 closed** — cannot be checked mechanically from this repo with
   certainty; check `~/git/preestablished/state-scorer/.beads/` if beads exist
   there, else print `UNKNOWN — verify in state-scorer packet` and count it as
   not-passed. Never print PASS on a heuristic.
2. **refwork-czi closed** — `bd show refwork-czi` status is `closed`.
3. **refwork-20v closed** — same.
4. **hand-play artifact exists** — probe, in order: the frozen corpus location
   the fast-follow's FULFILLMENT records name; else a raw `ramdiff record`
   session directory recorded in the fast-follow's package-04 evidence. Print
   which branch matched: `full-corpus`, `first-room-fallback`, `raw-session`,
   or `NONE`. `first-room-fallback` passes the gate but the script must print
   the reduction warning: item-3 blocked, gate 3 not declarable.

Also print (informational, not gating): scorer M4 status, stack-up hint
(`systemctl --user status` of the bridge unit or the documented equivalent),
and `rom-operator-bridge-l1w` open/closed for the smoke-window note.

The script's paths for condition 4 cannot be finalized until the fast-follow
freezes locations — implement with the currently-documented candidates and a
clearly-marked `CANDIDATE PATHS` block at the top for the executing agent to
update. A gate checker with stale paths must fail closed (NONE), not pass.

## Step 3: Branch decision record (at gate-open, not now)

When the gate first passes, append `.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md`:
date, `m6-gate-check.sh` output verbatim, which entry branch held
(full corpus / fallback / raw session), scorer build SHA available, and the
resulting scope (all items vs item-3-blocked reduction). Update the M6 bead
with the same via `bd` comment. Every later package's gate check is: "does
GATE-RECORD.md exist and name a branch that permits this package?"

## Exit Signal

- M6 bead exists with edges to czi/20v and the scorer-packet note.
- `tools/m6-gate-check.sh` runs and truthfully reports today's state
  (expected today: FAIL on all four conditions, artifact branch NONE).
