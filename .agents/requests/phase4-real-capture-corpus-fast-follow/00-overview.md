# Request: The Real-Capture Golden Corpus — Phase 4's Entry Ticket (Fast-Follow, Gated)

## Who Is Asking

The phases track, on behalf of Phase 4's consumers: the state-scorer
(M1 golden tests) and input-synthesizer (M2 macro packs) work — repos
not yet instantiated — and the two project-level Phase 4 requests
already filed against this repo. Filed 2026-07-07, round 2.

## Standing Relative To Round 1 — Read This First

This request is **explicitly gated** on the round-1 request in this repo
(`phase3-m4-first-room-gate-and-m5-stamp/`), which is unexecuted as of
this filing (HEAD *is* its filing commit; `bd ready` still shows exactly
`refwork-gp9`). Round 1 is the priority; do not open this one until its
entry conditions (below, `02-`) hold. It is filed now, not later, for one
reason: the corpus is a *fast-follow* — the lab session that closes
round 1 captures operator fields (ROM BLAKE3, padlog BLAKE3, run owner)
this request reuses directly, and knowing that while running round 1
avoids a second operator round-trip.

## Why reference-workload, Why This Chunk

Phase 4's entry requirement is verbatim this repo's deliverable: "real
RAM/framebuffer captures from the in-VM emulator (golden-test corpus),
and the demo feature map exercised against the real region layout"
(`phase-4-scoring-and-inputs.md`). And the work is unusually well-staged:

- **The validation/packaging tooling is done and locked.**
  `refwork-verify` ships `phase4-private-intake`, `phase4-layout`,
  `trace`, `phase4-score-plan`, `phase4-bundle-check`,
  `phase4-checksum-manifest`, `redaction-scan`
  (`crates/refwork-verify/src/main.rs` :97–:105, sources in
  `phase4_*.rs`; 20 phase4 tests in
  `crates/refwork-verify/tests/integration.rs`). The operator runbook
  exists (`docs/phase4-corpus-guide/`, 5 steps).
- **Both project requests are stuck on one floor.**
  `~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`:
  "not fulfilled," blocked on the Real Capture Evidence Floor — plus
  operator approval *for any public release of private game/revision
  metadata* (approval gates publication, not fulfillment).
  `pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`: "partially
  fulfilled" — the pad contract (`console16-12btn-v1`) is done; the
  live context fixture waits on the same floor.
- **Most of the pipeline is built — but not all of it.** The validation/
  packaging tools are done and locked; the runbook's own step 3 calls
  the capture *exporter* "the main missing producer step," the real-
  offset feature map must still be authored (placeholders are
  contractually disqualified), and the label contract needs a
  hand-played trajectory past the first boss that round 1's first-room
  run never produces. Those three fronts are in scope here — `02-`
  prices them honestly.

What this is **not**: refwork M6 (scoring/goal integration) stays out of
scope — the phase doc gates it on scorer M2–M3, which is gated on this
corpus. Don't invert the chain.

## The Ask In One Paragraph

Once round 1 closes and the operator hand-play session (entry
condition 6) is scheduled: build the capture exporter the runbook says
is missing, author the real-offset feature map + scoring program, then
run the pipeline — `phase4-private-intake` → `phase4-layout` →
`trace`/`phase4-score-plan` (hand-played trajectory labels: first-boss,
goal-positive, goal-negative; ≥1,000 captures) → `phase4-bundle-check`
+ `phase4-checksum-manifest` + `redaction-scan` — producing the
validated lab-private corpus bundle; close both FULFILLMENT floors to
their own acceptance standard (retrieval/access/retention handoff notes
+ operator approval); and freeze the corpus so state-scorer M1 and
input-synthesizer M2 start against a fixed target. A recorded
first-room-only v1 fallback exists if the hand-play session can't
happen (`02-`).

## Files In This Request

| File | Contents |
|---|---|
| `01-current-state.md` | Evidence: tooling inventory, the two stuck FULFILLMENTs, what round 1 produces |
| `02-requested-work.md` | Entry conditions (hard gate), the pipeline run, acceptance criteria, out of scope |
| `03-verification-offer.md` | Phases-track verification and the downstream-consumer handoff shape |
