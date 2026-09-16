# Epoch 0.2.3 Workload Image Build

Status: Blocked

Planning is complete. Implementation has not occurred.

## Application context

```json
{
  "application_context": {
    "has_active_users": false,
    "backward_compatibility_required": false,
    "feature_flags": "not-applicable",
    "confirmation_digest": "05bc2398fdb80d3c0b7c3f40342098faca1c6e3d113bf932c520867bc065f30b",
    "confirmed_at": "2026-09-15T03:14:24Z"
  }
}
```

The image is an internal, not-yet-consumed artifact. Do not retain a 0.1.x
manifest shape, create a compatibility alias, or add a feature flag. Generate
only the 0.2.0 bundle and its current provenance fields.

## Target and outcome

Beans returns three ready P1 issues rather than a unique winner. The selection
gate below chooses the epoch 0.2.3 re-baseline chain because its first item is
already complete outside Beans, its next item matches this Intel/KVM host, and
it unlocks the P0 corpus that also gates the remaining M6 work. GitHub PR #3 is
merged at `ce754be1d83dbc212a1308c06e63f7cc612c5bc7`, but
`reference-workload-qglp` still records the merge as open. After reconciling
that stale issue, the next task is `reference-workload-9xjj`: build and validate
`refwork-demo@0.2.0` on the Intel box, run the repository's clean-root
double-build proof, and publish a provisional private handoff record whose
bundle paths satisfy the determinism-hypervisor artifact inputs.

This is an operational artifact-build change. It affects tracker state,
checkout topology, generated `dist/` output, and private handoff evidence. It
does not require repository source edits.

## Success criteria

1. `reference-workload-qglp` is closed only after `origin/main` contains merge
   commit `ce754be1d83dbc212a1308c06e63f7cc612c5bc7`, workspace version `0.2.0`,
   and manifest generation of `meta.built_from.emu_version`.
2. `reference-workload-9xjj` is claimed before its build artifacts are
   produced.
3. A clean build checkout at the selected `origin/main` commit uses the
   `guest-sdk` revision in `image/guest-sdk.lock`, the kernel whose BLAKE3 is in
   `image/kernel.lock`, Zstandard 1.5.5, and the
   `x86_64-unknown-linux-musl` Rust target.
4. `cargo run --locked -p xtask -- image build` creates
   `dist/workload-image-0.2.0/`, and `image validate` accepts its manifest.
5. The manifest records `meta.version: "0.2.0"`, the exact reference-workload
   source revision, the pinned guest-sdk revision, and
   `meta.built_from.emu_version: "refwork-emu 0.2.3"`.
6. `determinism.unstamped.yaml` exists, includes the same source and emulator
   versions, and does not coexist with `determinism.last_green`.
7. `cargo run --locked -p xtask -- image double-build` reports identical
   `bzImage`, `initramfs.cpio.zst`, and `workload-image.yaml` bytes and hashes
   under its tracked-source packaging contract. The record does not claim an
   independent second guest-sdk agent compilation because the command shares
   that checkout's Cargo cache.
8. The downstream owner accepts the persistent handoff directory and artifact
   naming before it receives the validated root-a bundle.
   Its `bzImage` and `initramfs.cpio.zst` are usable as `DH_M9_BZIMAGE` and
   `DH_M9_INITRAMFS`. The private handoff record binds revisions, hashes, tool
   versions, date, and double-build verdict without private literals in
   repository or tracker output.
9. `reference-workload-9xjj` closes only after the persistent bundle and private
   handoff record are verified. Its closure makes `reference-workload-ftor`
   ready; it does not claim the later green stamp, registration, VM suite, or
   capture corpus.

## Scope

- Reconcile the already-merged PR with Beans.
- Create a clean, isolated sibling-checkout layout without changing the
  user's dirty primary worktrees.
- Build, validate, and double-build workload image 0.2.0.
- Copy the validated bundle to persistent private handoff storage and map its
  two guest artifacts to the downstream determinism-hypervisor inputs.
- Write the provisional private handoff record and update Beans.

## Non-goals

