# Risks

- Placeholder locks are reasonably documented for this iteration: `image/README.md:14` explains that `kernel.lock`, `builder.lock`, and `guest-sdk.lock` are package-04 input pins, and `image/README.md:15` says their placeholder values are resolved by the image build package before distributable output. Package 8 should still make the placeholder policy executable in validation so `pinned-placeholder` cannot silently become a final artifact unless that path is explicitly allowed.
- `layout_version` is confined to guest-sdk handoff files: `image/boot.toml:18`, `image/boot.toml:26`, `image/boot.toml:33`, `image/expected-regions.toml:8`, `image/expected-regions.toml:16`, and `image/expected-regions.toml:23`. No `workload-image.yaml` is committed, and `image/README.md:22` documents that WorkloadImage should own only region names and sizes.
- No game content was visible in the diff. The new image files are ASCII text and the tests reject common ROM-like files under `image/`.
