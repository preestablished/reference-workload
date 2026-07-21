# HDMA Mid-Frame Enable Fix (Plan)

## Outcome

The example game's story cinematic (letterboxed panels, per-scanline
palette gradient, typewriter text) renders correctly in
`ramdiff record --interactive` instead of a solid blue screen. Root cause
is proven; the fix is small and rides the still-open determinism epoch
window.

## Diagnosis (proven by experiment)

Operator recording 2026-07-21 (session `discovery-01`, 1,608 frames, WRAM
dump "the bad place" at frame 1364) reaches the story cinematic live. At
frame 1364 the screen is solid backdrop blue; reference behavior (OpenEmu)
shows a letterboxed cinematic.

PPU/DMA state at f1364 (via the introspect scaffold, replay):

- OAM holds dozens of on-screen sprites with uploaded tiles; BG state sane.
- The game drives the scene with three HDMA channels: ch1 → $2121/$2122
  (per-scanline CGRAM writes: gradient), ch5 → $212C/$212D (per-scanline
  TM/TS: the letterbox band), ch7 → $212E..$2131 (windows + color math).
- An H/V-IRQ at (v=0x24, h=0x98) **enables HDMA mid-frame**
  (`$420C := 0xa2` at line 36); NMI disables it at line 225. `$420C` is
  **0x00 at every frame start**.

The defect (`crates/refwork-emu/src/bus.rs`): the `$420C` write handler
only stores the mask (`self.hdmaen = value;`, ~:1290). A channel's
`hdma.state[ch].active` is set **only** by `init_hdma()`, which
`core_impl.rs` calls at frame start (line 0). With the mask 0 at init and
set only mid-frame, the channels never become active — `execute_hdma`
skips them (`!state.active`) for the whole session. All three raster
effects no-op; only the backdrop color remains: the solid blue screen.

**Experiment** (working-tree hack, marked "do not commit"): initializing
newly-set channels at the `$420C` write (A1T→A2A copy + first table-entry
load) makes frame 1364 render the actual cinematic — letterbox panels and
the story text, matching the reference. Root cause confirmed end-to-end.

## Constraints

- Determinism: integer-only, no new deps; the fix changes framebuffer
  hashes and host icount (the channels now do work) → **behavior-changing**
  — but guest-visible timing is UNCHANGED (refwork's HDMA does no cycle
  accounting), which is why WRAM-level replay compatibility is expected.
  The 2026-07-16 APU epoch window is still open (`refwork-1n8` migration
  of the canonical 45,230-frame session not yet run), so landing now adds
  zero extra epoch cost. Note the fix in the epoch rollout bead; bump
  EMU_VERSION to 0.2.1.
- Hardware grounding: **settled by dual review** — snes9x's `$420C` is
  `PPU.HDMA = Byte & ~PPU.HDMAEnded` (activate-without-init; games stage
  A2A/NTRL themselves via the writable $43x8-A, which refwork implements).
  The experiment's init-at-enable is the WRONG general semantics (would
  break resume-dependent titles) and is discarded; see 01.
- Clean-room: no game name, no tile/OAM data in repo files or tests.
- Working tree currently carries the introspect scaffold + the experiment
  hack; the final diff must contain the production fix + tests only (the
  scaffold stays uncommitted or is proposed separately with its clean-room
  constraints re-reviewed).

## Files

| File | Content |
|------|---------|
| `01-implementation.md` | Fix design, tests, verification gates |
