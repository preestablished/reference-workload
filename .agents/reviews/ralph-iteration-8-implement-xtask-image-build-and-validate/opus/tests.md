# Tests Run

- `cargo test --locked -p xtask image::tests` - passed.
- `cargo test --locked -p xtask --test image_inputs` - passed.
- `cargo fmt --all -- --check` - passed.
- `cargo clippy --locked -p xtask --all-targets -- -D warnings` - passed.
- `cargo run --locked -p xtask -- deny` - passed.
- `git diff --check main...HEAD` - passed.
- `cargo run --locked -p xtask -- image build --agent-bin /bin/true` - passed and wrote `dist/workload-image-0.1.0/`.
- `cargo run --locked -p xtask -- image validate dist/workload-image-0.1.0/workload-image.yaml` - passed.

# Mutation Checks

- Renamed `bzImage` and `initramfs.cpio.zst` in a temp copy and updated manifest file names to match existing hashes. Validation still passed, confirming the artifact-name bypass.
- Changed the generated virtio block device to `role: host-root`, `readonly: false`, and `required: false` in a temp copy. Validation still passed, confirming the machine-device contract bypass.
- Changed copied `boot.toml` / `harness.toml` fd and protocol values away from the expected contract. Validation still passed, confirming sidecar TOML contract drift is not rejected.
