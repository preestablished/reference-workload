# Review: Ralph iteration 10 M4 external-readiness evidence

## Findings

1. `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md:29` records `Reference-workload rev` as `01535d8b072be49c4031e83f44796cba2cc82edd`, which is `main` and `HEAD^`, not the reviewed branch head `9a6f8d48af8e6e5689353e63f72dcb6edf4c8891` that introduced this evidence note. Because the file exists only in the branch commit, this field cannot identify the reviewed artifact and is misleading for later traceability. Prior evidence notes distinguish checked source rev from the evidence-note commit; this note should do the same or record the branch head as the evidence note revision.

## Checks Performed

- Compared `main...ralph/iteration-10-record-m4-external-readiness-evidence`; the branch adds only `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`.
- Verified sibling checkout revisions and clean worktrees:
  - `../guest-sdk` at `08abbbc36f6afa6ad3aec0ce062c3383f8dcfcce`
  - `../determinism-hypervisor` at `b9737538f5fc2708d9cb09979df775c0ab388390`
  - `../snapshot-store` at `cac52afe66b0975601bc9ecbc67cd16b52cc181e`
  - `../control-plane` at `ca9ee9048d7fca8eec5fe512011b011128e2b0c3`
- Verified the cited hypervisor artifact root exists at `../determinism-hypervisor/target/m9-final-acceptance-20260621T004402Z/`.
- Verified the note's listed BLAKE3 hashes for `08-linux-ready.log`, `14-linux-m4-transparency.log`, `18-linux-worker-api.log`, and `06-artifacts-and-cache.log`.
- Verified the staged M9 artifact hashes for `bzImage`, `initramfs.cpio`, `base.img`, and `game.img`; `game.img` is byte-identical to local `target/synth-rom.rom`, so treating it as staged synthetic fixture evidence rather than operator-ROM evidence is correct.
- Verified the guest-sdk architecture citation path `../guest-sdk/prompts/docs/guest-sdk/ARCHITECTURE.md` and the cited READY/control sections exist.
- Searched the local preestablished workspace and relevant caches for operator `.rom`, `.padlog`, `first-room`, `rw3-report`, and related evidence paths. I found no locally available operator ROM, first-room padlog, room-transition report, or framebuffer checkpoint evidence beyond the planned note and synthetic/staged fixtures.

## Residual Gaps

- The note remains correctly blocked on operator-game first-room evidence: no owner/run assignment, operator ROM hash, padlog hash, implemented command/API entry point, report path/hash, room-transition proof, or framebuffer checkpoint hash was locally available.
- The selected frame-scheduling row cites data from `15-linux-m5-frame-scheduling.log`, whose BLAKE3 is locally `ae76a534fe1c8a7847f06fa1737502cf0f50bd56c45ca1108c4947378070e055`; the note gives the artifact root but not this individual file hash. I did not classify that as a blocking finding because the row is supplemental staged-fixture context, not the operator first-room evidence being claimed.
