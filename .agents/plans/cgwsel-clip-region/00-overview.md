# CGWSEL Clip-Region Swap + Window Semantics Fixes (Plan)

## Outcome

The story cinematic's picture band shows the full animated scene (sky,
clouds, grass, character silhouettes) instead of a black center with edge
slivers. Two latent window-path defects land in the same fix (open epoch).

## Diagnosis (proven by experiment; investigator report 2026-07-21)

Operator session `discovery-01` (2,172 frames, "problem frame" dump at
f1552) reproduces exactly in replay: letterbox + text + two sky/grass edge
slivers, black center, static f1500-1650. Register truth at f1552:

- CPU (per frame, vblank): W1 = [WH0=0x2F, WH1=0xCF] (screen center),
  WOBJSEL=0x33 → color-math window = W1 enabled + INVERTED (covers the two
  edge strips). Positions are static by design — the "curtain" at this
  point is not window animation (H1 refuted).
- HDMA ch7 per band line (47-158): TMW/TSW=0x15, CGWSEL=0x80, CGADSUB=0.
- CGWSEL=0x80 → clip region (bits 7:6) = 2 = hardware "force main black
  INSIDE the color window" = the edges → center picture visible.

**Defect**: `Ppu::composite_pixel` (crates/refwork-emu/src/ppu/mod.rs:1290-1296
at 0c9368c) has the middle encodings swapped — `1 => in_clip_window != 0,
2 => in_clip_window == 0` — i.e. 1 treated as clip-inside, 2 as
clip-outside. Hardware (fullsnes, Mesen2 ColorWindowMode): 0=never,
1=clip OUTSIDE the color window, 2=clip INSIDE, 3=always. (Asymmetric vs
bits 5:4, the math-enable region 0=always/1=inside/2=outside/3=never —
which our code implements correctly; only the clip field is inverted.)
We therefore clip the center to black. The same wrong encoding appears in
two comments (the `$2130` write handler ~:836 and the composite_pixel doc
~:1265-1268). Experiment: swapping the two arms renders the fully-open,
animating scene at f1450/1552/1650, letterbox and text intact, no faults
across the whole replay.

## Also landing (latent, same path, same epoch — fix now or pay a future
## epoch cut)

1. **`window_range_mask` wrap semantics** (mod.rs:45-62): `left > right`
   currently implements a wrapped active-outside-the-gap window; hardware
   treats it as an EMPTY window (no pixels inside). The unit test
   `window_range_mask_wrapped` (~:3406) codifies the wrong behavior and is
   updated to pin empty-window semantics.
2. **Half-math on clipped pixels** (mod.rs ~:1338): the halving condition
   lacks `&& !clip_main`; hardware disables halving when the main pixel
   was clipped to black (bsnes windowAbove gate).

Neither is exercised by this scene; both are real hardware deviations in
the exact code being touched.

## Constraints

- Behavior-changing (fb hashes; host icount negligible): rides the still-
  open 2026-07-16 epoch (refwork-1n8 pending; already noted for ≥0.2.1
  hashes). EMU_VERSION → 0.2.2 with a doc line, same epoch.
- Guest-invisible (pure compositing; no timing/CPU-visible state) — the
  operator's f1552 WRAM dump must byte-match the fixed replay.
- Clean-room: no game name/tile data in repo files or tests.
- Working tree carries TEMP-DIAG introspect accessors (3 files, ~65
  lines) used for verification; the landing diff excludes them.

## Files

| File | Content |
|------|---------|
| `01-implementation.md` | Exact edits, tests, verification |
