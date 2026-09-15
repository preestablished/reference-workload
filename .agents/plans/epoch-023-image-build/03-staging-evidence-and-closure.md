# 03 — Staging, Evidence, and Closure

## Goal and prerequisite state

Place the validated unstamped bundle in persistent private handoff storage,
bind its two guest artifacts to the downstream epoch-0.2.3 input contract,
write a private provisional evidence record, and close
`reference-workload-9xjj`. Package 2 must pass. Both blockers in
[00-overview.md](00-overview.md) must be resolved before this package starts.

## Repository and tracker evidence

- `image/README.md` defines the bundle as a handoff without ROM or game-derived
  content and states that green-stamp enforcement belongs to the downstream
  epoch plan.
- `reference-workload-9xjj` requires a provisional
  `$PRIVATE_ROOT/evidence/image-0.2.0-handoff.txt` and a persistent bundle for the
  determinism-hypervisor plan.
- `reference-workload-ftor` is blocked by `reference-workload-9xjj` and owns the
  scratch worker, 20-run VM suite, negative test, green stamp, register step,
  lab expectations, and finalization of the handoff record.
- `refwork-5tk` remains blocked by `reference-workload-ftor` and owns the real
  capture corpus. Completing this package must not imply those results.

## Change surface

Proposed external outputs:

- `$REFWORK_IMAGE_HANDOFF/` — proposed persistent private bundle directory,
  normally resolved as `$PRIVATE_ROOT/artifacts/workload-image-0.2.0`;
- `$PRIVATE_ROOT/evidence/image-0.2.0-handoff.txt` — proposed private
  provisional handoff record beneath an operator-approved existing private
  root.

Tracker mutations through `bn`:

- redacted completion note and closure for `reference-workload-9xjj`;
- no status change to `reference-workload-ftor` or `refwork-5tk`.

No repository file changes.

## Procedure

1. Resolve `PRIVATE_ROOT` from its pointer without printing it. Set
   `REFWORK_IMAGE_HANDOFF` to the exact persistent bundle location accepted by
   the downstream owner. Verify it is absolute, outside every repository, and
   on the intended Intel-box storage. The proposed default is not authoritative
   without that acceptance.
2. Create a uniquely named temporary directory beside the approved final
   handoff path. Copy root-a's validated `workload-image-0.2.0` directory with
   non-interactive file operations. Never overwrite an existing accepted
   bundle in place; do not copy the direct-build directory.
3. Validate the copied manifest at its destination with the same selected
   reference-workload binary or source checkout. Re-hash the three contract
   artifacts and require equality with both root-a and root-b before atomically
   renaming the temporary directory to the approved final name. Record the
   direct-build hashes separately without requiring cross-path equality.
4. Verify that the downstream owner's accepted contract sets `DH_M9_BZIMAGE` to
   `$REFWORK_IMAGE_HANDOFF/bzImage` and `DH_M9_INITRAMFS` to
   `$REFWORK_IMAGE_HANDOFF/initramfs.cpio.zst`. Record that mapping privately.
   Do not create or populate `DH_M9_BASE_IMAGE`, `DH_M9_GAME_IMAGE`, or
   `DH_M9_IMAGE_CACHE`; Plan B owns them. Require the contract to state whether
   `reference-workload-ftor` operates on this persistent bundle or copies it
   back to `dist/workload-image-0.2.0/` in a checkout at the recorded source
   revision before stamping.
5. Inspect any existing `image-0.2.0-handoff.txt` and obtain owner disposition
   before replacement. Under `umask 077`, write a uniquely named temporary
   record in `$PRIVATE_ROOT/evidence/`, mark it `provisional`, and record:

   - UTC date and build-host identifier;
   - final private bundle location;
   - reference-workload source revision;
   - control-plane and determinism-hypervisor input revisions;
   - guest-sdk revision;
   - kernel BLAKE3;
   - initramfs BLAKE3;
   - manifest BLAKE3;
   - manifest `meta.version` and `meta.built_from.emu_version`;
   - Zstandard version and Rust toolchain identity;
   - first-build validation verdict;
   - root-a and root-b validation verdicts;
   - double-build byte-equality verdict for all three artifacts;
   - explicit statement that no green stamp or registration has occurred;
   - downstream owner `reference-workload-ftor`.

6. Read the private temporary record back and compare every revision and hash
   against the generated files. Run the repository's redaction scanner if its
   forbid-list contract is available privately. At minimum, ensure no private
   path or game-derived value is copied into a repository file, command
   transcript, or Beans message. Require mode `0600`, then atomically rename it
   to `image-0.2.0-handoff.txt` only when the final target is absent or
   replacement was explicitly approved.
7. Add a redacted Beans note to `reference-workload-9xjj` containing only the
   source revision, the three artifact hashes, the double-build verdict, and a
   statement that the private provisional record and persistent bundle
   exist. Do not include either absolute path.
8. Close `reference-workload-9xjj` with a reason that names the 0.2.0 build,
   successful validation, byte-identical double-build, provisional handoff,
   and persistent-handoff completion.
9. Run `bn ready --json` and `bn show reference-workload-ftor --json`. Require
   the build issue to be closed and the re-stamp task to have no remaining
   reference-workload issue blocker. External plan-B and plan-C gates can still
   prevent execution and must remain explicit.
10. Confirm the persistent bundle and handoff record again after tracker
    closure. Retain the temporary worktrees until the mandatory session
    closeout. The accepted downstream contract defines how
    `reference-workload-ftor` consumes `REFWORK_IMAGE_HANDOFF`; it must not
    depend on the temporary build checkout.

## Invariants and error paths

- Copy to a temporary sibling and validate before final rename. Consumers must
  never observe a partial bundle.
- Existing final handoff content is a stop condition. Compare it by hash and
  ask the handoff owner whether to reuse or supersede it; do not overwrite.
- A post-copy hash mismatch invalidates the handoff attempt. Remove or
  quarantine only the new temporary copy and keep the issue open.
- The private record may contain private paths; repository and Beans outputs
  may contain only hashes, revisions, public versions, and redacted status.
- `provisional` means unstamped and unregistered. Only
  `reference-workload-ftor` can change that state to final.
- Cleanup starts only after the persistent bundle and private record are
  independently verified and the mandatory push succeeds. File a cleanup issue
  if exact temporary-worktree removal fails.

## Verification and acceptance

- The persistent handoff directory contains exactly the validated 0.2.0 bundle
  shape from Package 2.
- Destination hashes match root-a and root-b. The separately valid direct build
  is not required to match across its different physical build path.
- The private handoff record exists, is marked provisional, contains every
  required provenance field and verdict, and has mode `0600`.
- Public tracker output contains no private absolute path or game-derived data.
- Beans reports `reference-workload-9xjj` closed and exposes
  `reference-workload-ftor` as the next issue in this chain, subject to its
  external READY-ref and snapshot-store gates.
- The user's primary worktrees and pre-existing untracked files remain
  unchanged.

## Exclusions

- Do not publish the bundle outside the approved private handoff directory.
- Do not mark the private handoff final.
- Do not close the epic, re-stamp issue, or corpus issue.
- Do not remove any primary checkout or user-owned untracked directory.
