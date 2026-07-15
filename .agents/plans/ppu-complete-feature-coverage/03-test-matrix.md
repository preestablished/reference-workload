# Test matrix

## Pure helper tests

- Decode 2bpp, 4bpp, and 8bpp planar rows with known bit patterns.
- Direct-color conversion tests all tile-data and palette-group bit lanes.
- Priority tables cover both priority states for every active BG and all four
  OBJ priorities in every mode.
- Offset-per-tile covers mode 2 H/V, mode 4 H-vs-V selection, mode 6 hires
  lookup, BG1/BG2 validity, first-column behavior, scroll low bits, row-31/63
  map wrapping, and BG3 8x8/16x16 lookup geometry.
- Native pair projection uses integer channel math with odd/even component sums.
- Overscan row mapping and interlace field mapping test their boundary rows.

## Pixel tests

Build minimal VRAM tilemaps, tiles, palettes, OAM, and register state as current
PPU tests do. Assert XRGB output values or selected BGR555 values for:

1. Mode 2 BG1 and BG2, including an offset-map change at column 1.
2. Mode 4 BG1 8bpp and BG2 2bpp, with H and V offset entries.
3. Mode 5 512-sample behavior where the two native samples deliberately differ
   and the 256-wide projection proves both contributed. Cover adjacent-character
   transitions, hflip, odd H scroll, both tile sizes, and exact sub/main phase.
4. Mode 6 hires plus offset-per-tile, including OBJ, window edge, mosaic, and
   color-math interactions in native sample coordinates.
5. Direct color in modes 3, 4, and 7, and a negative test proving BG2/OBJ use
   CGRAM.
6. Mode 7 EXTBG low/high priority against each OBJ priority, including
   independent BG2 enable/window masking and bit-7 removal from its palette.
7. Mode 7 shared-latch M7HOFS/M7VOFS writes, signed center/origin boundary
   vectors, repeat modes 0/1/2/3, flips, and mosaic.
8. Pseudo-hires alternating main/sub samples.
9. Progressive STAT78 field alternation, ordinary-mode interlace retaining its
   BG row, and mode-5/6 even/odd field source rows.
10. OBJ interlace adjacent-line visibility, both fields, vflip, rectangular
   sprites, and base-size-6 behavior.
11. Overscan source rows 224/225/239/240, exact 7/8 crop, and core-level
   NMI/HVBJOY/autojoy/OAM/HDMA boundary behavior.
12. Window selector enable/invert bits independently for BG1/BG2/OBJ/COL and
   EXTBG BG2 through both TMW and TSW.
13. OBJ color math with otherwise identical palette-3/palette-4 sprites,
   including overlap with EXTBG.

Keep existing mode 0/1/color-math/window/sprite tests green. Before refactoring,
capture non-private golden output for fixed-frame mode-0 and synthetic mode-1
fixtures (pixel arrays or stable hashes). Add multi-layer priority parity tests
so deterministic but wrong output cannot pass only by matching itself twice.

## Fault coverage test

For fresh PPU instances:

- write all 256 BGMODE values;
- write all 256 CGWSEL values;
- write all 256 SETINI values in every BGMODE;
- assert every write returns `None`;
- render one scanline for every mode/SETINI/direct-color combination to catch
  latent panics or black fallback paths.

This test is the direct proof that gameplay cannot encounter another deliberate
PPU feature fault from these registers.

## Private-prefix replay

Use `$PRIVATE_ROM` and `$PRIVATE_PADLOG` with shell tracing disabled. The log
contains successful frames through 77,464; frames beyond it use the parser's
documented hold-last input. Run `refwork-verify double-run --frames 77600` (or an
equivalent verifier that exits nonzero on faults, reports completed frame count,
and compares both deterministic legs). Do not use `ramdiff record` as the proof:
it returns success after breaking on a fault and does not report completed frames.

Acceptance:

- no `Fault` at or before frame 77,465;
- at least 77,600 completed frames;
- both verifier legs produce the same chain, retained only as local evidence;
- if a later non-PPU fault appears, report it separately, but a remaining PPU
  feature fault means this plan is incomplete.

## Repository gates

Run, in order:

```bash
cargo fmt --all -- --check
cargo build --workspace --locked
cargo test -p refwork-emu
cargo test -p refwork-emu --features introspect
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- deny
cargo test --workspace --locked
cargo run --locked -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml
cargo run --locked -p refwork-featuremap -- schema > /tmp/feature-map.schema.json
diff -u schema/feature-map.schema.json /tmp/feature-map.schema.json
cargo run --locked --release -p xtask -- hash-chain --frames 10000
cargo test --release --locked -p xtask --test determinism -- --include-ignored
cargo test --release --locked -p xtask --test zero_alloc
```

Add a counting-allocator test that explicitly renders every mode and relevant
SETINI/CGWSEL path after warmup; the existing mode-1 zero-allocation ROM alone is
insufficient. If a gate name differs from the current CLI, derive the exact
command from `.github/workflows/ci.yaml`. Record unavailable sibling or
architecture dependencies, and require actual x86_64/aarch64 CI results for the
cross-architecture gate rather than representing a local subset as proof.
