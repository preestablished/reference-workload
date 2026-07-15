# Dual-review decisions

Two independent agents reviewed the initial six-file plan before implementation:
one for PPU hardware semantics and one for repository architecture, determinism,
privacy, and acceptance evidence.

## Accepted corrections

1. Overscan is a timing mode, not only a framebuffer projection. The plan now
   requires a frame-latched 225/240 vblank boundary across PPU, bus, core, NMI,
   HVBJOY, auto-joypad, OAM, HDMA, and rendering, with an exact 7/8 crop.
2. The field alternates even in progressive mode and STAT78 reports it. Only
   modes 5/6 double BG source Y for interlace; OBJ interlace has independent
   coverage/source-row rules and documented size quirks.
3. Existing window selector enable/invert decoding must be corrected before
   EXTBG relies on BG2 windows.
4. Mode 7 now specifies shared scroll-latch state, signed clipping, product
   truncation, and repeat-mode-3 tile-zero semantics.
5. Hires now specifies character selection, scroll/flip phase, native coordinate
   mapping, and XRGB rounding order, with OBJ/window/mosaic/color-math tests.
6. OBJ color math is restricted to palettes 4-7.
7. Offset-per-tile and Mode 7 direct-color boundary cases are explicit.
8. Fault sweeps cover every BGMODE byte and render combinations, while public
   fault variants remain for downstream API compatibility.
9. The private verification is correctly described as a recorded prefix plus
   hold-last. It uses environment variables and a fault-aware double-run verifier
   rather than `ramdiff record`, and no concrete private path is in the plan.
10. Golden non-private output protects against deterministic renderer regressions;
    an all-feature counting-allocator test supplements the existing mode-1 gate.
11. The local gate list is derived from CI and cross-architecture proof remains
    an actual CI requirement.

## Rejected correction

The architecture reviewer proposed moving pseudo-hires, overscan, OBJ interlace,
and screen interlace to SETINI bits 5, 4, 3, and 2. That is inconsistent with the
hardware `EX..HOiI` layout used by the Nintendo register documentation and the
ares implementation: pseudo-hires is bit 3, overscan bit 2, OBJ interlace bit 1,
and screen interlace bit 0; bits 5/4 are reserved. The original bit positions
remain, with single-bit tests required.

## Implementation authority

Files `00` through `06` together are the reviewed implementation plan. Where the
initial overview conflicts with this decision record, the corrected detailed
semantics and test matrix control. Implementation results belong in
`07-implementation-results.md`.
