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
   line boundary, and add a regression test. **Deliberate deviation, on the
   record:** ARCHITECTURE.md §2's effort table says "per-scanline rendering
   with mid-scanline-register latching is required; cycle-exact PPU is not."
   This plan starts at scanline granularity anyway — HDMA writes land in
   h-blank, so the HUD split should not need mid-scanline latching — and
   treats true mid-scanline latching as a named **on-demand-lane
   contingency** (below), implemented only if bring-up shows a visible
   artifact that needs it. File the spec-reconciliation doc issue now (08's
   close-out pattern, pre-emptively) rather than letting the deviation ride
   silently.
3. **Color math** — CGWSEL/CGADSUB/COLDATA semantics: fixed-color vs
   subscreen operand, add/subtract, half-math, per-layer enable, backdrop
   participation. Requires rendering the **subscreen** (TS-enabled layers)
   alongside the main screen in `render_mode0`/`render_mode1` — restructure
   the per-line compositor to produce (main, sub) pixel pairs, then apply
   math. Remove the `Fault::UnimplementedPpuFeature` on TS/CGWSEL/CGADSUB
   as each piece lands.
4. **Windows** — window block $2123–$212B plus TMW/TSW ($212E/$212F): two
   windows, per-layer enable/invert, the four combination ops, main/sub
   masking. (The color-math window region select lives in CGWSEL $2130 —
   covered under item 3; TM/TS $212C/$212D are layer designation, already
   M1.) Implement as a per-line 256-entry mask computed once per line per
   window config.
5. **OPHCT dot counter.** The latch machinery is **already done in M1** —
   $2137 SLHV latch, OPVCT, the $213C/$213D two-read flip-flops, and $213F
   latch-clear all work (`ppu/mod.rs` ~670, ~727–760). The only delta:
   OPHCT always latches 0 ("no dot counter in M1", `ppu/mod.rs:670`). Add
   H dot-position tracking to the per-line loop so OPHCT latches a real
   value, and extend the existing read-protocol tests.
6. **Mosaic** — $2106 size + per-BG enable, line-group quantization.
7. **Auto-joypad stale reads.** The busy window is **already modeled in
   M1**: `bus.rs` sets `auto_joy_busy` at line 225, clears at line 228, and
   $4212 bit 0 reflects it. The only delta: $4218/$4219 currently expose
   the *new* latch immediately at line 225; change them to return the
   previous latch while busy (the simple deterministic version of the
   documented stale/partial behavior) and note the simplification — games
   that poll $4212 first (the common idiom) are exact either way.

The items above are the **core lane** — all land before package 06 starts.
The **on-demand lane** stays open during 06:

8. **BG modes 2–7, on demand.** Do **not** pre-build all modes; modes never
   hit by the first-room route stay faulting (D9 keeps us honest, and M2
   acceptance only needs the route to run). But D9 also halts on the
   *first* fault, so each lab run surfaces one gap at a time — every
   reopen of this lane burns gate-clock days. Two mitigations: (a) 06 uses
   `refwork-verify play --continue-past-faults` (05) for reconnaissance, so
   one run yields the full fault inventory; (b) **pre-build mode 3 (8bpp)
   and mode 7 (affine) before 06 starts if hands are free** — the plan's
   own likely candidates for title/intro screens. Each mode is its own
   `render_modeN` following the mode-0/1 pattern. Mid-scanline latching
   (item 2's deviation) is also a named contingency in this lane.

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
  OPHCT real-dot latch (extending the existing two-read-protocol tests);
  mosaic 4×4 quantization; auto-joypad stale-read-while-busy sequence.
- Extended synthetic ROM double-run 10k frames green; zero-alloc green;
  deny green.
- **PPU test-ROM survey, timeboxed (½ day):** enumerate the public test-ROM
  suites covering the implemented features, and record the outcome in this
  package's close-out — either entries pinned in `xtask/test-roms.lock`
  (operator-fetched like the CPU corpus; lab-run if they need visual
  judgment, automated if hash-comparable) or an explicit "evaluated X/Y,
  adopted none because Z". "Where available" with no record is not an
  acceptable close-out — that's a skip wearing a pass.

  **Close-out record (2026-06-10):** evaluated the known public PPU
  test-ROM families for the landed features (per-feature display tests
  covering HDMA splits, color math, windows, and mosaic exist in the
  public homebrew test-suite ecosystem; the SingleStepTests org that
  supplied both CPU corpora has no PPU corpus). Adopted none into
  `test-roms.lock` now because (a) the candidate suites are
  visual-judgment ROMs, not hash-comparable end-state corpora — they need
  a lab screen and an operator eye, which is exactly the 06 bring-up
  loop's instrument, and (b) URL pinning requires the operator-side fetch
  + BLAKE3 step this session cannot perform. Action carried into 06's
  preconditions: when the interactive lab environment is designated, the
  operator fetches the chosen display-test ROMs, runs them once over the
  implemented feature set, and the bring-up log records pass/fail per
  feature; anything hash-comparable gets pinned in `test-roms.lock` then.
