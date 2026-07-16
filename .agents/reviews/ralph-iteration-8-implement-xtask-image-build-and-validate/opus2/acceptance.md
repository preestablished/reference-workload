# Acceptance Review

- `image build` CLI exists and requires `--agent-bin`: implemented in `xtask/src/main.rs:163`.
- `image validate PATH` CLI exists: implemented in `xtask/src/main.rs:214`.
- Static `refwork-harness` build path exists and builds the musl target: implemented in `xtask/src/image.rs:229`; local focused build passed.
- Dist output is ignored: `.gitignore:4` adds `dist/`.
- Dist artifact set is generated in code: `bzImage`, `initramfs.cpio.zst`, `workload-image.yaml`, `boot.toml`, `harness.toml`, `expected-regions.toml`, `README.md`, and `determinism.unstamped.yaml` are written or required by `xtask/src/image.rs:140` and `xtask/src/image.rs:175`.
- Newc writer is deterministic in code and unit-tested for repeatability/trailer presence: `xtask/src/image.rs:335` and `xtask/src/image.rs:1127`.

Acceptance gaps:

- `image validate` can validate artifact paths outside the manifest directory, so generated dist artifacts are not the only acceptance-verifiable bytes.
- `image validate` does not enforce the API cmdline whitelist or full machine device records.
- The build path does not honor the builder lock's zstd and panic strategy pins, weakening deterministic rebuild claims.
- The generated manifest is intentionally unstamped; later beads appear to own green-stamp enforcement, but API §4's current validation text says the xtask gate checks for it.
