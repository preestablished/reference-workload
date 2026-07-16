# Acceptance

- Image input files exist: `image/kernel.lock`, `image/kernel.config`, `image/builder.lock`, `image/guest-sdk.lock`, `image/boot.toml`, `image/harness.toml`, `image/expected-regions.toml`, and `image/README.md`.
- No game content observed in the committed image inputs. The new files are text manifests/config/docs, with no ROM, SRAM, framebuffer golden, or game-derived payload files.
- `boot.toml` names the `refwork-harness` autostart unit at `image/boot.toml:5`, uses `/usr/bin/refwork-harness --fd3` at `image/boot.toml:6` and `image/boot.toml:7`, and declares expected regions at `image/boot.toml:13`.
- Expected-region handoff includes `wram`, `framebuffer`, and `meta` names, sizes, and `layout_version` values at `image/expected-regions.toml:5`, `image/expected-regions.toml:12`, and `image/expected-regions.toml:20`.
- Docs state guest-sdk owns `boot.toml` schema at `image/README.md:6`, and `image/README.md:22` keeps region `layout_version` out of WorkloadImage.

Acceptance is functionally met, subject to the test robustness finding in `findings.md`.
