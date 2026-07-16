# Tests

Ran:

- `cargo test --locked -p xtask --test image_inputs` - passed, 6 tests.
- `python3`/`tomllib` parse check for `image/boot.toml`, `image/expected-regions.toml`, `image/harness.toml`, `image/kernel.lock`, `image/builder.lock`, and `image/guest-sdk.lock` - passed.
- `git diff --check main...HEAD` - passed.
- `file image/*` - all new image files reported as ASCII text.

Not run:

- Full workspace tests. This review focused on commit `5e34ef8` and the package-04 image input handoff diff.
