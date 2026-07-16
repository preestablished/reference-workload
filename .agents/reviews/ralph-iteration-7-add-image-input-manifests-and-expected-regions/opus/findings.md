# Findings

## Low: expected-region test can pass when a region is missing `layout_version`

- Reference: `xtask/tests/image_inputs.rs:48`
- Reference: `xtask/tests/image_inputs.rs:52`
- Reference: `xtask/tests/image_inputs.rs:58`

`expected_regions_include_sizes_and_layout_versions` finds the first `name = "..."`
for each region, then checks `region_block = &expected[name_index..]`. That block is
the rest of the file, not the current `[[regions]]` table. As a result, `wram` or
`framebuffer` can lose its own `layout_version` and the test still passes because
a later region contains `layout_version = 1`.

The current manifest itself is correct, but this is fragile acceptance coverage for
the explicit requirement that each expected region includes a guest-sdk-owned
`layout_version` value. Parse the TOML or bound each region block before checking
`name`, `size`, and `layout_version`.
