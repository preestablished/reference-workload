# 01 — Readiness and Clean Build Layout

## Goal and prerequisite state

Make Beans and Git agree that PR #3 is complete, claim
`reference-workload-9xjj`, and create an isolated checkout layout that meets
the image builder's exact revision, toolchain, and cleanliness requirements.
Both blockers in [00-overview.md](00-overview.md) must resolve before this
package claims work or creates temporary build output.

## Repository evidence

- `origin/main` contains merge commit
  `ce754be1d83dbc212a1308c06e63f7cc612c5bc7`.
- `Cargo.toml` declares workspace version 0.2.0.
- `xtask/src/image.rs::write_workload_manifest` writes
  `meta.built_from.emu_version`, and `validate_manifest` requires a nonempty
  value and pins `meta.version` to the generating xtask version.
- `image/guest-sdk.lock` pins guest-sdk commit
  `acb1d3e8514a35eff66e94696d3a497b21328323`.
- `image/kernel.lock` pins the kernel artifact BLAKE3
  `595466463a37efac6822ffccf3e61d0a2230e7d223a94c0bce5eb78b2f43bee9`.
- `xtask/src/image.rs::sibling_checkout` resolves `control-plane`,
  `determinism-hypervisor`, and `guest-sdk` from the build checkout's parent.
- `xtask/src/image.rs::double_build` requires a fully clean
  reference-workload checkout and scoped-clean sibling inputs.

## Change surface

Existing repositories and inputs:

- this repository at `origin/main`;
- sibling `control-plane` at a recorded commit;
- sibling `determinism-hypervisor` at a recorded commit;
- sibling `guest-sdk` at the revision from `image/guest-sdk.lock`;
- `image/kernel.lock`, `image/builder.lock`, `image/guest-sdk.lock`,
  `image/boot.toml`, `image/harness.toml`, and
  `image/expected-regions.toml`.

Proposed external build layout under a newly created temporary directory:

```text
$REFWORK_BUILD_ROOT/                         (proposed temporary root)
├── reference-workload/                      (proposed detached worktree)
├── control-plane/                           (proposed detached worktree)
├── determinism-hypervisor/                  (proposed detached worktree)
└── guest-sdk/                               (proposed detached worktree)
    └── image/build/bzImage                  (proposed verified artifact copy)
```

`REFWORK_BUILD_ROOT` must resolve outside all four primary source checkouts.
The worktrees are disposable only after Package 4 persists and reads back the
canonical bundle and Package 5 completes the mandatory push.

Proposed private execution evidence beneath the operator-selected existing
private root:

- `$PRIVATE_ROOT/evidence/image-0.2.0-execution.log` (proposed private log,
  mode `0600`, never committed).

## Procedure

1. Resolve both blockers. Under `umask 077`, create the proposed private
   execution log at `$PRIVATE_ROOT/evidence/image-0.2.0-execution.log`; it is
   retained with the handoff record and never committed. Run `bn sync` and
   save `bn ready --json` there.
   Re-evaluate every P1 issue using the selection table in `00-overview.md`.
   Stop for operator selection if the table's host or dependency facts changed.
   Then run `bn show reference-workload-qglp --json` and
   `bn show reference-workload-9xjj --json`.
2. Fetch `origin/main`. Verify the PR merge and acceptance surface:

   ```sh
   git merge-base --is-ancestor ce754be1d83dbc212a1308c06e63f7cc612c5bc7 origin/main
   git merge-base --is-ancestor fd3037f0268f04fc1f71dd356e6c2d88695efb28 origin/main
   git show origin/main:Cargo.toml | grep -F 'version = "0.2.0"'
   git grep -n 'emu_version' origin/main -- xtask/src/image.rs image/README.md
   ```

3. Close `reference-workload-qglp` with a reason that names PR #3 and the
   verified merge commit. Do not close it based only on GitHub's state field.
4. Run `bn ready --json`; require `reference-workload-9xjj` to appear. Claim it
   atomically with `bn update reference-workload-9xjj --claim`.
