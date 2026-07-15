# Implementation architecture

## 1. Make background pixels carry their source

In `ppu/bg.rs`, replace the current implicit `cgram_idx`-only assumption with a
compact pixel description containing:

- raw color index (transparency),
- palette/CGRAM index,
- tile palette group (for direct color),
- priority bit,
- layer id or enough context for the compositor to apply direct color.

Add reusable 2/4/8bpp planar readers and a coordinate-based tile fetch helper.
The current bespoke mode-3 reader should become the 8bpp case of that shared
path. Keep all buffers fixed-size and stack/`Ppu::new` allocated.

## 2. Replace per-mode duplicated selection with tables

Introduce a small internal mode descriptor and priority-rank function. Render
active BG lines according to the descriptor, then select a `LayerPixel` for
main and sub screens with one common function. Preserve the mode 0/1 output via
existing tests plus new parity tests before deleting old mode-specific paths.

Use the common flow for modes 0-6:

1. render each active BG into a fixed line buffer;
2. render OBJ once;
3. apply layer windows and screen enables;
4. select the highest-ranked main and sub pixels;
5. resolve CGRAM/direct color and apply color math;
6. publish the low-resolution pixel or hires pair projection.

Mode 7 may retain a specialized affine fetcher, but it must feed the same
layer-selection/compositor types.

## 3. Offset-per-tile helper

Add a pure helper that accepts mode, target BG, output sample coordinate, base
scroll, and BG3 state, then returns effective H/V source coordinates. Keep BG3
offset-map reads in a testable helper. Avoid mutating `bg_scroll`; offsets are
per fetched tile column.

## 4. Color source and compositor

Change `composite_pixel` to accept resolved main/sub color sources rather than
raw CGRAM indices. It must:

- resolve direct color before clipping/color math;
- retain fixed-color fallback when the sub screen is transparent;
- correct OBJ color-math eligibility so only OBJ palettes 4-7 (CGRAM indices
  192-255) participate;
- produce BGR555 first, then brightness-scaled XRGB;
- expose a value-returning inner helper so hires can finish two native XRGB
  samples and box-filter their 8-bit channels without double-writing a cell.

## 5. SETINI state

Decode SETINI into named accessors or fields (`interlace`, `obj_interlace`,
`overscan`, `pseudo_hires`, `extbg`) instead of scattering magic masks. Add a
field bit that toggles once per `begin_frame` regardless of interlace enable and
return it from STAT78 bit 7. Frame-latch overscan/interlace display state so
mid-frame SETINI writes do not move the current frame's scheduler boundary.
Latch all register bits on write and remove the feature fault.

Update `diag`/`diag_compositor` only if needed for replay diagnosis; avoid a
public API change solely for tests because module tests can inspect private
fields.

## 6. Fault taxonomy cleanup

After implementation:

- make BGMODE accept all `0..=7` modes;
- make CGWSEL accept direct color;
- make SETINI accept all values;
- retain the public `UnimplementedBgMode` and `UnimplementedPpuFeature` enum
  variants for API compatibility, but remove every PPU construction site;
- add an exhaustive test that writes every byte to BGMODE, CGWSEL, and SETINI
  and asserts no fault.

Do not broadly weaken D9. Other fault variants remain intact.

## Expected files

- `crates/refwork-emu/src/ppu/bg.rs`: generalized tile fetch and offset support.
- `crates/refwork-emu/src/ppu/mod.rs`: mode descriptors, common composition,
  Mode 7 EXTBG/direct color, SETINI/projection state, tests.
- `crates/refwork-emu/src/ppu/sprite.rs`: OBJ interlace source-row support.
- `crates/refwork-emu/src/bus.rs`: frame-latched overscan vblank/NMI/HVBJOY,
  auto-joypad, and HDMA timing.
- `crates/refwork-emu/src/core_impl.rs`: dynamic visible/HDMA/render boundary.
- `crates/refwork-emu/src/fault.rs`: retain public variants, remove stale scope
  wording only if needed.
- `crates/refwork-emu/src/timing.rs`: document fixed-output projection.
- `xtask/asm/synth.s65` or focused Rust tests only if extending the synthetic ROM
  is materially simpler than expressing the feature as hand-built VRAM/OAM.

Avoid touching ramdiff behavior, session data, manifests, or the private-ROM
launcher. The replay uses those artifacts, but implementation remains in the
emulator core and scheduler.
