# Backdrop Color Math (Plan)

## Outcome + diagnosis (proven 2026-07-22)

The cinematic's one-line full-width sky stripe at the picture band's bottom
edge (operator session `discovery-01`, bad_frame.bin at f2126) is the
already-filed latent `refwork-dxr` materializing: those boundary lines are
blacked on hardware by SUBTRACT color math applied to the **backdrop**
($2131=0xa1 subtract enable incl. backdrop bit5, COLDATA staged white
0x7FFF; the "sky" is CGRAM color 0 via HDMA ch1, not a BG layer). Our
compositor gate `if do_math && !main_is_backdrop`
(crates/refwork-emu/src/ppu/mod.rs:1319 at 55d7647) categorically excludes
backdrop pixels from math; hardware applies math to backdrop per CGADSUB
bit5. All five call sites already pass `cgadsub & 0x20` as math_enabled
for backdrop — only the gate is wrong. All HDMA-phase hypotheses were
killed with per-line schedule reconstruction (our reload semantics match
snes9x's exactly; the inter-channel skew is intentional in the game's
tables; our whole schedule sits one uniform invisible line early).

## Fix

`ppu/mod.rs:1319`: `if do_math && !main_is_backdrop` → `if do_math`,
PLUS the mandatory accompaniment (both reviewers): `main_is_backdrop`'s
only remaining consumer is the introspect diag block, so add
`#[cfg(not(feature = "introspect"))] { let _ = main_is_backdrop; }` (the
:1706 in-file precedent) or default-feature clippy -D warnings fails CI.
Operand plumbing is already correct (verified: sub-transparent→COLDATA
fallback, halve gates, clip ordering all hardware-correct for backdrop).
EMU_VERSION → 0.2.3 (same 2026-07-16 epoch, doc line).

## Tests

- New (public register path, hardcoded bytes, :3444-style seam; TM=$00 so
  the picker falls to backdrop): (1) positive — red backdrop (CGRAM 0 via
  $2121/22), COLDATA white, CGWSEL=0x00, CGADSUB=0xA0 subtract|backdrop →
  black; (2) negative — CGADSUB=0x80 (bit5 clear) → red unmodified;
  (3) half pin (reviewer-required) — CGADSUB=bit5|0x40 add, fixed-color
  operand → asserted HALVED backdrop result (guards the halve semantics
  snes9x confirms: backdrop halves like any layer). Must-fail check
  executed once against the old gate.
- Existing matrix (default / audio+introspect), clippy, deny, 600f
  determinism.

## Verification

- Replay current discovery-01: f2126 render has zero non-black-margin
  rows (stripe gone); bad_frame.bin byte-matches WRAM (proven pre-emptively
  by the investigator; re-confirm on the landing build).
- Prior session f1552: same stripe (128 px, rows 158-159) disappears —
  expected change; WRAM byte-match holds.
- bak-9 spot renders (title f1500, HUD f2600): expected byte-identical
  (gameplay CGADSUB=0x03, bit5 clear).
- Canonical 45,230-frame fault-free smoke + spot renders.
- Operator live check.

## Landing

Commit-first: stash ALL uncommitted (TEMP-DIAG + TEMP-HACK), implement
fix+tests clean, gates, commit; replay verification via a minimal
public-API replayer (no diag needed); push; refwork-1n8 note (hashes
≥0.2.3); close refwork-s38 + refwork-dxr; m6 rebuild.