5. Pin the implementation inputs to reference-workload
   `ce754be1d83dbc212a1308c06e63f7cc612c5bc7`, control-plane
   `e07b1eb3ac97024492211a63f9ffca401ea96249`, and determinism-hypervisor
   `60004db545825c08118d82c4fbd13ae1a1d28054`. If either sibling's selected
   main has advanced, stop for operator acceptance of the newer revision and
   record all accepted SHAs before building. Read the guest-sdk revision from
   `image/guest-sdk.lock`; do not substitute its current `main`.
6. Install Rust 1.98.1 if needed and set `RUSTUP_TOOLCHAIN=1.98.1` for every
   build and test command. Require rustc 1.98.1, Cargo 1.98.1, and rustfmt
   1.9.0-stable, then record their full version strings. A different toolchain
   is a stop condition because the repository has no `rust-toolchain.toml` pin.
7. Create `REFWORK_BUILD_ROOT` with `mktemp -d` outside the source checkouts.
   Add detached worktrees for each repository with the exact sibling names
   shown above. Use the selected main commit for reference-workload, the
   accepted exact revisions for control-plane and determinism-hypervisor, and
   the lock revision for guest-sdk.
8. Verify the existing primary guest-sdk kernel artifact against
   `image/kernel.lock`, create `guest-sdk/image/build/` in the isolated layout,
   and copy only the verified `bzImage` into it. Verify the copied bytes again.
   This ignored artifact is not present in a detached source worktree, and the
   package explicitly excludes rebuilding it.
9. Verify every detached worktree reports an empty `git status --short`.
   Verify the scoped paths checked by `double_build` are also empty.
10. From `$REFWORK_BUILD_ROOT/reference-workload`, verify:

   ```sh
   test "$(uname -m)" = x86_64
   test -e /dev/kvm
   test "$(zstd --version | sed -n 's/.* v\([0-9.]*\),.*/\1/p')" = 1.5.5
   rustup target list --installed | grep -Fx x86_64-unknown-linux-musl
   test "$(b3sum ../guest-sdk/image/build/bzImage | awk '{print $1}')" = \
     595466463a37efac6822ffccf3e61d0a2230e7d223a94c0bce5eb78b2f43bee9
   test "$(git -C ../guest-sdk rev-parse HEAD)" = \
     acb1d3e8514a35eff66e94696d3a497b21328323
   ```

11. Run `cargo test --locked -p xtask`. Stop on any failure; do not build a
    release bundle from a source revision whose image contract tests fail.

## Invariants and error paths

- Never run `git clean`, stash, reset, or branch-switch in the user's primary
  worktrees.
- Never echo `PRIVATE_ROOT`, `REFWORK_IMAGE_HANDOFF`, or their contents to
  Beans, repository files, or a public log. Store the execution log only at the
  proposed private path above with mode `0600`.
- A missing pinned guest-sdk object or kernel artifact is a stop condition.
  Do not repin or rebuild as part of this issue.
- A dirty detached worktree indicates setup drift. Recreate that temporary
  worktree rather than weakening the clean-build gate.
- If `origin/main` advances after selection, either restart with the new
  reviewed revision or pin the previously accepted merge commit explicitly and
  record that decision. Do not mix sources from two revisions.
- `/dev/kvm` is an environment confirmation for the downstream Intel-box
  chain; no VM operation belongs to this package.

## Verification and acceptance

- Beans reports `reference-workload-qglp` closed and
  `reference-workload-9xjj` in progress with the executing agent assigned.
- Each build-layout checkout is at its recorded revision and clean.
- Workspace version, emulator version, accepted sibling revisions, guest-sdk
  pin, kernel hash, zstd version, Rust 1.98 toolchain, Rust target, and required
  image inputs all match.
- `cargo test --locked -p xtask` passes.
- The primary worktrees retain all pre-existing untracked files unchanged.

## Exclusions

- Do not create or modify source files while resolving preflight failures.
- Do not fetch private game data into the build layout.
- Do not proceed with a convenient newer guest-sdk revision.
