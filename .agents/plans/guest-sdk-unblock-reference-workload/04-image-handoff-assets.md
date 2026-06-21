# 04 - Image Handoff Assets

**Upstream package:** RW-2.

**Purpose:** produce the artifacts guest-sdk and the hypervisor need for the
M4 join point: deterministic image layout, `workload-image.yaml`, `boot.toml`,
expected regions, region names/sizes, pad layout, and green-stamp fields.

This package publishes assets and proves reproducible image construction. It
does not claim real-agent READY validation; package 05 owns that.

## Dependencies

- Packages 02 and 03 complete.
- A pinned or prebuilt `detguest-agent` from guest-sdk. Until guest-sdk's real
  build path is available, accept `--agent-bin <path>` and record the source
  revision in the manifest.
- DH-1 / hypervisor Linux direct-boot floor cited before RW-2 closeout.
  Asset-only preparation may proceed before DH-1, but it must not close
  `guest-sdk-ext-refwork-m4-image-handoff` or claim package-04 acceptance.

## Deliverables

1. Image input files:
   - `image/kernel.lock`: kernel source tag or tarball URL plus BLAKE3.
   - `image/kernel.config`: minimal direct-boot Linux config required by the
     hypervisor and guest-sdk devices.
   - `image/builder.lock`: pinned builder container digest or equivalent toolchain
     pin.
   - `image/guest-sdk.lock`: guest-sdk rev used for `detguest-agent`.
   - `image/boot.toml`: guest-sdk-owned boot manifest instance. It must name the
     `refwork-harness` autostart unit and expected regions, including pinned
     guest-sdk `layout_version` values for each required region. The concrete
     `boot.toml` schema is guest-sdk-owned; this repo supplies an instance.
   - `image/harness.toml`: reference-workload-owned harness config such as
     optional `vram`/`sram` publication. No game content.
2. `xtask image` command family:
   - `cargo run --locked -p xtask -- image build --agent-bin <path>` builds
     `dist/workload-image-<version>/`.
   - `cargo run --locked -p xtask -- image validate <dist/workload-image-.../workload-image.yaml>`
     recomputes hashes and validates the manifest against API.md section 4.
   - `cargo run --locked -p xtask -- image double-build` builds from two clean
     checkouts or clean build directories and byte-compares `bzImage`,
     `initramfs.cpio.zst`, and `workload-image.yaml`. Each clean root must have
     a sibling `control-plane` checkout or the proto source recorded in package
     01.
   - `cargo run --locked -p xtask -- image register` may be a no-op or direct
     `dist/` handoff until control-plane artifact registration exists, but it
     must refuse missing determinism green stamps once package 06 lands.
3. Deterministic initramfs assembly:
   - Include exactly the required guest files:
     `/init`, `/sbin/detguest-agent`, `/usr/bin/refwork-harness`,
     `/etc/detguest/boot.toml`, `/etc/refwork/harness.toml`, device nodes and
     symlinks required by the guest.
   - Write `newc` cpio in sorted path order with fixed mtime, uid/gid, modes,
     and inode numbering.
   - Compress with a pinned deterministic zstd implementation and level.
   - Build `refwork-harness` as static musl release with path remapping and
     `panic=abort` if that is already the image policy.
4. `workload-image.yaml` writer:
   - Follow API.md section 4 exactly.
   - Include `meta.name = refwork-demo`.
   - Include `built_from.repo`, this repo git rev, and guest-sdk rev.
   - Include `artifacts.kernel` and `artifacts.initramfs` file names and BLAKE3
     hashes.
   - Include machine config: `vcpus: 1`, `mem_mib: 128`, `virtio-blk`
     game-image device, `detguest-channel`, and `pv-pad`.
   - Include regions:
     `wram` size 131072, `framebuffer` size 229376 and format
     `xrgb8888-256x224-stride1024`, `meta` size 4096, optional `vram` size
     65536, optional `sram` size 0 with cart-dependent note.
   - Include exact rational `fps` from API.md.
   - Include `pad_layout` byte-for-byte equivalent to API.md section 3.4:
     A, B, X, Y, L, R, Up, Down, Left, Right, Start, Select on bits 0-11 and
     reserved bits 12-15.
   - Include defaults for `feature_map` and `scoring_program`.
   - Include the API.md `determinism` block in a stable shape. Before package
     06, emit an explicit unschedulable sidecar such as
     `determinism.unstamped.yaml`; do not invent alternate
     `workload-image.yaml` fields unless API.md is updated.
5. Handoff files:
   - `dist/.../expected-regions.yaml` or equivalent machine-readable list for
     guest-sdk READY gating. It must include each required region name, size,
     and pinned `layout_version`. Do not put `layout_version` into
     `workload-image.yaml` unless the reference-workload API is updated to own
     that field.
   - `dist/.../boot.toml`.
   - `dist/.../harness.toml`.
   - A short `dist/.../README.md` explaining that the operator ROM is attached
     separately as the read-only game-image device.

## Validation Rules

The image validation command must fail if:

- `machine.vcpus != 1`.
- Required regions are missing or smaller than the feature map declares.
- `fps` is absent or represented as a float.
- `pad_layout` differs from the canonical table.
- Artifact hashes do not match the files.
- `boot.cmdline` restates hypervisor-owned canonical flags instead of append-only
  extras.
- Game content is present anywhere in `dist/`.

## Acceptance

- `cargo run --locked -p xtask -- image build --agent-bin <path>` produces a
  complete `dist/` folder.
- `cargo run --locked -p xtask -- image validate <manifest>` passes on the
  produced manifest.
- `cargo run --locked -p xtask -- image double-build` proves byte-identical
  kernel, initramfs, and manifest from two clean build roots.
- A DH-1 artifact or CI/lab run is cited showing the package-04
  kernel/initramfs shape is compatible with the hypervisor Linux direct-boot
  baseline. This is not real-agent READY validation; package 05 still owns that.
- Guest-sdk can consume `boot.toml`, expected-region list, region names/sizes,
  region `layout_version` values, and pad layout without parsing feature maps.
- The manifest and handoff files name `wram`, `framebuffer`, and `meta` exactly;
  optional `vram` and `sram` are explicit.

## Out Of Scope

- Real `detguest-agent` READY validation. Package 05 owns it.
- Full control-plane artifact registry. Direct manifest plus `dist/` handoff is
  allowed until control-plane exists.
