# Implementation results

## Behavior and files changed

- `crates/refwork-emu/src/ppu/bg.rs` adds shared 2/4/8bpp pixel fetch with
  palette-group metadata and native-width tile addressing.
- `crates/refwork-emu/src/ppu/mod.rs` accepts every BGMODE, CGWSEL, and SETINI
  byte; implements modes 2/4/5/6, offset-per-tile fetch, direct color,
  pseudo-hires, corrected windows, Mode 7 EXTBG/affine/repeat behavior, field
  status, and frame-latched overscan projection.
- `crates/refwork-emu/src/ppu/sprite.rs` adds OBJ interlace row selection and
  size quirks.
- `crates/refwork-emu/src/bus.rs`, `core_impl.rs`, and `timing.rs` use the
  frame-latched 225/240 vblank boundary for rendering, HDMA, NMI, HVBJOY,
  auto-joypad, and OAM timing.
- `crates/refwork-emu/examples/rom_diag.rs` uses the current Clippy-preferred
  divisibility predicate so the all-feature target gate remains clean.
- The public legacy PPU fault variants remain API-compatible, but production
  PPU code no longer constructs either feature fault.

## Verification

- `cargo test -p refwork-emu --features introspect`: 196 passed, 0 failed.
- `cargo clippy -p refwork-emu --all-targets --all-features -- -D warnings`:
  passed with 0 warnings.
- Release `xtask` determinism tests with ignored tests enabled: 2 passed,
  0 failed, including 10,000 frames.
- `cargo test --release --locked -p xtask --test zero_alloc`: 1 passed,
  0 failed.
- `cargo run --locked -p xtask -- deny`: passed.
- `cargo fmt -p refwork-emu -- --check`, `git diff --check`, and the installed
  launcher-binary comparison: passed.
- Fault-aware private double-run: deterministic for 77,600 frames, 0 faults.
- Replay with the final rebuilt binary: completed 77,600 frames, 0 faults.

## External gates

- `cargo test --workspace --locked` passed all emulator and determinism test
  binaries reached, including the 30-test verifier integration binary, then
  stopped at 7 passed / 1 failed in `xtask/tests/image_inputs.rs`: the checkout
  lacks the generated `dist/workload-image-0.1.0/workload-image.yaml` fixture.
- `cargo fmt --all -- --check` could not load the sibling hypervisor workspace
  because its referenced `snapstore-client` manifest is absent. Package-scoped
  formatting passed.
