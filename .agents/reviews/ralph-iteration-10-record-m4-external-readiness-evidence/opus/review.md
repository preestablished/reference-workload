# Review: Ralph Iteration 10 M4 External Readiness Evidence

## Findings

1. **Medium - Reference-workload revision is inaccurate or ambiguous.**  
   `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md:29` records `Reference-workload rev` as `01535d8b072be49c4031e83f44796cba2cc82edd`, but this evidence note exists only at branch head `9a6f8d48af8e6e5689353e63f72dcb6edf4c8891`; `01535d8...` is the `main` merge base. If the intent is to cite the product/base code excluding the evidence-only commit, the field should say that explicitly and also record the evidence-note commit. As written, a consumer following the listed repo revision cannot retrieve the evidence file, which weakens the durable provenance the bead asks for.

2. **Low - Frame-scheduling evidence is summarized without its hashed source log.**  
   `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md:44` and `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md:80` cite Linux frame scheduling and exact frame tables, but the `Key files and hashes` table at `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md:61` omits `15-linux-m5-frame-scheduling.log`, the log that contains those frame tables. The file exists under the cited M9 artifact root and hashes to `ae76a534fe1c8a7847f06fa1737502cf0f50bd56c45ca1108c4947378070e055`; include that path/hash if the frame-scheduling rows are part of the durable partial-DH-2 citation.

## Verified Context

- The branch changes only `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`.
- Sibling checkout revisions match the note for `guest-sdk`, `determinism-hypervisor`, `snapshot-store`, and `control-plane`; all four sibling working trees were clean when checked.
- The cited M9 artifact root exists, and the listed BLAKE3 hashes for `08-linux-ready.log`, `14-linux-m4-transparency.log`, `18-linux-worker-api.log`, and `06-artifacts-and-cache.log` match `b3sum`.
- The READY fields, M4 transparency hashes/diffs, worker API test names, runner name/labels, tested hypervisor SHA, host kernel/CPU/microcode, and staged `bzImage`/`initramfs.cpio`/`base.img`/`game.img` hashes match the cited hypervisor artifacts.
- The note correctly distinguishes staged M9 Linux readiness from missing operator-game first-room evidence: operator ROM BLAKE3, first-room padlog BLAKE3, durable first-room owner/run assignment, and exact implemented first-room command/API entry point are all marked missing.
- The note does not fake package-05 readiness. `refwork-d7t.10` remains open, its bead text says it may remain open until sister repos provide evidence, and `refwork-d7t.11` remains dependent on it.

## Residual Gaps

Package-05 is still blocked until a real operator-game run records owner, runner, artifact root, external revisions, package-04 manifest hash, operator ROM hash, padlog hash, implemented command/API invocation, report path/hash, READY proof, room-transition proof through host region capture, and framebuffer checkpoint hash proof.
