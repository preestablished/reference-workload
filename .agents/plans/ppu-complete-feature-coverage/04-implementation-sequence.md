# Implementation sequence

Each step should leave focused tests green. Do not wait until the final step to
learn that the common compositor changed modes 0/1.

## Step 1: characterize and protect existing output

Add missing mode 0/1 priority, sub-screen, and window/color-math parity tests.
Add pure tests for the current planar readers and fixed non-private golden output
for mode 0 and synthetic mode 1. Correct window selector enable/invert decoding
under its own tests. Run `cargo test -p refwork-emu`.

## Step 2: generalize pixel representation and tile fetch

Add palette-group/color-source metadata, share the 2/4/8bpp planar decode, and
move modes 0/1/3 onto the generalized fetch without changing output. Add direct
color conversion as a pure helper but keep the register fault until behavioral
tests are ready.

## Step 3: common priority and main/sub compositor

Introduce mode descriptors/ranks and use one selector for modes 0/1/3. Refactor
the compositor into a value-producing BGR555 stage plus framebuffer write.
Re-run all PPU tests and compare synthetic determinism before proceeding.

## Step 4: modes 2 and 4

Implement and test BG3 offset-map reads and effective coordinates, then enable
mode 2 and mode 4 descriptors. Implement BG1 direct color for modes 3/4 and
remove the CGWSEL fault after its tests pass.

## Step 5: hires modes 5 and 6 plus pseudo-hires

Add 512-sample line buffers or an allocation-free two-sample loop, the native
pair projection, mode 5, mode 6 offset lookup, and pseudo-hires. Keep all fixed
buffers allocated by `Ppu::new`; exercise these paths under a counting allocator.

## Step 6: Mode 7 correctness and EXTBG

Correct affine coordinate/repeat semantics, feed Mode 7 through the common
selector, add direct color, then add EXTBG BG2 with palette-bit priority.
Change SETINI `$40` from faulting to rendered only after EXTBG pixel tests pass.

## Step 7: remaining SETINI behavior

Implement unconditional field tracking/STAT78, BG/OBJ interlace source selection,
frame-latched overscan timing across bus/core/PPU, exact output mapping, and
external-sync/reserved no-op latching. Remove the remaining SETINI fault. Add
exhaustive no-feature-fault coverage while retaining the public fault variants.

## Step 8: replay and broad verification

Run the private recorded prefix plus hold-last past the old frame using the
fault-aware double-run verifier. Run the repository gate matrix. Search for stale
`unimplemented` documentation, fault assertions, and mode-specific fallback
black rendering; update comments to match the completed feature set.

## Step 9: implementation record

Add `07-implementation-results.md` in this plan directory containing only:

- files/behavior changed;
- test commands and pass/fail counts;
- replay frame count and whether a fault occurred;
- any external gates that truly could not run.

Do not include the private ROM name/path, pad contents, screenshot, framebuffer
bytes, or hashes derived from private content.
