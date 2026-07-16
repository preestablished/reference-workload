# Review: Ralph Iteration 9 Image Double-Build And Register Guard

## Findings

### High: `image double-build` can pass using dirty reference-workload sources

References: `xtask/src/image.rs:288`, `xtask/src/image.rs:291`, `xtask/src/image.rs:397`, `xtask/src/image.rs:420`, `.agents/plans/guest-sdk-unblock-reference-workload/m4-image-handoff-evidence.md:55`

`double_build` records `git rev-parse HEAD` for the manifest and checks only the sibling `control-plane` checkout for cleanliness. It then materializes each clean root with `git ls-files` plus `std::fs::copy` from the current worktree. That copies modified tracked files and staged-but-uncommitted files into both roots while the manifest still claims the artifacts came from `HEAD`. Because both roots receive the same dirty bytes, the byte comparison still passes.

This does not meet the package-04 acceptance language that `image double-build` proves byte-identical output from clean checkouts/build roots. It can produce green evidence for source that cannot be reconstructed from the recorded implementation rev. The branch also lacks a test that dirty reference-workload state is rejected or that the clean roots are materialized from the committed tree.

Fix: either reject a dirty source checkout before building, or materialize both roots from `git archive HEAD`/tree objects and keep `HEAD` as the recorded source rev.

### Medium: `image register --require-green-stamp` accepts any file named `determinism.last_green`

References: `xtask/src/image.rs:347`, `xtask/src/image.rs:353`, `xtask/src/image.rs:361`, `xtask/src/image.rs:1831`, `xtask/src/image.rs:1835`

When the green-stamp path exists, `register_image` returns `DirectDistStamped` without parsing the stamp or checking that it matches the manifest hash/git rev/report hash. The unit test codifies this by writing `b"green"` to `determinism.last_green` and accepting it under `require_green_stamp = true`.

That weakens the package-06 handoff guard: once callers pass `--require-green-stamp` or the sentinel/evidence file makes green stamps mandatory, a stale, empty, or dummy sidecar can register as "green stamp present". The package-06 plan says the stamp/report carries suite version, git rev, CI/lab timestamp, and report hash, and register should refuse images without a fresh green suite report.

Fix: define the minimal `determinism.last_green` schema now, validate that it references this manifest hash and source rev, and add negative tests for malformed/stale stamps.

### Medium: M4 evidence omits the required DH-1 direct-boot baseline citation

References: `.agents/plans/guest-sdk-unblock-reference-workload/m4-image-handoff-evidence.md:25`, `.agents/plans/guest-sdk-unblock-reference-workload/m4-image-handoff-evidence.md:53`, `.agents/plans/guest-sdk-unblock-reference-workload/m4-image-handoff-evidence.md:116`, `.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:113`

The evidence records local build/register/double-build outputs and then says the package proves deterministic package-04 handoff construction. It does not cite a DH-1 artifact, CI run, or lab run showing the kernel/initramfs shape is compatible with the hypervisor Linux direct-boot baseline.

The package-04 acceptance criteria require that citation before claiming RW-2/M4 handoff acceptance. Without it, the evidence is useful deterministic-build evidence, but not complete package-04 handoff evidence.

Fix: add the DH-1 artifact/run reference, revision, and relevant result to the evidence note, or explicitly mark the note as pre-acceptance evidence that must not close package 04.

## Verification

- `cargo test -p xtask` passed.
- `cargo run --locked -p xtask -- image register --manifest dist/workload-image-0.1.0/workload-image.yaml` passed locally against the generated ignored `dist/` bundle.
- `cargo run --locked -p xtask -- image double-build` passed locally and produced byte-identical `bzImage`, `initramfs.cpio.zst`, and `workload-image.yaml`.
