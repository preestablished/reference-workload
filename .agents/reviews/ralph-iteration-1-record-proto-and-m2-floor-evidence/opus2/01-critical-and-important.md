# Critical And Important

## Critical

No Critical issues found.

## Important

### I-1 - Waiver lacks operator-grade provenance and is not recorded in the phase-2 bring-up log

Severity: Important

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:91-98`

Description: The waiver is the mechanism that lets synthetic M3 and M4 preparation proceed despite the missing host-side operator-game first-room evidence. The upstream RW-0 language requires an operator waiver with date, owner, reason, and scope, recorded in the bring-up log. This note says the owner is Matt Spurlin and that it was recorded by Codex during `/ralph`, but it does not identify the actual approving operator, approval artifact, or bring-up-log entry. A future gate reader could treat this as an authorized operator waiver when it is only a local evidence assertion.

Suggested fix:

```markdown
### Waiver

| Field | Value |
|---|---|
| Date | 2026-06-21 |
| Approved by | <operator name and role, not just recording agent> |
| Approval artifact | <issue/comment/lab-note path proving the waiver was granted> |
| Recorded in | `.agents/plans/phase-2/bringup-log.md` waiver/provenance entry |
| Reason | Operator-game lab artifacts are not available in this checkout, and the feature map remains explicitly placeholder/unvalidated. |
| Scope | Waives only host-side operator-game first-room, map-check, and real-hardware aarch64 demo-game evidence before synthetic M3 harness/mock-agent and asset-only M4 preparation. |
| Non-scope | Does not close RW-0 as M2 complete, does not close RW-3/RW-4, and does not waive image reproducibility, in-VM first-room readiness, package 05, package 06, or final M2/M5 lab acceptance. |
| Required follow-up | Replace this waiver with lab artifact pointers before any package 05/06 closure or phase-3 entry claim. |
```

### I-2 - Cross-architecture evidence is from a different SHA without an applicability proof

Severity: Important

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:57-67`, `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:107`

Description: The evidence note maps the x86_64/aarch64 acceptance clause to a nightly run at `9afaa0a69a3ea57ed4e10ff29a53b716b5559990`, while this review branch starts from `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63` and ends at `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1`. I verified locally that the SHA difference appears to be bead metadata and `.gitignore` only, but the evidence note itself does not say that. Without that bridge, the note overstates the proof by claiming current-checkout acceptance from artifacts built at another revision.

Suggested fix:

~~~markdown
## Cross-Architecture Evidence Applicability

The downloaded nightly evidence was produced at `9afaa0a69a3ea57ed4e10ff29a53b716b5559990`.
This checkout's base/source rev is `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63`; review
branch head is `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1`.

Applicability check:

```sh
git diff --name-only 9afaa0a69a3ea57ed4e10ff29a53b716b5559990..8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63
```

Result: only bead metadata and `.gitignore` changed; no crates, Cargo manifests,
feature maps, scoring files, xtask gates, or CI workflow inputs changed. If any
source/test/gate/input file differs in a future branch, rerun the cross-arch hash job
at that branch or base SHA before citing it as RW-0 evidence.
~~~

### I-3 - Package 04 can be read as closing RW-2 without the upstream DH-1 dependency

Severity: Important

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:68-71`, `.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:14-19`, `.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:102-113`

Description: The upstream reference-workload unblock plan says RW-2 depends on RW-1 and DH-1. The new graph routes package 04 directly after packages 02/03, and package 04 says the hypervisor Linux boot floor is only available for "final smoke" while its acceptance criteria do not require any DH-1 citation before closing the handoff. That creates a task-graph ambiguity: implementation agents could mark package 04 and `guest-sdk-ext-refwork-m4-image-handoff` complete with deterministic files that have never been checked against the Linux direct-boot floor the owner graph requires.

Suggested fix:

```markdown
## Dependencies

- Packages 02 and 03 complete.
- DH-1 / hypervisor Linux direct-boot floor cited before RW-2 closeout.
  Asset-only preparation may proceed before DH-1, but it must not close
  `guest-sdk-ext-refwork-m4-image-handoff` or claim package-04 acceptance.
- A pinned or prebuilt `detguest-agent` from guest-sdk...

## Acceptance

- A DH-1 artifact or CI/lab run is cited showing the package-04 kernel/initramfs
  shape is compatible with the hypervisor Linux direct-boot baseline. This is not
  real-agent READY validation; package 05 still owns that.
```

### I-4 - Region allocation wording says "page-sized" instead of exact page-multiple region sizes

Severity: Important

Path and lines: `.agents/plans/guest-sdk-unblock-reference-workload/02-harness-state-machine.md:58-68`

Description: "Allocate page-aligned, page-sized regions before `Core::new`" is dangerously precise systems wording. `wram` and `framebuffer` are not one page; they must be exact API-sized, page-aligned, page-multiple mappings. A literal implementation would allocate 4096-byte regions, breaking `RegionBuffers`, feature-map reads, framebuffer capture, and guest-sdk region publication.

Suggested fix:

```markdown
5. Region buffers:
   - Allocate page-aligned mappings whose lengths are exact page multiples and
     match the API/manifest sizes before `Core::new`: `wram` = 131072 bytes,
     `framebuffer` = 229376 bytes, `meta` = 4096 bytes; optional `vram`/`sram`
     use their manifest/configured sizes.
```
