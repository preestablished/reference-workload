# Hardware semantics to preserve

Use the Nintendo development-manual register definitions and the current ares
SFC PPU implementation as implementation references. Do not transplant code;
translate observable behavior into the existing Rust scanline architecture.

Reference points:

- SETINI fields: `EX..HOiI` (`external sync`, `EXTBG`, `pseudo-hires`,
  `overscan`, `OBJ interlace`, `screen interlace`).
- ares SFC PPU mode assignment, background coordinate, Mode 7, DAC/direct-color,
  and SETINI paths under `ares/sfc/ppu/`.
- Public links: <https://snes.nesdev.org/wiki/PPU_registers>,
  <https://snes.nesdev.org/wiki/Mode_7>, and
  <https://github.com/ares-emulator/ares/tree/master/ares/sfc/ppu>.

## Mode table

Implement one declarative description for bpp/layer activation and one priority
description per mode, rather than another large copy of the current mode 0/1/3
loops.

| Mode | BG1 | BG2 | BG3 | BG4 | Special behavior |
|---|---:|---:|---:|---:|---|
| 0 | 2bpp | 2bpp | 2bpp | 2bpp | per-BG palette banks |
| 1 | 4bpp | 4bpp | 2bpp | off | BG3-priority bit |
| 2 | 4bpp | 4bpp | offset source | off | H/V offset-per-tile |
| 3 | 8bpp | 4bpp | off | off | BG1 direct color eligible |
| 4 | 8bpp | 2bpp | offset source | off | H-or-V offset-per-tile, BG1 direct color eligible |
| 5 | 4bpp | 2bpp | off | off | native 512-dot horizontal sampling |
| 6 | 4bpp | off | offset source | off | native 512-dot sampling + H/V offset-per-tile |
| 7 | affine 8bpp | EXTBG duplicate | off | off | BG1 direct color eligible |

Use numeric ranks for layer selection. Preserve the existing documented order
for modes 0 and 1 and add these high-to-low orders:

- modes 2-5: `OBJ3, BG1hi, OBJ2, BG2hi, OBJ1, BG1lo, OBJ0, BG2lo, backdrop`;
- mode 6: `OBJ3, BG1hi, OBJ2, OBJ1, BG1lo, OBJ0, backdrop`;
- mode 7 without EXTBG: `OBJ3, OBJ2, BG1, OBJ1, OBJ0, backdrop`;
- mode 7 with EXTBG: `OBJ3, EXTBG-hi, OBJ2, BG1, OBJ1, EXTBG-lo, OBJ0,
  backdrop`.

The same priority engine must build both main and sub screens while respecting
TM/TS and TMW/TSW.

## Mode 7 EXTBG

- SETINI bit 6 only changes rendering in Mode 7. It must never fault in another
  mode.
- BG1 uses the full Mode 7 pixel byte. BG2 views the same byte when EXTBG is on,
  uses bit 7 as its priority selector, and uses bits 0-6 as its indexed CGRAM
  color. Zero remains transparent.
- BG2 is independently enabled and windowed through the ordinary BG2 TM/TS and
  TMW/TSW bits.
- Correct the existing window-selector decode before relying on this path:
  within each two-bit layer field, bit 0 is invert and bit 1 is enable. Test
  enable and invert independently for BG1, BG2, OBJ, and color windows.
- Direct color applies to Mode 7 BG1 only; EXTBG BG2 remains CGRAM-indexed.
- Add dedicated 13-bit M7HOFS/M7VOFS shadows. Writes to `$210D/$210E` update
  them through the same shared Mode 7 byte latch used by `$211B-$2120`; ordinary
  BG scroll shadows continue to follow their own latch behavior.
- Fix Mode 7 coordinate math while touching this path: sign-extend center and
  scroll, use the hardware clip function for `(scroll - center)`, apply `& !63`
  to each matrix product in the origin formula, and implement all M7SEL repeat
  encodings, flips, and mosaic. Repeat mode 3 forces tile number zero when out
  of bounds but retains transformed low X/Y bits for the pixel within tile 0.
  Current code uses the center as if it were scroll and collapses repeat modes.

## Direct color

Add a pixel color-source representation so the compositor can distinguish a
CGRAM index from a direct BGR555 value. Direct color is eligible only for BG1 in
modes 3, 4, and 7 when CGWSEL bit 0 is set.

Given 8-bit tile color `BBGGGRRR` and tile palette group `...bgr`, construct:

```text
0BBb00GG Gg0RRRr0
```

Transparent color index zero stays transparent even though its computed direct
color may be nonzero. OBJ, BG2, and EXTBG must remain indexed. Mode 7 BG1 uses
palette group zero because its tilemap has no palette-group field.

## Offset per tile

- BG3 is an offset map source and is not itself displayed in modes 2, 4, or 6.
- The leftmost screen tile column uses the layer's ordinary scroll. Offset
  entries affect subsequent 8-pixel columns.
- Modes 2 and 6 can independently replace H and V coordinates using BG3's two
  offset words and validity bits 13/14 for BG1/BG2. Fetch V from the next BG3
  map row, wrapping according to BG3's configured screen size.
- Mode 4 uses one fetched word: bits 13/14 independently enable BG1/BG2 and bit
  15 selects vertical rather than horizontal replacement.
- Preserve the low three horizontal-scroll bits when replacing H offset.
- Fetch offset words using BG3's own tilemap size and scroll rules, including
  boundary wrapping and its 8x8/16x16 tile-size bit. In mode 6, account for the
  doubled horizontal lookup coordinate explicitly.

Unit-test the first-column exception, target validity bits, H/V selection,
low-bit preservation, negative/wrapped offsets, and both target BGs.

## Hires, pseudo-hires, interlace, and overscan policy

The public framebuffer is fixed at 256x224. Implement full internal sample
selection but define a deterministic projection:

- Native hires (modes 5/6) fetches 512 horizontal BG samples and applies the
  documented sub/main pair phase. H scroll is scaled; each tilemap entry selects
  adjacent characters across hires dots, including correct hflip and both tile
  sizes. Pseudo-hires uses the same alternating presentation for low-resolution
  modes. Define how 256-coordinate OBJ, window edges, and mosaic boundaries map
  onto the native sample pair.
- Project each native pair to one published pixel with an integer per-channel
  box filter after each native sample has completed color math, brightness, and
  XRGB expansion. This fixes the rounding order and retains both samples.
- The field bit alternates every frame even when progressive output is selected,
  and STAT78 bit 7 reports it. Screen interlace changes field presentation;
  only modes 5/6 double the tile-BG source Y and add the field. Ordinary modes
  keep the same BG source row across fields. The fixed schedule deliberately
  omits the hardware's alternating 263-line interlace field; document this
  contract-level timing compromise rather than claiming cycle accuracy.
- OBJ interlace is independent of screen interlace: halve scanline coverage and
  use a doubled, field-adjusted source Y with correct vflip direction. Cover the
  base-size 6/7 height quirk and rectangular sprites.
- Overscan changes the frame-latched visible/vblank boundary from 225 to 240.
  Use that boundary consistently for rendering, HDMA, NMI, `$4212`, auto-joypad,
  and OAM reload. Source lines 1-239 are rendered; project them to 224 rows with
  an exact 7-top/8-bottom crop. Total frame length remains 262 lines.
- External-sync bit 7 and reserved bits 4/5 are latched but have no effect in a
  standalone emulator and never fault.

This projection is a repository API policy, not a claim that native hardware is
256x224 in these modes.
