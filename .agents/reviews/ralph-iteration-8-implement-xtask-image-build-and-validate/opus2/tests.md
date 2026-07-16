# Tests Run

- `cargo test --locked -p xtask image`
  - Passed. Covered the new `xtask::image` unit tests and filtered image-input tests.
- `cargo test --locked -p xtask --test image_inputs`
  - Passed. All 6 image input tests passed.
- `cargo fmt --all -- --check`
  - Passed.
- `git diff --check main...HEAD`
  - Passed.
- `cargo build --release --locked --target x86_64-unknown-linux-musl -p refwork-harness`
  - Passed. `file target/x86_64-unknown-linux-musl/release/refwork-harness` reports `static-pie linked`; `ldd` reports `statically linked`.
- `readelf -Ws target/x86_64-unknown-linux-musl/release/refwork-harness | rg 'panic_unwind|_Unwind|rust_eh_personality'`
  - Found unwind/panic symbols despite `image/builder.lock` declaring `panic = "abort"`.
- `cargo clippy --locked -p xtask --all-targets -- -D warnings`
  - Passed.

Not run:

- `cargo run --locked -p xtask -- image build --agent-bin <path>` because it writes generated `dist/` artifacts outside the review-artifact directory.
