# Verification And Handoff Shape

## Phases-Track Verification

On your resolution we will:

1. re-run `phase4-bundle-check` + `phase4-checksum-manifest` +
   `redaction-scan` from a clean checkout against the recorded bundle id
   and confirm hashes match the FULFILLMENT records;
2. check the label coverage table against the staged-milestone anchors
   (start-area / first-upgrade / first-boss present, goal and no-goal
   examples both present);
3. confirm the freeze rule is recorded and the corpus version id appears
   in both FULFILLMENT.md files and the bundle manifest — one id
   everywhere.

## Downstream Handoff

The corpus's consumers don't exist as repos yet (state-scorer,
input-synthesizer). The handoff is therefore paper-first: the two
FULFILLMENT.md records plus the downstream-smoke instructions
(acceptance item 5) are the interface those repos open against. When
they are instantiated, their first request will cite this request dir —
write the smoke instructions to be executable by a cold agent with only
bundle access and the pad contract (`console16-12btn-v1`).

## Choreography

- **Predecessor:** `phase3-m4-first-room-gate-and-m5-stamp/` (round 1,
  this repo) — produces every input this request consumes. Single lab
  session ideally covers round-1 steps 1–3 *and* this request's capture
  export, with the operator briefed on both up front (`01-`, operator
  inputs section).
- **Sibling:** the determinism-hypervisor round-2 request (filed today)
  covers the capture *mechanism* side (long-Run survivability after the
  OOM fix, CaptureSpec end-to-end). Division: they prove the engine,
  you produce and package the corpus. If a capture-engine defect
  surfaces while producing the corpus, file it to them — don't absorb
  it here.
- **Bridge:** the `q63` capture-export path in rom-operator-bridge is
  landed and can serve as a second capture route for corpus rows if the
  direct harness route stalls; their round-2 request stages the
  corresponding capture smoke.

## Handback Shape

Append `04-resolution.md` here: git SHAs, bundle id + hash manifest,
label coverage table, FULFILLMENT diffs, smoke-instruction locations.
We respond with `05-verification.md` after the checks above.

## Contact / Tracking

- Project-level requests this fulfills:
  `~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/`
  and `pad-alphabet-and-phase4-context-fixtures/`.
- Runbook: `docs/phase4-corpus-guide/`.
- Gate tracker: the round-1 beads per `02-`'s entry conditions —
  `refwork-gp9`, `refwork-d7t.11/.12/.13/.14`, and `.1`
  (closed or sign-off-deferred). `.15` (CI wiring) is deliberately
  *not* a gate for corpus production.
