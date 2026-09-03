# Decision: discovery-02 supersedes discovery-01; recording freeze lifted

Date: 2026-09-03. Decided by Matt (operator).

**Status: ACCEPTED 2026-09-03.** Operator confirmation ("use it", in
session) recorded as the `freeze lifted 2026-09-03: …` line in
`$PR/evidence/discovery-02-epoch.txt`. Drafted by the coding agent per
`~/.agents/projects/reference-workload/plans/discovery-02-recording-readiness/`
(package 01 §1.2).

## Decision

- discovery-01 (45,230 frames, 16 dumps, recorded before the 2026-07-16
  APU-clock epoch cut; now at `$PR/ramdiff/discovery-01.bak-6`) is
  **superseded** by the discovery-02 recording set (`discovery-02-main`,
  `discovery-02-gameover-death`, `discovery-02-gameover-timer`).
- Epoch-cut protocol rollout step 6
  (`.agents/plans/audio-fidelity-and-apu-clock/03-epoch-cut-protocol.md`:
  migrate discovery-01, regenerate its 16 dumps, re-freeze) is **dropped**.
- The 11 known offsets (game_mode, room_id, area_id, player_x, player_y,
  health, max_health, upgrade latch, midboss latch, boss latch, stage_clear)
  are **re-confirmed on discovery-02** by the processing plan
  (`discovery-02-processing-and-interim-scoring`) instead of being carried
  over by replay.
- The discovery-02 **recording freeze is lifted** at `EMU_VERSION`
  `refwork-emu 0.2.3` (recorded with the build commit in
  `$PR/evidence/discovery-02-epoch.txt`). Every discovery-02 session is
  stamped with that version and the ROM's BLAKE3 in its `session.yaml`
  (`ramdiff` package 01 of the plan) so a build change is detectable.

## Reason

- The user's request needs events discovery-01 does not contain: score
  brackets, a game over by death, a game over by level-time expiry, and the
  W1-S4 boss defeat leading into W2. A migration of discovery-01 would not
  supply them; a new recording is needed regardless.
- Protocol step 6's own caveat applies: a pre-epoch padlog replayed under
  the new APU/PPU timing may **semantically derail** (the inputs are the
  same, the game state they produce is not guaranteed to be). Re-confirming
  offsets against a fresh, stamped recording is cheaper and sounder than
  validating a replayed one.
- Everything downstream (feature map v3, scoring program v3, the gate-3
  trajectory, state-scorer fixtures) is rebuilt from discovery-02 anyway.

## What stays open

Bead `refwork-1n8` is re-scoped, not closed. Still owed there: verify/fix
the `integration.rs` frame_ctr pin, package-06 re-stamp, operator lab
expect regeneration, sibling icount re-baseline, doc updates for old
corpus-id references. The canonical discovery-01 at `.bak-6` is kept
(never deleted) as the offset re-confirmation source and is pinned by
`$PR/evidence/discovery-01-canonical.txt`.

Precedent: `.agents/decisions/2026-07-16-apu-clock-epoch-cut.md`.
