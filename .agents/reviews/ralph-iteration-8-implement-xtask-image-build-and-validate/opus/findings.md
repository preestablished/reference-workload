# Findings

## High: `image validate` accepts bundles without the required artifact filenames

- References: `xtask/src/image.rs:495-523`, `xtask/src/image.rs:698-714`, `xtask/src/image.rs:402-408`
- Acceptance impact: package-04 requires the handoff directory to contain `bzImage` or a documented placeholder and `initramfs.cpio.zst`. The validator only hashes whatever paths the manifest names under `artifacts.kernel.file` and `artifacts.initramfs.file`; it never enforces the package artifact names and `validate_handoff_files` does not require either file.
- Evidence: I copied `dist/workload-image-0.1.0` to a temp dir, renamed `bzImage` to `kernel.payload` and `initramfs.cpio.zst` to `initramfs.payload`, updated `workload-image.yaml` to point at those renamed files without changing the hashes, and `cargo run --locked -p xtask -- image validate <tmp>/workload-image.yaml` still printed `image validate: OK`.
- Why this matters: a malformed or API-drifted bundle can pass validation while missing the exact files downstream package consumers are contracted to boot.

## High: machine device contract fields are not validated

- References: `xtask/src/image.rs:412-418`, `xtask/src/image.rs:544-563`
- Acceptance impact: `image validate` should reject package-04 contract violations. The generated manifest requires `virtio-blk` with `role: game-image`, `readonly: true`, and `required: true`, plus required `detguest-channel` and `pv-pad` devices. Validation only checks that a device with each `kind` exists.
- Evidence: I changed the generated manifest's virtio block device from `{ kind: virtio-blk, role: game-image, readonly: true, required: true }` to `{ kind: virtio-blk, role: host-root, readonly: false, required: false }`; validation still passed.
- Why this matters: this bypass allows a manifest to drift away from the game-image handoff contract, including making the game block device writable or no longer identified as the game image.

## High: sidecar TOML schema/contract violations are not rejected

- References: `xtask/src/image.rs:132-145`, `xtask/src/image.rs:698-714`, `image/boot.toml:4-13`, `image/harness.toml:4-17`
- Acceptance impact: the bundle includes `boot.toml`, `harness.toml`, and expected region handoff data, and validation is expected to reject package-04 schema/contract violations. Current validation checks that `boot.toml` and `harness.toml` exist, but it does not parse or validate their schema versions, owners, autostart path, fd 3 control channel, load game device, protocol version, or required region policy.
- Evidence: I changed copied `boot.toml` / `harness.toml` values from `control_fd = 3` to `control_fd = 4` and `protocol_version = 1` to `protocol_version = 99`; `image validate` still passed.
- Why this matters: a generated or hand-edited bundle can be non-bootable or incompatible with the fd-3 harness protocol while passing the validator.

## Medium: initramfs compression depends on an unpinned host `zstd`

- References: `xtask/src/image.rs:291-313`, `image/builder.lock:12-15`
- Acceptance impact: deterministic archive/build behavior is part of the review scope. `image/builder.lock` records `zstd = "pinned-by-cargo-lock"`, but the implementation shells out to whatever `zstd` binary is on `PATH`.
- Why this matters: the compressed `initramfs.cpio.zst` bytes, and therefore the manifest hash at `xtask/src/image.rs:178-186`, can vary across developer/CI hosts or future zstd versions even when the raw deterministic `newc` archive is identical. The build should either use a pinned Rust zstd implementation from Cargo.lock or record/enforce the external tool identity.
