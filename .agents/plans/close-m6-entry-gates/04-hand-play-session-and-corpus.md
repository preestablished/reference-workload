# Package 04 — Hand-Play Session & Corpus (Operator Session + Agent Pipeline)

**Gate:** packages 02–03 complete (approved branch, validated real
map/layout, committed exporter). Runbook: fast-follow
`04-operator-capture-and-labeling.md` (full branch) or
`04a-first-room-fallback.md` (fallback) — read the applicable one fully;
then `05-bundle-validation-and-freeze.md` for the freeze.

## What M6 actually needs from this package

M6's entry condition 4 is satisfied by the **raw hand-play session
artifact alone** — full 5tk closure (freeze/handoff/fulfillments) is 5tk's
own acceptance, not M6's gate. Don't block M6's gate-open on the last
mile of 5tk paperwork; do both, but know which milestone unlocks what.

## Operator briefing (deliver before the session — this is why M6 filed early)

The recorded trajectory must include, in one coherent padlog:
1. start area + the leaving-start-area transition;
2. first upgrade;
3. first boss + post-boss evidence;
4. ordinary goal-negative states along the way;
5. a credits/late-game goal-positive state (hand-play, approved snapshot,
   or operator-provided late-game save RAM — record the source; identity
   of image/ROM/map/scoring/layout must be preserved).

M6-specific labeling needs (from its request item 3): stage annotations
must be derivable at each transition, and the credits fixture is what the
goal-only-on-credits proof rests on — without it, gate 3 of Phase 4
cannot be declared even on the full branch.

## Agent pipeline (during/after the session)

Per fast-follow 04 steps 3–8: exporter run (≥1,000 states, framebuffers
on every primary row, cadence covering transitions), immediate
`phase4-artifact-check`, dedup-groups (both relation types), operator
labels joined by capture id (operator reviews the mandatory examples),
K=32 `phase4-score-plan`, `trace` → `trajectory/first-boss.jsonl` +
trace report. Then fast-follow 05: bundle validation, checksum freeze,
retrieval re-verify.

Worker safety: confirm deployed worker provenance includes hypervisor
`c0337ab` or later, else bounded-run caps (fast-follow 01 step 3).
Window: `rom-operator-bridge-l1w` was open as of 2026-07-12 — long
capture sessions must not overlap live Play; re-check and coordinate.

## Fallback branch

Execute 04a instead: scripted first-room log only, separately typed
fallback bundle, honest reduced claims, follow-on owner for the missing
trajectory. Gate 4 then passes with branch=first-room-fallback and the
M6 reductions apply (item 3 blocked, exit gate 3 not declared).

## Exit signal

Full: frozen (or freeze-in-progress) corpus with trajectory + labels;
`refwork-5tk` closable per its own acceptance. Either branch: the
artifact exists at a durable path the M6 gate checker can probe —
hand that path to package 05.
