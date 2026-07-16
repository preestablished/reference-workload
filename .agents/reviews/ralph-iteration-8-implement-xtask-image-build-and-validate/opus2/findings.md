# Findings

## High: `image validate` accepts artifact paths outside the bundle

`xtask/src/image.rs:504` reads each artifact `file` string from the manifest and `xtask/src/image.rs:516` hashes `base.join(file)` without rejecting absolute paths, `..`, or unexpected filenames. `validate_no_game_content` only walks the manifest directory at `xtask/src/image.rs:738`, so a manifest can point `artifacts.initramfs.file` at `../game.sfc` or another external payload with a matching BLAKE3 and still pass validation. That bypasses both the "adjacent artifacts" assumption and the no-game-content gate.

Fix: require the kernel/initramfs filenames to be exactly the API names (`bzImage`, `initramfs.cpio.zst`) or, at minimum, require normalized relative paths that canonicalize under the manifest directory before hashing and scanning.

## Medium: boot cmdline validation is a deny-list, not the API whitelist

API §4 says `boot.cmdline` may only append whitelisted extras such as `quiet`; `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md:537` documents the append-only contract. The implementation only rejects four substrings at `xtask/src/image.rs:535`, so `root=...`, `rdinit=...`, `single`, `loglevel=99`, timer flags, or other non-whitelisted extras validate. Those values affect boot behavior and MachineConfig identity, so they should be rejected before registration.

Fix: parse the cmdline into tokens and accept only `quiet` plus the documented `loglevel=0..7` form, rejecting duplicates, empty tokens, NUL/non-ASCII, and unsupported keys.

## Medium: machine device validation ignores required API attributes

The API requires the game image device to be `kind: virtio-blk`, `role: game-image`, `readonly: true`, and `required: true`, and requires the detguest-channel and pv-pad devices to be required (`/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md:544`). `validate_machine` only checks that some device has each `kind` at `xtask/src/image.rs:553`, so a manifest with `virtio-blk` but the wrong role, writable mode, or `required: false` passes.

Fix: validate each expected device as a complete record, including `role`, `readonly`, and `required` where specified.

## Medium: builder-lock determinism is not honored for zstd or panic strategy

`image/builder.lock:14` says zstd is `pinned-by-cargo-lock`, but `compress_zstd` shells out to the host `zstd` binary at `xtask/src/image.rs:291`; `Cargo.lock` has no `zstd` crate entry. Different host zstd versions can produce different frames for the same newc payload.

The same lock declares `panic = "abort"` at `image/builder.lock:16`, but `build_static_harness` only runs `cargo build --release --target x86_64-unknown-linux-musl` at `xtask/src/image.rs:230`. On this branch, the resulting musl binary is statically linked, but `readelf -Ws target/x86_64-unknown-linux-musl/release/refwork-harness` shows `panic_unwind`, `_Unwind_*`, and `rust_eh_personality` symbols, so the image build is not honoring the recorded panic strategy.

Fix: either enforce these pins in the build path (`panic = "abort"` in the release profile or explicit rustflags, and a Cargo-pinned zstd encoder) or update the lock and acceptance language to describe the actual host-tool dependency.
