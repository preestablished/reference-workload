# Plan: Close The M4 First-Room Gate And Stamp M5

Plan filed 2026-07-07, addressing
`.agents/requests/phase3-m4-first-room-gate-and-m5-stamp/`. Written for a
coding agent working in this repository.

## Relationship To Prior Plans

This plan **continues** `.agents/plans/phase3-m4-first-room-unblock/` — it
does not replace it. The tooling steps of that plan (04, 05 staged-fixture
halves) are DONE; what remains is the lab chain: rebuild → READY regen →
cutover → first-room → M5 20× stamp → closeout, plus the M2 paper-trail
bead. Where a step here says "per the prior plan", read that plan's file
for the full procedure; this plan records only what changed and what's
left.

## State As Verified 2026-07-07 (do not re-derive; re-verify only if stale)

- `main` is at `1295414`; the three commits past `2ea42ad` are
  request-docs only, so "build from current main" and the request's
  evidence base (`2ea42ad`) describe the same code.
- All upstream dependencies are green: guest-sdk Ms4, the boot-scheduling
  deadlock (first real emulator+game READY on the real worker 2026-07-05,
  `.agents/requests/phase3-ready-not-emitted-real-worker/04-verification.md`),
  hypervisor M9/capture/D7, snapshot-store M7 GC, and the emulator
  black-screen chain (`84933d9`, `8eff8d9`, `2ea42ad`).
- **The request's "bead hygiene" step is stale.** The edges it says are
  missing (`.12→.11`, `.13→.12`, `.14→.13`, `.15→.14`, `.11→gp9`) all
  exist in `bd dep tree` today. Step 01 verifies rather than adds.
- The branch-reconciliation question is moot: all prior work branches are
  ancestors of `main`. Build from `main`.
- `dist/workload-image-0.1.0/determinism.unstamped.yaml` still pins
  `git_rev: 84933d9` and defers the stamp — replacing it with a green
  stamp is step 04's deliverable.
- **The workspace does not currently compile with the `mock` feature**:
  `crates/refwork-dh-client/src/mock.rs:529` is missing the
  `build_profile` field that the sibling `dh-proto` added to
  `GetWorkerInfoResponse`. Step 01 fixes this before anything else runs.
- `image/guest-sdk.lock` was already bumped to `487ff564` on 2026-07-05
  (commit `667ca8b`), which includes the boot-scheduling deadlock fix —
  older bead comments citing pin `c03e90b` are stale.

## Bead Map And Sequencing

| Step | File | Bead(s) | Depends on |
|---|---|---|---|
| 01 | `01-preflight-and-operator-ask.md` | (hygiene) | — |
| 02 | `02-image-rebuild-and-ready-snapshot.md` | `refwork-gp9` | 01 |
| 03 | `03-first-room-in-vm.md` | `refwork-d7t.11` | 02 + operator cutover |
| 04 | `04-m5-suite-and-green-stamp.md` | `refwork-d7t.12/.13/.14` | 03 |
| 05 | `05-ci-and-closeout.md` | `refwork-d7t.15` | 04 |
| 06 | `06-m2-paper-trail.md` | `refwork-d7t.1` | soft on 03 — fuller evidence if run after it (see file) |

Steps 02→03 are designed as **one lab session** with the bridge team on
the cutover (their offer: `03-verification-offer.md` in the request dir;
same-day windows normally fine). Step 04 follows immediately on the same
snapshot. Steps 05 and 06 interleave as cleanup.

## Hard Boundaries (operator-gated — flag, never do autonomously)

1. **Never write `BRIDGE_REAL_SNAPSHOT_REF`** or touch the private
   handoff env channel — it restarts the live bridge/worker and
   invalidates slot leases (`rom-operator-bridge-72o`). Hand the new
   snapshot ref to the bridge team and let them execute the cutover.
2. **Never point tooling at `/run/dh/grpc.sock`** — that UDS belongs to
   the deployed worker. Launch a local worker on a scratch socket for all
   suite runs (procedure in step 02).
3. **Lab fields no agent can supply**: operator ROM BLAKE3, first-room
   padlog BLAKE3, run owner. Step 01 consolidates these into a single
   early ask; do not fabricate and do not let them trickle out.
4. **Clean-room discipline**: evidence records revisions, command shapes,
   hashes, artifact paths — never ROM bytes, padlog semantics,
   framebuffer images, WRAM dumps, or lab goldens.
5. `~/git/preestablished/.dh-clean-ff1e88c` is the deployed worker's
   build tree — **read-only**. Use a fresh pinned scratch worktree
   (a prior session used `.dh-clean-4c44263` as the pattern).

## Acceptance Criteria (from the request, verbatim mapping)

1. New READY snapshot ref recorded; double-build byte-identity
   re-verified at the new rev → step 02.
2. First-room evidence artifact filed + bridge browser-side confirmation
   → step 03.
3. M5 green stamp in `dist/` (20/20 both legs, Intel runner, negative
   demonstrated); `.12/.13/.14` closed; `xtask image --register` refuses
   without a fresh stamp and manifest `determinism.last_green` populated
   — or both explicitly deferred to `.15` with recorded reason → step 04.
4. CI real-worker legs (or recorded reason they stay lab-manual);
   guest-sdk handoff updated → step 05.
5. `refwork-d7t.1` closed with extended `m2-floor-evidence.md` as
   evidence → step 06.

## Out Of Scope

M6 scoring/goal integration (Phase 4), emulator performance work (see
`.agents/requests/emulator-perf-profiling-first/`, separate), and the
hypervisor-side test-cap retune (filed against that repo).
