# Acceptance

- Image input files exist: satisfied by `image/kernel.lock:1`,
  `image/kernel.config:1`, `image/builder.lock:1`, `image/guest-sdk.lock:1`,
  `image/boot.toml:1`, `image/harness.toml:1`,
  `image/expected-regions.toml:1`, and `image/README.md:1`.
- No game content: satisfied in the current diff. The image directory contains
  only text manifests/config/placeholder locks, and `image/README.md:18` states
  ROM, SRAM, framebuffer golden, and game-derived bytes do not belong there.
- `boot.toml` names `refwork-harness` autostart: satisfied by
  `image/boot.toml:4`, `image/boot.toml:5`, `image/boot.toml:6`, and
  `image/boot.toml:7`.
- `boot.toml` names required expected regions: satisfied by
  `image/boot.toml:11` and `image/boot.toml:13`.
- Expected-region handoff includes `wram`, `framebuffer`, and `meta` names,
  sizes, and `layout_version` values: satisfied by `image/expected-regions.toml:5`,
  `image/expected-regions.toml:12`, and `image/expected-regions.toml:20`.
- The expected sizes match the harness region constants: `wram` 131072 bytes,
  framebuffer 229376 bytes, and `meta` 4096 bytes.
- Guest-sdk owns the handoff schema: satisfied by `image/boot.toml:2`,
  `image/expected-regions.toml:2`, `image/guest-sdk.lock:12`, and
  `image/README.md:6`.
- Docs state guest-sdk owns `boot.toml` schema: satisfied by
  `image/README.md:6`.

Acceptance is functionally met by the committed manifests. The only finding is
test fragility around per-region `layout_version` enforcement.