- Do not change Rust, shell, workflow, manifest, lock, or image-input source.
- Do not rebuild or repin the kernel.
- Do not repin guest-sdk to its current `main` revision.
- Do not run `xtask image register`; `reference-workload-ftor` owns green
  stamping and registration.
- Do not run KVM VM gates, deploy the stack, create snapshots, or capture the
  real corpus.
- Do not commit `dist/`, ROM data, private paths, decoded values, WRAM offsets,
  framebuffer data, or the private handoff record.
- Do not preserve a 0.1.x artifact contract or workflow.

## Repository-grounded findings

- `Cargo.toml` sets the workspace version to 0.2.0.
- `crates/refwork-emu/src/lib.rs::EMU_VERSION` is `refwork-emu 0.2.3` and is
  intentionally independent of the image version.
- `xtask/src/image.rs::build_image_with_git_rev` replaces an existing
  `dist/workload-image-0.2.0/`, builds a musl harness, copies the pinned kernel,
  creates the deterministic initramfs, writes the manifest and unstamped
  sidecar, then validates the result.
- `xtask/src/image.rs::double_build` refuses a dirty reference-workload
  checkout, a dirty control-plane checkout, changes under
  `determinism-hypervisor/{crates/dh-proto,proto}`, or changes under
  `guest-sdk/{crates,Cargo.toml,Cargo.lock}`. It builds twice from tracked
  source and compares the three contract artifacts byte for byte.
- The primary reference-workload checkout contains unrelated untracked
  `.agents/plans/validate-private-phase4-contracts/`, `.beads/`, and `data/`.
  The primary control-plane checkout also contains an unrelated untracked plan.
  These belong to the user and must remain untouched.
- The checked-out guest-sdk `main` is newer than the required
  `acb1d3e8514a35eff66e94696d3a497b21328323`, but that pinned commit is present
  locally. The existing `guest-sdk/image/build/bzImage` matches the BLAKE3 in
  `image/kernel.lock`.
- The host is `x86_64`, `/dev/kvm` exists, Zstandard reports 1.5.5, and the
  musl target is installed. Rust is 1.98.1, Cargo is 1.98.1, and rustfmt is
  1.9.0-stable, matching the handoff's Rust 1.98 requirement. KVM is not used
  by this package.
- The required private-root pointer
  `~/.agents/projects/reference-workload/private-root.path` is absent.
- `determinism-hypervisor/crates/dh-worker/src/m9_handoff.rs` consumes the
  guest artifacts through `DH_M9_BZIMAGE` and `DH_M9_INITRAMFS`. Plan B owns
  `DH_M9_BASE_IMAGE`, `DH_M9_GAME_IMAGE`, and `DH_M9_IMAGE_CACHE`.
- The planning baselines are control-plane
  `e07b1eb3ac97024492211a63f9ffca401ea96249` and determinism-hypervisor
  `60004db545825c08118d82c4fbd13ae1a1d28054`. Neither repository has a lock
  file here, so a different revision requires operator acceptance before build.

## Beans selection gate

| Ready P1 | Evidence | Disposition |
|---|---|---|
| `reference-workload-qglp` | PR #3 is merged; closure unlocks `reference-workload-9xjj`, then `reference-workload-ftor`, then P0 `refwork-5tk`. | Reconcile it, then execute `reference-workload-9xjj` on this Intel/KVM host. |
| `refwork-ob3` | Its 2026-09-14 log says items 4–5 remain gated on the Intel-box stack and `refwork-5tk`; the existing resolution package also requires the full corpus and smoke. | Do not select independently; the chosen epoch chain advances its blockers. |
| `refwork-279` | Acceptance explicitly requires the lab Mac, an F310 in D mode, and a human on-hardware checklist. | Defer because this host is `x86_64` Linux and cannot discharge the task. |

If these facts change before execution, rerun `bn ready --json` and return the
choice to the operator rather than silently applying this tie-break.

## Change model

```text
PR #3 merged, Beans stale
        |
        v
verify merge acceptance -> close qglp -> claim 9xjj
        |
        v
isolated clean sibling worktrees at accepted revisions
        |
        v
build -> validate -> repository clean-root double-build
        |
        v
persistent private bundle + DH_M9 artifact mapping + provisional handoff
        |
        v
close 9xjj -> reference-workload-ftor becomes ready
```

