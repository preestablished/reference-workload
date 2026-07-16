# Summary

Reviewer 1 found four actionable issues: three validation bypasses and one deterministic-build concern.

The build command does create the expected output directory and the generated happy-path manifest validates. The main gap is that `image validate` is too permissive for package-04: it validates hashes and selected manifest fields, but it does not enforce required artifact names, full device semantics, or the boot/harness TOML contracts. Temporary mutation checks confirmed those malformed bundles still validate successfully.

Recommended fix direction: make validation schema-driven enough to enforce exact artifact basenames, reject absolute or parent-relative artifact paths, validate machine device field values, parse and validate `boot.toml` / `harness.toml`, and either use a Cargo-pinned zstd implementation or explicitly pin/enforce the external `zstd` tool.
