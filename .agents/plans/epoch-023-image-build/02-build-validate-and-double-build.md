# 02 — Build, Validate, and Double-Build

## Goal and prerequisite state

Produce the unstamped workload-image 0.2.0 bundle from the clean layout and
prove contract validity plus the byte-for-byte packaging reproducibility that
the repository's double-build command defines. Package 1 must pass.

## Repository evidence

- `xtask/src/main.rs::cmd_image_build` invokes
  `xtask::image::build_image` without the `--agent-bin` escape hatch.
- `xtask/src/image.rs::build_agent_from_pinned_sibling` builds
  `detguest-agent` for `x86_64-unknown-linux-musl` only after exact guest-sdk
  revision verification.
- `xtask/src/image.rs::build_static_harness` compiles `refwork-harness` with
  `panic=abort` and remapped sibling paths, then rejects unwind symbols.
- `xtask/src/image.rs::validate_manifest` verifies artifact hashes, boot and
  machine contracts, regions, frame rate, pad layout, defaults, sidecars,
  handoff files, and absence of game content.
- `xtask/src/image.rs::compare_double_build_artifacts` compares complete bytes
  for `bzImage`, `initramfs.cpio.zst`, and `workload-image.yaml`.
- `dist/` is ignored by Git and is generated output.

## Change surface

Generated files below the existing, ignored `dist/` parent in the isolated
reference-workload worktree:

```text
dist/workload-image-0.2.0/                    (proposed generated directory)
├── README.md                                 (proposed generated file)
├── boot.toml                                 (proposed generated copy)
├── bzImage                                   (proposed generated copy)
├── determinism.unstamped.yaml                (proposed generated file)
├── expected-regions.toml                     (proposed generated copy)
├── harness.toml                              (proposed generated copy)
├── initramfs.cpio.zst                        (proposed generated file)
└── workload-image.yaml                       (proposed generated file)
```

Temporary proof outputs under existing ignored `target/`:

- `target/image-work/` (proposed generated scratch directory);
- `target/image-double-build/root-a/` (proposed generated comparison root);
- `target/image-double-build/root-b/` (proposed generated comparison root).

No tracked repository file changes.

## Procedure

1. In `$REFWORK_BUILD_ROOT/reference-workload`, capture the exact source and
   sibling revisions in the private scratch note. Re-run all cleanliness checks
   immediately before building.
2. Build through the normal pinned-agent path:

   ```sh
   cargo run --locked -p xtask -- image build
   ```

   Do not use `--agent-bin`.
3. Validate the generated bundle explicitly even though `image build` already
   performs validation:

   ```sh
   cargo run --locked -p xtask -- image validate \
     dist/workload-image-0.2.0/workload-image.yaml
   ```

4. Inspect the non-secret generated metadata. Require the manifest to identify
   `refwork-demo`, version 0.2.0, the selected reference-workload revision, the
   pinned guest-sdk revision, and emulator version `refwork-emu 0.2.3`. Require
   `determinism.unstamped.yaml` to identify the same bundle, source revision,
   and emulator version.
5. Require exactly one determinism sidecar:

   ```sh
   test -f dist/workload-image-0.2.0/determinism.unstamped.yaml
   test ! -e dist/workload-image-0.2.0/determinism.last_green
   ```

6. Hash `bzImage`, `initramfs.cpio.zst`, and `workload-image.yaml`; record the
   lowercase BLAKE3 values and byte counts in the private scratch note.
7. Hash the pinned guest-sdk `detguest-agent` that the direct build produced.
   Record that hash as an input and require it to remain unchanged through the
   double-build. This detects input drift but does not claim a second
   independent agent compilation.
8. Re-run `git status --short` across the four isolated worktrees. Only ignored
   `target/` and `dist/` output may exist in reference-workload; the command
   itself must remain empty.
9. Run the repository's clean-root packaging reproducibility proof:

   ```sh
   cargo run --locked -p xtask -- image double-build
   ```

10. Require the command to print `image double-build: OK` and one comparison
    row each for `bzImage`, `initramfs.cpio.zst`, and `workload-image.yaml`.
    The root-a and root-b hashes and byte counts must match. Record the direct
    build separately; its different physical build path means equality with
    root-a/root-b is not an established repository requirement.
11. Run the explicit validator against both double-build manifests and retain
    the command results in the private scratch note:

    ```sh
    cargo run --locked -p xtask -- image validate \
      target/image-double-build/root-a/reference-workload/dist/workload-image-0.2.0/workload-image.yaml
    cargo run --locked -p xtask -- image validate \
      target/image-double-build/root-b/reference-workload/dist/workload-image-0.2.0/workload-image.yaml
    ```

## Intended behavior and invariants

- The bundle version and emulator epoch remain distinct: 0.2.0 names the
  workload image, while `refwork-emu 0.2.3` names deterministic emulator
  behavior.
- The manifest `git_rev` is the selected reference-workload implementation
  commit. Both double-build manifests use the same revision even though their
  temporary checkout paths differ.
- The initramfs contains the agent built from the pinned guest-sdk commit and
  the harness built from the selected reference-workload commit.
- Path remapping prevents absolute checkout paths from changing artifact bytes.
- `double_build` rematerializes reference-workload tracked source twice but
  symlinks both roots to one guest-sdk checkout. Its result proves packaging
  reproducibility for the same pinned agent build input, not independent
  guest-sdk compiler determinism.
- Root-a is the canonical handoff source because the repository proves it
  byte-identical to root-b. The direct build is an earlier validation smoke and
  is not copied downstream.
- The bundle remains unstamped. A premature green stamp or registration is an
  error, not an optimization.
- No build input or output contains a ROM, SRAM, framebuffer golden, or
  game-derived bytes.

## Error paths and recovery

- If the first build fails because a tool or artifact is missing, stop and
  record the named preflight failure. Do not edit locks or source inside this
  task.
- If validation fails, retain the isolated output for diagnosis and do not
  hand it off.
- If double-build differs, retain both roots, record the three reported hashes,
  file a separate reproducibility bug, and keep `reference-workload-9xjj` in
  progress.
- If a build is interrupted, rerun `image build` in the isolated worktree. The
  command deliberately replaces only its local 0.2.0 output directory.
- Rollback before consumption consists of removing or quarantining the persistent
  bundle and deleting the temporary build root after evidence capture. No user
  or stored-data migration exists.

## Verification and acceptance

- `cargo test --locked -p xtask` passes from Package 1.
- The direct build and all three manifest validation runs pass.
- Generated manifest and sidecar values match the selected revisions and
  versions exactly.
- Root-a and root-b artifacts match byte for byte under the documented
  shared-agent-input limitation.
- Repository status contains no tracked or untracked build files because
  outputs remain under ignored `dist/` and `target/`.
- Nothing has been staged downstream and no tracker issue has been closed yet.

## Exclusions

- Do not run the package-06 `vm-first-room`, `vm-suite`, green-stamp, or
  register steps.
- Do not interpret a valid first build as a substitute for double-build.
- Do not commit generated artifacts.
