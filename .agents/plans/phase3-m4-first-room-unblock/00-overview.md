# Plan: M4 In-VM Bring-Up Is Unblocked — Drive To First Room And M5

Filed 2026-07-02 by the `rom-operator-bridge` project (the Phase 3
validation surface), written for a coding agent working in this repository.

## Why This Plan Exists

Your own readiness audit
(`../guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`)
holds `refwork-d7t.10` BLOCKED on guest-sdk GS-5/GS-6 gaps and missing
operator-run evidence. **The guest-sdk half is now closed** (2026-07-02),
and the hypervisor's framebuffer contract changed the same day. This plan
records what changed upstream, then sequences the work this repo owns to
reach the Phase 3 exit gate items that run through it:

- exit gate 1: M5 determinism suite, 20× zero-flake, including mid-game
  snapshot/restore continuity;
- exit gate 3: a scripted input log plays the first room **in-VM**, driven
  entirely through the worker gRPC API
  (`RestoreSnapshot → InjectInputs → Run → GetFramebuffer` shows the room).

## Bead Mapping

| Plan step | Bead |
|---|---|
| `02-refresh-readiness-evidence.md` | `refwork-d7t.10` (can now leave BLOCKED) |
| `03-image-rebuild-and-ready-snapshot.md` | package-04 refresh + hypervisor handoff |
| `04-first-room-verifier.md` | `refwork-d7t.11` |
| `05-determinism-suite.md` | `refwork-d7t.12`–`.14` |
| `06-verification-and-closeout.md` | `refwork-d7t.15` + bridge verification |

## Before You Start (Repo Hygiene)

- **This plan is untracked** in the working tree, which is currently on
  branch `codex/phase4-corpus-guide` (mid-flight phase-4 corpus work, not
  `main`). Commit the plan first, and decide with the operator whether
  steps 02–06 run on a fresh branch off `main` or here — do not assume.
- **Step 03 has no bead yet** (every other step maps to `refwork-d7t.*`).
  Create one under the epic before starting (e.g. `bd create "Rebuild
  package-04 image + regenerate READY snapshot" -d "…" -p 2 -l impl
  -t task --silent`, then `bd dep add` it under `refwork-d7t`).
- **The bead graph does not encode this plan's sequencing** — `.11`–`.15`
  have no dependency edges on `.10` or each other, so `bd ready` will
  offer later steps prematurely. Add the edges as your first action
  (`bd dep add refwork-d7t.11 refwork-d7t.10`, `.13` on `.11`, `.14` on
  `.13`, `.15` on `.14`, plus edges onto the new step-03 bead).

## Ground Rules Carried Forward

- **Clean-room discipline** (your audit states it; unchanged): evidence
  records revisions, command shapes, hashes, artifact paths — never ROM
  bytes, padlog semantics, framebuffer images, WRAM dumps, or lab goldens.
- The operator supplies the game image; this repo ships no game content.
  The operator-run fields your audit lists as MISSING (owner, ROM BLAKE3,
  padlog BLAKE3) still need a human assignment — flag, don't fabricate.
- Evidence discipline: same artifact-root + BLAKE3 pattern as the
  hypervisor M9 acceptance and guest-sdk Ms4 acceptance
  (`~/git/preestablished/guest-sdk/target/m4-acceptance-20260702T135319Z/`; guest-sdk's own commit/bead records cite the earlier `…T045721Z` run — same config, pre-docs-commit rev).

## Files In This Plan

| File | Contents |
|---|---|
| `01-upstream-state-2026-07-02.md` | What changed in guest-sdk and determinism-hypervisor; stale references to stop chasing |
| `02-refresh-readiness-evidence.md` | Update the audit note; verify GS-5/GS-6 against the real path |
| `03-image-rebuild-and-ready-snapshot.md` | Rebuild package-04 against new guest-sdk; regenerate the READY snapshot |
| `04-first-room-verifier.md` | Implement `refwork-verify vm-first-room` (`refwork-d7t.11`) |
| `05-determinism-suite.md` | M5 double-run + restore-continuity suite, 20× zero-flake |
| `06-verification-and-closeout.md` | Evidence, bead closeout, and the bridge's verification offer |
