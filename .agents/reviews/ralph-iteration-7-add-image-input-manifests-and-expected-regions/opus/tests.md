# Tests

Ran:

```text
cargo test --locked -p xtask image_inputs
cargo test --locked -p xtask --test image_inputs
git diff --check main...HEAD
```

Results:

- `cargo test --locked -p xtask image_inputs` passed, but only exercised two
  tests because the name filter did not run every test in `tests/image_inputs.rs`.
- `cargo test --locked -p xtask --test image_inputs` passed all six image-input
  tests.
- `git diff --check main...HEAD` passed.

Coverage note:

- The test target checks that the input files exist, placeholder hashes match,
  no ROM-like files are committed, the README states guest-sdk ownership, and the
  manifests contain expected strings.
- The expected-region layout test is not table-bounded; see `findings.md`.
