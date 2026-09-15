# 04 — Execution Handoff

## Current status

Blocked on two owner-supplied inputs:

1. Create `~/.agents/projects/reference-workload/private-root.path` on the
   Intel box and make the referenced `evidence/` directory writable.
2. Have the determinism-hypervisor epoch-0.2.3 owner restore or explicitly
   replace Plan B's staging contract, including the persistent path, artifact
   filenames, `DH_M9_BZIMAGE`/`DH_M9_INITRAMFS` mapping, and custody through
   `reference-workload-ftor`, including its in-place or `dist/` restoration
   procedure.

Implementation has not occurred.

## Dependency-ordered work packages

### Package 1 — Reconcile the tracker and create clean inputs

Result: Beans reflects the merged PR, `reference-workload-9xjj` is claimed,
and clean detached worktrees provide the exact source and sibling revisions.

Existing files and interfaces:

- `Cargo.toml` workspace version;
- `crates/refwork-emu/src/lib.rs::EMU_VERSION`;
- `image/guest-sdk.lock` and `image/kernel.lock`;
- `xtask/src/image.rs::{sibling_checkout,double_build}`;
- `bn show`, `bn close`, `bn ready`, and `bn update --claim`.

Proposed external paths:

- `$REFWORK_BUILD_ROOT/{reference-workload,control-plane,determinism-hypervisor,guest-sdk}`
  as detached temporary worktrees.

Prerequisites: both owner-supplied inputs must resolve before this package
claims work. The merge acceptance check precedes closing
`reference-workload-qglp`; its closure precedes claiming
`reference-workload-9xjj`.

Verification:

```sh
git merge-base --is-ancestor ce754be1d83dbc212a1308c06e63f7cc612c5bc7 origin/main
git merge-base --is-ancestor fd3037f0268f04fc1f71dd356e6c2d88695efb28 origin/main
git status --short
cargo test --locked -p xtask
```

Acceptance: all version, revision, artifact, toolchain, and clean-checkout gates
in [01-readiness-and-clean-build-layout.md](01-readiness-and-clean-build-layout.md)
pass without changing a primary worktree.

### Package 2 — Build and validate the direct bundle

Result: the isolated worktree contains a valid, unstamped 0.2.0 bundle with
the selected revision and emulator epoch.

Existing implementation:

- `xtask/src/main.rs::{cmd_image_build,cmd_image_validate}`;
- `xtask/src/image.rs::{build_image,validate_manifest}`.

Proposed generated files:

- `dist/workload-image-0.2.0/` under the isolated reference-workload worktree.

Prerequisite: Package 1.

Verification:

```sh
cargo run --locked -p xtask -- image build
cargo run --locked -p xtask -- image validate \
  dist/workload-image-0.2.0/workload-image.yaml
```

Acceptance: manifest and unstamped-sidecar identity matches the selected
source, guest-sdk pin, image 0.2.0, and emulator 0.2.3; all artifact hashes and
required handoff files validate.

### Package 3 — Prove clean-root reproducibility

Result: the repository's two tracked-source packaging builds produce identical
kernel, initramfs, and manifest bytes from one pinned guest-sdk agent input.

Existing implementation:

- `xtask/src/main.rs::cmd_image_double_build`;
- `xtask/src/image.rs::{double_build,build_from_clean_root,compare_double_build_artifacts}`.

Prerequisite: Package 2 and a renewed clean-status check. It cannot run in
parallel with Package 2 because both use the isolated reference-workload
`target/` tree.

Verification:

```sh
cargo run --locked -p xtask -- image double-build
cargo run --locked -p xtask -- image validate \
  target/image-double-build/root-a/reference-workload/dist/workload-image-0.2.0/workload-image.yaml
cargo run --locked -p xtask -- image validate \
  target/image-double-build/root-b/reference-workload/dist/workload-image-0.2.0/workload-image.yaml
```

Acceptance: root-a and root-b artifacts are byte-identical with equal BLAKE3
hashes and byte counts. The direct build is validated and recorded separately.
The result does not claim two independent guest-sdk agent compilations.

### Package 4 — Persist, record, and close

