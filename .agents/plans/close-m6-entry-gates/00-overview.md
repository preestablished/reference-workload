# Close The M6 Entry Gates (czi / 20v / hand-play)

## Outcome

Drive the three failing M6 entry conditions to PASS so
`.agents/requests/phase4-m6-scoring-goal-integration/` can open:

| Gate | Today | Closes when |
|---|---|---|
| 2. `refwork-czi` | in progress — implementation done, sitting **uncommitted** in the working tree | committed + final clean-checkout gate recorded |
| 3. `refwork-20v` | open | real-offset private map/scoring/layout validated under the private root |
| 4. hand-play artifact | none — `refwork-5tk` no-go | operator session produces the `ramdiff record` hand-play trajectory (full corpus, or explicitly approved first-room fallback with its reduced claims) |

Gate 1 (scorer M3) already passes — state-scorer's M1–M4 chain resolved
2026-07-12; their service is first-boss-ready, so nothing on their side
blocks even M6's items 4–5 once these three close.

## What this plan is and is not

The fast-follow plan
(`.agents/plans/phase4-real-capture-corpus-fast-follow/`, packages 01–08)
already contains the detailed runbooks for all three gates. **This plan does
not duplicate them.** It is the orchestration layer: correct sequencing,
the agent-vs-operator split, the exact operator asks (scripted, minimal,
batched), stale-fact corrections discovered since the fast-follow was
planned, and the closeout wiring back to the M6 gate checker.

The executing agent reads the referenced fast-follow package *before*
executing each phase here; where this plan and a fast-follow package
disagree on mechanics, the fast-follow package wins (it is the runbook);
where they disagree on *facts* (bead states, gate status), re-verify live —
both plans' snapshots go stale.

## Operator involvement model

The operator (Matt) is available but asks must be batched and minimal:

1. **Ask 1 — launch decision** (package 02): one structured question set
   covering the fast-follow's operator checklist, branch selection
   (full corpus / first-room fallback / blocked), and private-root location.
   Nothing private is produced before this.
2. **Ask 2 — the session itself** (packages 03/04): controlled hand-play
   for `ramdiff` offset discovery and the recorded trajectory (through
   first boss + credits/late-game fixture). One session should feed both
   20v discovery and the 5tk corpus where possible — that is the whole
   reason M6's packet was filed early.
3. **Ask 3 — push approval** (package 05): main is unpushed and pushes to
   main need explicit approval.

Everything else — czi commit + clean-checkout gate, validators, layout
generation, artifact checks, bead bookkeeping, gate-checker updates — is
agent work and must not be delegated to the operator.

## Packages

| File | Package | Blocked on |
|---|---|---|
| `01-close-refwork-czi.md` | Verify, commit, clean-checkout-gate, and close the exporter work | nothing — start immediately |
| `02-operator-launch-decision.md` | Ask 1: checklist, branch, private root; record the decision | operator answer |
| `03-close-refwork-20v.md` | Real map/scoring/layout discovery + validation | 01 (exporter SHA for layout), 02 (+ session time for discovery hand-play) |
| `04-hand-play-session-and-corpus.md` | The recorded trajectory; corpus production per selected branch | 02, 03 |
| `05-closeout-and-gate-verification.md` | Bead closures with evidence, gate-checker CANDIDATE PATHS update, 4/4 verification, push ask | 01–04 |

## Sequencing

01 immediately (it also unblocks: every later phase wants the exporter
committed so provenance can cite a real SHA, and 20v's layout step needs
the exporter commit id). Then 02 (single operator ask). 03 and 04 are one
combined operator session where practical — discovery captures for 20v and
the recorded trajectory for gate 4 share the session; the fast-follow's
packages 03/04 describe the split if they must be separate. 05 last.

## Standing constraints (inherited, still binding)

- Never track private game-derived payloads; private root lives outside
  every checkout. No ROM names, offsets, decoded values, or private paths
  in public records, beads, or terminal transcripts.
- The first-room fallback branch closes gate 4 for *reduced* claims only:
  M6 item 3 cannot run and Phase 4 exit gate 3 is not declared. Never
  soften; record the branch taken everywhere.
- One corpus id everywhere once frozen; no re-derivation under new ids.
- Stale-fact corrections found while planning (re-verify, don't trust
  either snapshot): `refwork-d7t.1` is **closed** (2026-07-11) — the 20v
  bead's preflight comment predates that; scorer M1–M4 are **closed** —
  the fast-follow overview predates that.
