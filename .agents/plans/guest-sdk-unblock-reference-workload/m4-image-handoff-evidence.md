# M4 Image Handoff Evidence

RW-2/package-04 evidence note for bead `refwork-d7t.9`, recorded during Ralph
iteration 9.

Clean-room boundary: this note records command results, hashes, revisions, and
artifact paths only. It does not include game content, ROM bytes, framebuffer
goldens, WRAM dumps, SRAM, or padlog semantics.

## Local Run Context

| Field | Value |
|---|---|
| Date | 2026-06-22T00:11:38Z |
| Owner | Matt Spurlin (`refwork-d7t.9` owner); recorded by Codex during `/ralph` |
| Machine | `infra-control` |
| Architecture | `x86_64` |
| Branch | `ralph/iteration-9-implement-image-double-build-and-register-guard` |
| Verified artifact source rev | `1721437865678cb7da29058d7940764cec570c22` |
| Control-plane source | sibling checkout `../control-plane` |
| Control-plane rev | `ca9ee9048d7fca8eec5fe512011b011128e2b0c3` |
| Control-plane worktree | clean (`git -C ../control-plane status --short` produced no output) |
| Guest agent input | placeholder payload from `image/guest-sdk.lock` |

This note records the artifact hashes produced at the verified artifact source
rev above. Later documentation-only commits change `meta.built_from.git_rev` in
newly generated manifests by design, so branch-head rebuilds produce a new
manifest hash while preserving the kernel, initramfs, boot, region, and pad
contracts recorded here.

## Commands

```sh
cargo run --locked -p xtask -- image double-build
printf 'refwork-guest-sdk-placeholder-v1\n' > target/detguest-agent-placeholder
chmod 755 target/detguest-agent-placeholder
cargo run --locked -p xtask -- image build --agent-bin target/detguest-agent-placeholder
cargo run --locked -p xtask -- image register --manifest dist/workload-image-0.1.0/workload-image.yaml
cargo run --locked -p xtask -- image register --manifest dist/workload-image-0.1.0/workload-image.yaml --require-green-stamp
```

The first four commands passed locally. The final command intentionally failed
closed because only `determinism.unstamped.yaml` exists before package 06:
`missing determinism green stamp ... determinism.last_green`.

## Direct Handoff

| Artifact | Path | BLAKE3 |
|---|---|---|
| WorkloadImage manifest | `dist/workload-image-0.1.0/workload-image.yaml` | `b09dae3b79d1fa6fc314b91c1ccb54bb0b1317682481039ffb69afe157ba3fc3` |
| `boot.toml` | `dist/workload-image-0.1.0/boot.toml` | `802fa34f70b9a1f1fc96f0c79611b0d38cc84bda0556907f12ab241a97d89a23` |
| Expected-region handoff | `dist/workload-image-0.1.0/expected-regions.toml` | `55c95af82bef1712d6252f8c4f491592a1d6d6aa8e1e4a80bdd9c43a6a365d5c` |
| Harness config | `dist/workload-image-0.1.0/harness.toml` | `d5623fe12a28a10736f70ca298c687c8fc8723786f77a8144bd8da2b5d9c3edd` |
| README | `dist/workload-image-0.1.0/README.md` | `a85cb7552071b1c1a06f0c4678fb482de23f1c1800cd1afe06d6af32fe637c5e` |
| Unstamped determinism sidecar | `dist/workload-image-0.1.0/determinism.unstamped.yaml` | `6b613fc6ff13ddae996aa68ccda1bcfcd5f9dd6f25ca10e8e1d06387584eaf58` |

`image register` validated the manifest and reported a direct `dist/` handoff.
No control-plane registry upload was attempted because full registry support is
not present yet. The command accepts the unstamped sidecar before package 06,
but `--require-green-stamp` fails closed. When a green stamp is present,
validation requires a structured `determinism.last_green` sidecar tied to this
manifest hash and reference-workload git rev.

## Double-Build Result

`cargo run --locked -p xtask -- image double-build` created two clean roots under
`target/image-double-build/`, each with a copied Git-tracked source tree and a
sibling `control-plane` symlink to the recorded checkout above.

| Compared file | Bytes | BLAKE3 | Result |
|---|---:|---|---|
| `bzImage` | 34 | `9ae72dbae3e7a6e0b89fd3d3f0420b991c6187429420345777c2173ae9600ab7` | byte-identical |
| `initramfs.cpio.zst` | 302127 | `7467720ac006be828edfda4f21b4269cdf0bdfc709e4707e784d5a228afabe9b` | byte-identical |
| `workload-image.yaml` | 1577 | `b09dae3b79d1fa6fc314b91c1ccb54bb0b1317682481039ffb69afe157ba3fc3` | byte-identical |

The double-build manifest path for root A was
`target/image-double-build/root-a/reference-workload/dist/workload-image-0.1.0/workload-image.yaml`.
Root B produced the same manifest bytes at the corresponding `root-b` path.

## Region Handoff

`expected-regions.toml` is the guest-sdk handoff file for READY gating.

| Region | Size | Format | Layout version | Required | Writable |
|---|---:|---|---:|---|---|
| `wram` | 131072 | none | 1 | true | false |
| `framebuffer` | 229376 | `xrgb8888-256x224-stride1024` | 1 | true | false |
| `meta` | 4096 | none | 1 | true | false |

The WorkloadImage manifest also advertises optional `vram` at 65536 bytes and
optional `sram` at 0 bytes with `cart-dependent` note. Region
`layout_version` is intentionally kept out of `workload-image.yaml`; it lives in
the guest-sdk-owned handoff files.

## Boot And Pad Layout

`boot.toml` contract:

| Field | Value |
|---|---|
| `schema_owner` | `guest-sdk` |
| `autostart.name` | `refwork-harness` |
| `autostart.path` | `/usr/bin/refwork-harness` |
| `autostart.control_fd` | `3` |
| `autostart.load_game_device` | `/dev/vdb` |
| `ready.after` | `regions-registered-and-start-sent` |
| `ready.expected_regions` | `wram`, `framebuffer`, `meta` |

Pad layout in `workload-image.yaml`:

| Bit | Button |
|---:|---|
| 0 | A |
| 1 | B |
| 2 | X |
| 3 | Y |
| 4 | L |
| 5 | R |
| 6 | Up |
| 7 | Down |
| 8 | Left |
| 9 | Right |
| 10 | Start |
| 11 | Select |
| 12-15 | reserved |

## Scope Notes

This bead proves deterministic package-04 handoff construction, direct
registration/no-op behavior, and manifest validation. It does not prove the
external DH-1 Linux direct-boot baseline, real `detguest-agent` READY behavior,
first-room operator-game progress, or package-06 full determinism green-stamp
acceptance. The DH-1 external-readiness citation remains open for
`refwork-d7t.10`; packages 05 and 06 own the READY and full-suite evidence.
