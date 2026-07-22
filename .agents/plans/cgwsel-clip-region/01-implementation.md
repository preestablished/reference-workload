# Implementation — CGWSEL Clip Region + Window Fixes

All in `crates/refwork-emu/src/ppu/mod.rs` (+ EMU_VERSION in lib.rs).
Line refs are for clean 0c9368c.

## Edit 1 (the bug): composite_pixel clip-region arms (:1290-1296)

```rust
let clip_main = match clip_region {
    0 => false,
    1 => in_clip_window == 0, // clip OUTSIDE the color window
    2 => in_clip_window != 0, // clip INSIDE the color window
    3 => true,
    _ => false,
};
```

Correct the stale comments: the `$2130` write-handler comment (~:836,
which also wrongly claims bits 5:4 use the "same encoding"), the
composite_pixel doc block (~:1265-1268), and the `compute_window_masks`
doc (~:895-899, says bit0=enable/bit1=invert; implementation and hardware
are bit0=invert/bit1=enable). Hardware encoding for bits 7:6: 0=never,
1=clip outside, 2=clip inside, 3=always; asymmetric vs math-enable bits
5:4 (0=always/1=inside/2=outside/3=never). Clipped pixels are literal
(0,0,0) — already correct at :1299-1301.

## Edit 2: window_range_mask empty-window semantics (:45-62)

`left > right` → return an all-zero mask (empty window), replacing the
wrapped implementation. Also fix the doc comment at :41-43 ("inclusive,
wrapping u8"). Update `window_range_mask_wrapped` (~:3399-3407) to pin
empty-window semantics (rename e.g. `window_range_mask_degenerate_is_empty`);
keep `window_range_mask_normal` unchanged. Blast radius note:
`window_range_mask` feeds ALL window masks via `compute_window_masks`
(:907-908) — BG1-4 and OBJ layer masking AND the color window — so this
edit reaches every scene using degenerate windows on any layer (no in-repo
test/fixture depends on the wrapped behavior; verified by review).

## Edit 3: half-math suppressed on clipped main pixel (~:1338)

Add `&& !clip_main` to the halving condition (order: compute clip_main
first; verify no borrow/order conflict in the actual code).

## Edit 4: EMU_VERSION → "refwork-emu 0.2.2" (lib.rs), doc line:
`0.2.2 = CGWSEL clip-region + window semantics (same 2026-07-16 epoch)`.

## Tests (in-crate, synthetic)

1. Clip-region encoding, via the PUBLIC register path (mirror
   `ppu_cgwsel_fixed_color_ignores_enabled_subscreen_layer` at :3444):
   `make_ppu()`, `p.write` brightness/WH0/WH1/WOBJSEL/CGWSEL, opaque BG
   pixel, `render_scanline`, assert HARDCODED framebuffer bytes for
   pixel-inside and pixel-outside × regions 1 and 2 (the discriminating
   arms; 0/3 behave identically old vs new). Expectations must be written
   as literal bytes — deriving them from any mirrored `match` is
   forbidden (tautology). Execute the must-fail check once by temporarily
   reverting Edit 1.
2. Degenerate window is empty (updated test, Edit 2).
3. Half-math not applied to clipped pixels: the clipped main pixel MUST
   be a non-backdrop opaque layer pixel (the math branch is gated
   `do_math && !main_is_backdrop` at :1324 — a backdrop pixel never
   reaches halving and the test would be vacuous). Concretely: opaque BG1
   tile + CGADSUB half|enable + fixed-color sub operand + clip-always;
   old code halves (visible non-black), new code yields the un-halved
   hardware result. Hardcoded byte expectations, same seam as test 1.
4. Full `-p refwork-emu` matrix + clippy + deny + 600f determinism gate.

## Verification

- Replay the CURRENT `discovery-01` (the 2,172-frame operator session,
  NOT the 45,230-frame canonical): f1450/1552/1650 render the open
  animated picture (compare with investigator's windiag-fix renders);
  letterbox + text intact; no faults; `problem_frame.bin` (f1552)
  byte-matches replay WRAM (guest-invisible gate; triage as before).
- Extended replay gates (Edit 1 touches every CGWSEL-clip scene, Edit 2
  every degenerate window on any layer):
  (a) the 3,850-frame title/attract session (discovery-01.bak-9 era):
  fault-free, WRAM byte-match at its 16.. dump points if present or
  end-state; spot renders — one title frame, one gameplay HUD frame from
  the f2600-era color-math region (cgadsub=03) — identical to pre-fix
  renders or explained.
  (b) the canonical 45,230-frame session (identify by frame/dump count):
  fault-free replay; hash-chain vs pre-fix expected to MISMATCH (behavior
  changed — not a failure); pass = zero faults + 3-5 sane spot renders.
  Front-loads work refwork-1n8 owes anyway.
- Operator live check: play to the cinematic; curtain opens into the
  scene.

## Landing

- COMMIT-FIRST procedure (the old reset dance wiped a fix once):
  (a) `git stash push -m temp-diag -- crates/refwork-emu/src/ppu/mod.rs
  crates/refwork-emu/src/bus.rs crates/refwork-emu/src/core_impl.rs`;
  (b) implement Edits 1-4 + tests on the clean tree, run gates, COMMIT;
  (c) `git stash pop` to restore diag on top for introspect replay
  verification (conflicts land in regenerable diag hunks only — the fix
  is already in history); (d) discard diag when done.
- Single commit: fixes + tests + comments + version. Bead refwork-8fp.
- Note in refwork-1n8: epoch hashes now ≥0.2.2.
- Out of scope, filed as its own bead: backdrop color math is dead at
  HEAD (CGADSUB bit 5 never applies — the `!main_is_backdrop` gate at
  :1324 blocks it; hardware applies math to backdrop). Real deviation,
  larger blast radius, deliberately not in this landing.
- m6 checkout: pull + release rebuild.
