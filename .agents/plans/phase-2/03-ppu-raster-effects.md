# 03 — PPU mid-frame raster effects, color math, windows, HDMA; auto-joypad timing

**Independent of 01/02.** Extends the M1 scanline renderer
(`crates/refwork-emu/src/ppu/`) from "modes 0–1, whole-frame registers" to
what the demo game's first room actually exercises. M1's D9 discipline
(fault on any unimplemented enable) is the to-do list: every
`Fault::UnimplementedPpuFeature` site in `ppu/mod.rs` is a candidate, and
package 06's bring-up log tells us which ones fire.

## Build order within the package

Implement in this order — it's dependency order *and* likelihood order for
a HUD-split game:

1. **HDMA** (`dma.rs`) — the mechanism behind the HUD split and most
   mid-frame effects. Per-scanline table walk: direct and indirect modes,
   all eight transfer patterns (shared with general DMA), repeat flag,
   line-counter decrement, channel termination, init at start of v-blank's
   end (line 0 reload), the documented general-DMA/HDMA interaction
   (channel conflict = fault for now, D9, unless bring-up shows the game
   needs the real behavior). HDMA writes land on PPU registers *between
   scanlines* — the existing per-line loop in `core_impl.rs`
   (`start_line`/`set_line`/`render_scanline`) is the natural seam: apply
   HDMA for line N before rendering line N.
2. **Per-scanline register latching.** M1 renders each line from current
   register state already — verify that scroll/brightness/mode register
   writes mid-frame (via HDMA or IRQ handlers) take effect on the *next*
   line boundary, and add a regression test. (True mid-*scanline* latching
   is explicitly out of scope per IMPLEMENTATION-PLAN — scanline-accurate,
   not cycle-exact.)
3. **Color math** — CGWSEL/CGADSUB/COLDATA semantics: fixed-color vs
   subscreen operand, add/subtract, half-math, per-layer enable, backdrop
   participation. Requires rendering the **subscreen** (TS-enabled layers)
   alongside the main screen in `render_mode0`/`render_mode1` — restructure
   the per-line compositor to produce (main, sub) pixel pairs, then apply
   math. Remove the `Fault::UnimplementedPpuFeature` on TS/CGWSEL/CGADSUB
   as each piece lands.
4. **Windows** — $2123–$212F: two windows, per-layer enable/invert, the
   four combination ops, main/sub masking (TMW/TSW), color-math window
   region select. Implement as a per-line 256-entry mask computed once per
   line per window config.
5. **H/V counter latch** — $2137 (SLHV) software latch, OPHCT/OPVCT
   ($213C/$213D) two-read protocol with high/low flip-flops, $213F status
   read clearing the latch flag. The counter values are a pure function of
   the per-line position the core already tracks; the M1 "always 0" stub is
   in `ppu/mod.rs`.
6. **Mosaic** — $2106 size + per-BG enable, line-group quantization.
7. **Auto-joypad busy window** (`joypad.rs` + `bus.rs`): model the
   documented latch window (auto-read occupies scanlines ~225–227): $4212
   bit 0 reflects in-progress status; $4218/$4219 reads during the busy
   window return the documented stale/partial behavior — implement the
   simple deterministic version (return previous latch while busy) and note
   it; games that poll $4212 first (the common idiom) are exact.
8. **BG modes 2–7, on demand only.** Do **not** pre-build all modes. The
   demo game's first room determines the set: when bring-up (06) hits
   `Fault::UnimplementedBgMode { mode }`, implement that mode. Mode 3
   (8bpp) and mode 7 (affine) are the likely candidates for title/intro
   screens; each is its own `render_modeN` following the mode-0/1 pattern.
   Modes never hit by the first-room route stay faulting — D9 keeps us
   honest, and M2 acceptance only needs the route to run.

## Constraints

- All compositing stays integer (color math is 5-bit-per-channel adds with
  clamp — natural integers; mode 7 matrix math is 8.8 fixed-point per the
  public docs — `i32`, no floats, deny gate enforces).
- No new allocations per frame (D8): window masks, subscreen line buffer,
  HDMA channel state are fixed-size fields allocated in `Ppu::new`.
- Every newly-implemented register keeps a fault path for the corners we
  still don't do (e.g. mode-7 EXTBG via SETINI stays a fault until needed).
- Synthetic ROM gains segments for each landed feature (HDMA gradient,
  color-math blend, window shape, counter-latch read) so CI's double-run
  hash covers them permanently — same pattern as M1.

## Acceptance (package-local)

- Per-feature unit tests with hand-computed expected pixels (existing
  `ppu` test style): HDMA scroll-split renders two regions; color-math
  add/half on known CGRAM values; window mask truth-table for all four ops;
  OPHCT/OPVCT two-read protocol; mosaic 4×4 quantization; auto-joypad $4212
  busy-bit sequence.
- Extended synthetic ROM double-run 10k frames green; zero-alloc green;
  deny green.
- Public PPU test ROMs where available for the implemented features (pin in
  `xtask/test-roms.lock`, operator-fetched like the CPU corpus) — run on
  the lab runner, not CI, if they need visual/manual judgment; automate the
  hash-comparable ones.
