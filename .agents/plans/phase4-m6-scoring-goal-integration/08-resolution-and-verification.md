# Package 08 — Resolution & Self-Verification (Last)

## Self-verification matrix (run before writing the resolution)

Pre-run every check the phases track announced (request
`03-verification-offer.md`) so the resolution never claims what a re-run
would contradict:

| # | Phases-track check | Local pre-check |
|---|---|---|
| 1 | Re-run `refwork-verify trace` from clean checkout; diff JSONL vs frozen fixture (hash match) | Do exactly that: fresh `git worktree`/clone, recorded command line, `sha256sum` diff. Byte-identical or the freeze is wrong. |
| 2 | Re-evaluate frozen trajectory through the named scorer build; confirm monotonic + goal-only-on-credits from scratch | Re-run package 04's evaluation from only the frozen artifacts + recorded SHAs, on a machine state that didn't produce them. |
| 3 | Cross-check fixture-corpus id/hashes vs fast-follow FULFILLMENT records (`~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`, `~/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`) | grep both records; one id, same hashes, no drift. |
| 4 | Read smoke evidence: burst/Fault/mismatch counts + window note | Confirm the evidence block contains all fields in package 06's list. |
| 5 | Phase 4 exit gate 3 declarable verbatim | Read the gate wording next to the evidence; hand-play branch only. |

## `04-resolution.md` skeleton

Create `.agents/requests/phase4-m6-scoring-goal-integration/04-resolution.md`
(it does not exist yet — the request directory holds only `00`–`03`). The
acceptance baseline is the owner IMPLEMENTATION-PLAN §M6's three Accept
bullets, adopted by reference
(`~/.agents/projects/determinism/docs/reference-workload/IMPLEMENTATION-PLAN.md`,
"### M6" section: <1 ms/state on the 1,000-state fixture; trace → monotonic
staged score + goal-only-on-credits; 1,000-burst zero-Fault/zero-mismatch
smoke — packages 05/04/06 respectively):

```markdown
# Resolution (M6)

## Identifiers
- reference-workload SHA: <...>   scorer build SHA: <...>
- Loaded map hash: <...>          program hash: <...>
- Corpus id: <...>  Labeled-trajectory hash: <...>  Sidecar hash: <...>

## Entry-gate branch
<full-corpus | first-room-fallback | raw-session>, per GATE-RECORD.md (<date>).
[Fallback only:] Item 3 did not run; Phase 4 exit gate 3 is NOT declared —
blocked on the hand-play session.

## Bead states
<M6 bead + children, czi/20v/5tk final states>

## Validation results (the four)
1. Compile + hand-score joint validation: <result, both-sides record refs>
2. Scripted-trajectory checkpoints: <expected vs observed table ref>
3. Gate-3 fixture: monotonicity <result>, goal-only-on-credits <result>
4. Fixture corpus + budget: <match count, p50/p99/max, both-sides refs>

## Smoke
Bursts <n>, Faults <n>, spot-replays <n>, hash mismatches <n>; window note.
[Recorded separately from the scorer packet's joint smoke: <ref>.]

## Handoff surface
<registered (id) | manifest+dist, follow-on named>

## Gate assessment
Gate 3: <declared / NOT declared + blocker>. Evidence contributed to
gates 1–2: <plainly listed>. Cross-references: scorer packet resolution
<path> (items 1–2, 4, 5-window).
```

## Close-out

- Close the M6 bead (`bd close $M6 -r "..."`) — or, in the fallback branch,
  leave it open with a comment naming the hand-play blocker, matching the
  resolution.
- Update memory/FULFILLMENT-adjacent records only where this packet owns
  them; the fast-follow owns its own.
