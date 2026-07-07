# Requested Work

## Entry Conditions (Hard Gate — Verify Before Opening)

Do not start this request until **all** of:

1. `refwork-gp9` closed — new READY snapshot ref recorded, double-build
   byte-identity re-verified at the build rev. (The
   `BRIDGE_REAL_SNAPSHOT_REF` browser cutover is round-1 closure but not
   a capture prerequisite — captures come from the local lab worker.)
2. `refwork-d7t.11` closed — `refwork-verify vm-first-room` green
   end-to-end against the real worker.
3. `refwork-d7t.12/.13/.14` closed — the M5 green stamp in `dist/`,
   20/20 both legs on the Intel lab runner. (`.15` CI wiring is *not*
   gated on — corpus production doesn't need it.)
4. `refwork-d7t.1` closed **or explicitly deferred with the epic
   owner's sign-off** — don't let a documentation bead strand a
   completed capture set.
5. Operator lab fields on record: ROM BLAKE3, run owner.
6. **Operator hand-play session scheduled**: a hand-played trajectory
   through at least the first boss, plus a credits/late-game
   goal-positive fixture, captured through the in-VM path with a padlog
   (the artifact contract requires these labels; a first-room-only run
   cannot supply them). Fold this into the same lab session as
   conditions 1–3 — that is why this request is filed alongside round 1.

If you're reading this and conditions 1–5 fail, the work item is the
round-1 request. If only condition 6 fails, the recorded fallback is a
**v1 first-room-only corpus** (decode goldens, dedup groups,
volatile/stable pairs) with `phase-4-scorer-golden-artifacts` marked
*partially* fulfilled and the trajectory corpus filed as an explicit
follow-on — say which path you took in the resolution.

## What We Need (Behavioral)

1. **The capture exporter (in scope, own bead).** The runbook's step 3
   is the acknowledged "missing producer": build the export job that
   drives the Phase 3 in-VM capture path (hypervisor
   `CaptureSpec`/`feature_bytes`/`fb_lz4` — the engine-side proof is the
   hypervisor's round-2 request; consume it, don't rebuild it) and
   emits the row contract: `captures/index.jsonl` +
   `artifacts/feature-bytes/` + `artifacts/framebuffer/`, feature bytes
   packed per `layout.json.ranges`, per-row refs/hashes, **≥1,000
   captured states** with framebuffers.
2. **Real feature map + scoring program (in scope).** Author and
   `map-check`-validate the private real-offset feature map and scoring
   program for the operator ROM (runbook step 2). `phase4-layout` then
   proves decode against the real region layout — record layout_version
   and map rev. This is feature-discovery work (real `room_id`/player
   coordinates/etc. addresses), not a rerun of the placeholder files.
3. **Labeled score corpus.** `trace` + `phase4-score-plan` over the
   hand-played trajectory: the tool's mandatory `--first-boss`,
   `--goal-positive`, `--goal-negative` capture ids supplied from the
   condition-6 session; staged-milestone anchors (leave start area,
   first upgrade, first boss) labeled; K=32 batch shape preserved for
   the scorer's exit-gate benchmark.
4. **Bundle + integrity.** `phase4-bundle-check` (including its
   `trajectory/first-boss.jsonl` coverage validation),
   `phase4-checksum-manifest`, `redaction-scan`; private payloads out
   of git; opaque bundle id + hashes recorded.
5. **Close the floors, to their own standard.** Update both
   FULFILLMENT.md files with the full handoff-notes list their
   acceptance requires: private artifact path/registry ref, retrieval
   command + fallback, access group/token owner, retention expectation,
   regeneration + validation commands — not just id and hashes. Record
   the **operator approval** (or private-only disposition) for any
   game/revision metadata, per the scorer-golden blocker.
6. **Freeze it.** Corpus versioned (id + hash manifest + capture count
   + label coverage table); later additions are a new version that
   re-runs `phase4-bundle-check` and gets its own FULFILLMENT addendum
   — never a mutation.

## Suggested Sequencing (Yours To Overrule)

2 (map authorship — can start against round-1's early captures) and 1
(exporter — fixture-testable before the lab session) in parallel while
round 1 executes; then the lab session covers conditions 1–3 + 6 in one
sitting; then 3 → 4 → 5 → 6.

## Acceptance Criteria

1. Both FULFILLMENT.md files updated to their own acceptance standard
   (the handoff-notes list above), with operator approval recorded.
2. `phase4-layout` + `map-check` output proving real-offset decode
   against the real layout — the Phase 4 entry gate's second clause,
   citable.
3. `redaction-scan` green; no private payload tracked by git.
4. Corpus version record durable in-repo: id, hash manifest, capture
   count (≥1,000 or the recorded v1-fallback scope), label coverage
   table including the three mandatory label ids, frozen-forever rule.
5. Downstream smoke handoffs written for state-scorer M1 and
   input-synthesizer M2 (executable by a cold agent with bundle access
   and the `console16-12btn-v1` pad contract).
6. Exporter + feature-map work tracked as beads with close reasons.

## Out Of Scope For This Request

- **Refwork M6** (scoring/goal integration) — gated on scorer M2–M3;
  this corpus unblocks that chain.
- The capture *engine* (hypervisor's round-2 request) — consume its
  proof; file defects to them.
- Creating the state-scorer / input-synthesizer repos — a program-level
  Phase-0 gap (`phases/README.md` matrix: scorer P0 = `wksp`, synth
  P0 = M0) owned by nobody today; flagged in the round-2 work-order
  note, not tracked in any repo request.
- Round 1 itself.
