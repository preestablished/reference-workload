# Risks

- The image handoff files duplicate region metadata in `image/boot.toml:15` and
  `image/expected-regions.toml:5`. The values currently match, but keeping both
  files in sync will depend on tests that validate each table structurally.
- `xtask/tests/image_inputs.rs:33` and `xtask/tests/image_inputs.rs:45` use
  substring checks instead of a TOML parser. This leaves room for malformed TOML
  or duplicate/conflicting tables to satisfy the tests accidentally.
- No game payloads are present in this diff. The committed `image/` files are
  small text inputs only, and the README explicitly excludes ROM, SRAM,
  framebuffer golden, and game-derived bytes at `image/README.md:18`.