Result: accepted persistent private handoff storage contains the validated
root-a bundle, the private provisional handoff binds all provenance and proof,
and Beans closes `reference-workload-9xjj`.

Proposed external outputs:

- `$REFWORK_IMAGE_HANDOFF/`, normally
  `$PRIVATE_ROOT/artifacts/workload-image-0.2.0`;
- `$PRIVATE_ROOT/evidence/image-0.2.0-handoff.txt`.

Prerequisites: Package 3 and both owner-supplied inputs. No part of Package 4
may run speculatively against an inferred or merely proposed private path.

Verification:

- Run `image validate` against the persistent handoff copy.
- Compare root-a, root-b, and persistent-handoff byte counts and BLAKE3s.
  Record the separately validated direct build without requiring cross-path
  equality.
- Read back and verify every handoff field.
- Run `bn show reference-workload-9xjj --json` and
  `bn show reference-workload-ftor --json` after closure.

Acceptance: all checks in
[03-staging-evidence-and-closure.md](03-staging-evidence-and-closure.md) pass;
the downstream issue becomes next without any claim that its external gates
are satisfied.

### Package 5 — Mandatory repository and tracker closeout

Result: Git and the shared Beans hub are synchronized, temporary worktree
metadata is pruned, and continuation context is recorded.

Prerequisite: Package 4 has closed `reference-workload-9xjj`. File a Beans
issue first for every newly discovered defect or deferred cleanup item.

Run the repository-required sequence from the primary checkout:

```sh
git pull --rebase
bn status --json
git push
git status
bn status --json
```

Require `git push` to succeed and `git status` to say the branch is up to date
with its origin. Then remove the four detached temporary worktrees, remove only
the exact `REFWORK_BUILD_ROOT` created by this execution, run `git worktree
prune`, and prune stale remote-tracking refs. File a Beans cleanup issue if any
of those exact targets cannot be removed. Do not drop pre-existing user stashes
or branches. If downstream work will continue in another session, write a
redacted `bn handoff create --file - --issue reference-workload-ftor` record
that names the persistent bundle and private evidence by variable, not by
absolute path.

## Integration and regression gates

1. `cargo test --locked -p xtask` passes before artifact generation.
2. Direct, root-a, root-b, and persistent handoff manifests all pass
   `image validate`.
3. Root-a, root-b, and persistent handoff artifact hashes agree for the kernel,
   initramfs, and manifest. Direct-build hashes are recorded separately.
4. No `determinism.last_green` exists and `image register` is not invoked.
5. `git status --short` stays empty in every isolated source worktree; the
   primary worktrees retain their pre-existing state.
6. A privacy review confirms that repository and Beans output include no
   private path or game-derived value.
7. `git push` succeeds, Git reports up to date, and both final
   `bn status --json` checks report a synchronized hub.

## Definition of done

- PR #3 acceptance is verified and its stale Beans issue is closed.
- `reference-workload-9xjj` was claimed before build execution.
- Workload image 0.2.0 is valid and byte-reproducible under the documented
  shared-agent-input limitation from the selected merged main source.
- The approved persistent handoff location contains the verified bundle.
- The private provisional handoff record contains all required provenance,
  hashes, and verdicts.
- `reference-workload-9xjj` is closed with a redacted reason.
- Temporary worktrees are removed after evidence verification.
- The primary Git branch and Beans hub are pushed and synchronized according
  to `AGENTS.md`.
- `reference-workload-ftor` is identified as the next chain step, still gated
  by its determinism-hypervisor READY ref and snapshot-store service.

## Deferred work and follow-up

- `reference-workload-ftor`: build the worker, regenerate lab expectations,
  run the 20x and negative VM suites, write the green stamp, register the image,
  finalize the handoff, and run map-check.
- `refwork-5tk`: capture and freeze the real >=1000-state corpus after the
  re-stamp and deployed-stack gates pass.
- `refwork-ob3` items 4–5: consume the full corpus and run the exploration
  readiness smoke in the later plan.
- Any image build, validation, or reproducibility defect discovered here must
  receive a separate Beans issue before source changes begin.