## Key decisions

1. Use isolated detached worktrees beneath a temporary build root. This
   preserves unrelated user changes and satisfies `double_build` cleanliness
   checks without stashing, cleaning, or switching the primary sibling repos.
2. Build the reference-workload worktree at the merged `origin/main` commit,
   not at this plan branch. The generated `git_rev` must identify shipped
   implementation source, not planning documentation.
3. Use the exact guest-sdk lock revision. A newer clean guest-sdk checkout is
   still invalid because `build_agent_from_pinned_sibling` compares `HEAD`
   exactly.
4. Keep the bundle unstamped and do not call register. The later
   `reference-workload-ftor` package owns the 20-run VM proof, green stamp,
   sentinel, and registration.
5. Treat `dist/` as disposable generated output and the persistent private
   bundle plus evidence record as the durable handoff. The built-in
   double-build proves tracked-source packaging reproducibility while reusing
   the pinned guest-sdk checkout and Cargo cache. Rollback is rejection of the
   unconsumed private bundle.

Rejected alternatives:

- Do not clean or stash the primary worktrees. That risks user-owned untracked
  data and is unnecessary.
- Do not pass `--agent-bin` to bypass the guest-sdk revision gate. The normal
  build must prove the checked-in lock.
- Do not close `reference-workload-9xjj` after only the first build. Validation,
  double-build equality, persistent handoff, and private evidence are part of
  the issue.

## Blockers and gates

Blocking, owner: operator. Create
`~/.agents/projects/reference-workload/private-root.path` with the approved
private-root location, and make its `evidence/` directory writable on the
Intel box. The implementer must resolve the pointer without printing its
contents.

Blocking, owner: determinism-hypervisor epoch-0.2.3 operator. Restore the
canonical Plan B staging contract or explicitly accept a replacement that
names the persistent directory, exact artifact filenames, and custody through
`reference-workload-ftor`, including whether that task operates in place or
copies the bundle back to `dist/workload-image-0.2.0/` at the recorded source
revision. The proposed candidate is
`REFWORK_IMAGE_HANDOFF="$PRIVATE_ROOT/artifacts/workload-image-0.2.0"`, with
`$REFWORK_IMAGE_HANDOFF/bzImage` supplied as `DH_M9_BZIMAGE` and
`$REFWORK_IMAGE_HANDOFF/initramfs.cpio.zst` supplied as `DH_M9_INITRAMFS`.
Plan B owns the other three `DH_M9_*` inputs. The proposal is not accepted
evidence until the downstream owner records approval.

Do not claim either tracker issue or start artifact generation until both
blockers resolve. This prevents completed output from being stranded in a
temporary directory without a durable handoff or resumable session record.

## Assumptions and risks

- Assumption: the merge commit remains the selected `origin/main` source. If
  main advances, record the new commit and re-run the acceptance checks; do not
  silently build an unreviewed source revision.
- Risk: a temporary worktree placed under the reference-workload checkout is
  included in cleanliness checks. Place the build root outside every source
  checkout.
- Risk: the build command removes an existing 0.2.0 output directory. Run it
  only inside the isolated worktree.
- Risk: terminal output or Beans notes can disclose private paths. Use variable
  names and hashes only; keep full paths inside the private handoff file.
- Risk: a successful `image validate` does not prove reproducibility. Require
  the separate double-build report before private handoff.
- Risk: the built-in double-build shares one guest-sdk Cargo target and does
  not independently prove agent compilation determinism. Record the pinned
  agent binary hash as an input and do not overstate the result.

## Document map

- [01-readiness-and-clean-build-layout.md](01-readiness-and-clean-build-layout.md)
  — reconcile tracking, claim the task, and construct the isolated build layout.
- [02-build-validate-and-double-build.md](02-build-validate-and-double-build.md)
  — produce the bundle and prove its manifest and byte reproducibility.
- [03-staging-evidence-and-closure.md](03-staging-evidence-and-closure.md)
  — persist the bundle, write private evidence, close the task, and hand off.
- [04-execution-handoff.md](04-execution-handoff.md) — dependency-ordered
  execution packages, commands, gates, and definition of done.
