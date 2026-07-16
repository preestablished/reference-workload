# Request: M6 — Scoring/Goal Integration + Exploration Readiness (Gated, Joint With state-scorer)

## Who Is Asking

The phases track, on behalf of Phase 4
(`~/.agents/projects/determinism/phases/phase-4-scoring-and-inputs.md`) —
M6 is this repo's Phase 4 close-out and supplies **Phase 4 exit gate 3**:
"the staged scoring program scores a hand-played trajectory monotonically
through its stages, and the goal predicate fires only on the credits
flag." Filed 2026-07-12, the day Phase 3 closed (round 5).

## Standing Relative To The Fast-Follow — Read This First

This request is **explicitly gated** and deliberately thin. The
already-filed `phase4-real-capture-corpus-fast-follow/` request (round 2,
updated 2026-07-10; live beads `refwork-czi`/`refwork-20v`/`refwork-5tk`)
owns the capture exporter, the real-offset private feature map + scoring
program, the hand-play session, and the frozen corpus. **Do not duplicate
any of that here.** This request is what remains of the owner
IMPLEMENTATION-PLAN's §M6 *after* the fast-follow delivers: proving the
scoring semantics against a live state-scorer build and closing the
exploration-readiness smoke. The phase doc gates M6 on scorer M2–M3,
which do not exist yet (state-scorer's chain was requested today, same
round) — so this packet is filed now for choreography, opened later.

Why file it now rather than when the gate opens: the fast-follow's
hand-play session (its entry condition 6) is the *same operator session*
that produces M6's labeled trajectory, and state-scorer's M2 acceptance
compiles this repo's `scoring/demo-game.yaml`. Both counterparts need to
know M6's consumer exists before they run — same reasoning that paired
the fast-follow with round 1.

## The Ask In One Paragraph

Once the entry gate holds (fast-follow corpus artifacts + scorer M3
closed; scorer M4 service for the end-to-end items): load this repo's
feature map and staged scoring program into the live scorer and verify
stage scores on the scripted trajectory (leave start area → first
upgrade → first boss score exactly per API.md §2.1); convert the
hand-played `ramdiff record` session via `refwork-verify trace` into the
labeled feature-trajectory JSONL and show the staged program scores it
monotonically with the goal predicate firing only on the credits flag
(gate-3 fixture); supply the scorer's 1,000-state fixture corpus with
expected scores and confirm its <1 ms/state budget on this map; run the
exploration-readiness smoke (orchestrator dev loop, 1,000 bursts, zero
`Fault`s, zero determinism-hash mismatches on spot-replayed branches);
and ship the manifest + `dist/` layout handoff (control-plane
`WorkloadImage` registration only if their resource API exists by then).

## Files In This Request

| File | Contents |
|---|---|
| `00-overview.md` | This file — who/why/scope boundary vs the fast-follow |
| `01-current-state.md` | Evidence: tooling, beads, gate status of every input |
| `02-requested-work.md` | Entry gate, work items, acceptance, out-of-scope |
| `03-verification-offer.md` | Verification, handback, sibling choreography |
