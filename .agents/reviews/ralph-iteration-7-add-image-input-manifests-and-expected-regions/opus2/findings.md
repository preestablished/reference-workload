# Findings

## Medium: image input tests can pass with inconsistent region fields

- `xtask/tests/image_inputs.rs:34` checks `boot.toml` with raw `contains` calls. It confirms `refwork-harness`, the `expected_regions` literal, and region names, but it does not verify that each `boot.toml` region carries the expected size and `layout_version`.
- `xtask/tests/image_inputs.rs:48` finds a region name in `expected-regions.toml`, then `xtask/tests/image_inputs.rs:52` scans from that name to EOF. A later region block can satisfy the `size` or `layout_version` assertion at `xtask/tests/image_inputs.rs:54` and `xtask/tests/image_inputs.rs:58`, so the test does not prove the field belongs to the intended region.

The committed manifests currently look consistent, but the regression guard is fragile around one of this iteration's explicit review targets: region sizes and `layout_version` handling. Parse the TOML and compare bounded per-region records across `image/boot.toml` and `image/expected-regions.toml`.
