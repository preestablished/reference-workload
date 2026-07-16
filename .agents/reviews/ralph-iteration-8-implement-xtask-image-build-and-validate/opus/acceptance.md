# Acceptance Review

- `image build --agent-bin` exists and produces `dist/workload-image-0.1.0/`.
- The generated directory contains `bzImage`, `initramfs.cpio.zst`, `workload-image.yaml`, `boot.toml`, `harness.toml`, `expected-regions.toml`, and `README.md`.
- The generated manifest recomputes and validates hashes for the artifact paths it names.
- The validator rejects several manifest-level drifts covered by unit tests: wrong vCPU count, float FPS, pad layout drift, region `layout_version` inside `workload-image.yaml`, and visible game-like payload files.
- Acceptance is not fully met because `image validate` accepts package-04 contract violations:
  - required artifact file names can be changed away from `bzImage` and `initramfs.cpio.zst`;
  - game-image device fields can be changed away from required readonly game-image semantics;
  - `boot.toml` and `harness.toml` contract fields can be invalid while validation still succeeds.
- Deterministic build acceptance is weakened by use of an unpinned host `zstd` binary despite the builder lock claiming Cargo-lock pinning.
