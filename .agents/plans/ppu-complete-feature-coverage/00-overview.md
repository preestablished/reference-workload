# PPU complete-feature coverage plan

## Objective

Fix the recorded failure at frame 77,465:

```text
UnimplementedPpuFeature { reg: 51, value: 64 }
```

Register `51` is `$2133` (`SETINI`) and value `64` is `$40`, the Mode 7
`EXTBG` enable. The implementation must render EXTBG semantics, not merely
silence the fault. To prevent the same stop-and-fix loop on later gameplay,
this plan also closes every other deliberate PPU feature fault currently in
`refwork-emu`.

## Current evidence

- `Ppu::write(0x33, 0x40)` faults immediately because the SETINI mask includes
  bit 6, despite the nearby comment saying EXTBG should only matter in Mode 7.
- BGMODE accepts only modes 0, 1, 3, and 7; modes 2, 4, 5, and 6 return
  `UnimplementedBgMode`.
- CGWSEL direct-color bit 0 returns `UnimplementedPpuFeature`.
- SETINI screen interlace, OBJ interlace, overscan, pseudo-hires, and EXTBG
  return `UnimplementedPpuFeature`.
- Existing PPU tests pass (`cargo test -p refwork-emu`: 180 tests total), but
  the test suite currently asserts that the missing features fault.
- The private recording preserved inputs through frame 77,464. Replay can use
  that prefix plus the script engine's documented hold-last behavior for the
  unrecorded failing frame. The private ROM and pad-log paths remain outside the
  repo and are supplied through environment variables.

## Scope

The required lane implements all currently faulting PPU functionality:

1. Mode 7 EXTBG, including BG2 palette-bit priority and BG1/BG2/OBJ ordering.
2. BG modes 2, 4, 5, and 6, including their bpp combinations and priority.
3. Offset-per-tile behavior used by modes 2, 4, and 6.
4. Direct color for BG1 in modes 3, 4, and 7.
5. Pseudo-hires and the native-hires behavior of modes 5 and 6, normalized to
   the repository's fixed 256-pixel published framebuffer.
6. Screen interlace, OBJ interlace, and overscan, normalized to the fixed
   256x224 framebuffer and fixed-frame public contract.
7. Harmless SETINI external-sync/reserved bits remain latched and non-faulting.

This is scanline-level emulation consistent with the existing core. It does not
turn the PPU into a dot/cycle-accurate implementation, change the public
framebuffer geometry, or add sprite range/time overflow limits. Those are
accuracy enhancements rather than current `Unimplemented*` fault paths.

## Non-negotiable constraints

- Preserve D1-D9: single-threaded, no host time/RNG/floats, allocation-stable
  frames, deterministic integer rendering, and loud faults for true anomalies.
- Do not commit, print, hash into source artifacts, or otherwise expose the
  private ROM, pad payload, screenshots, or framebuffer dumps.
- Keep `FB_WIDTH=256`, `FB_HEIGHT=224`, `FB_STRIDE=1024`, and `FB_BYTES`
  unchanged. Compatibility policy belongs inside the renderer.
- Reuse the existing `Ppu`, window, color-math, sprite, CGRAM, and VRAM state;
  do not vendor a second emulator core.
- Every accepted register/mode needs a behavioral test. Replacing a fault with
  an untested ignore is not implementation.

## Completion definition

Completion requires all of the following:

- No PPU register value or BGMODE value can produce either
  `UnimplementedPpuFeature` or `UnimplementedBgMode`.
- Focused tests prove the expected pixels/priority/coordinate behavior for each
  feature listed above.
- The recorded private prefix plus hold-last input replays past frame 77,465
  with no fault and completes the explicitly requested verification length.
- Default and introspection builds pass, as do formatting, clippy, deny,
  determinism, and zero-allocation gates available locally.
- A final search and fault-injection test demonstrate that no deliberate PPU
  feature-fault site remains.
