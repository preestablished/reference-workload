## Action Items

### Critical

- [ ] None.

### Important

- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:91] Add operator-approved waiver provenance and record/cross-link it in the phase-2 bring-up log.
- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:57] Add an applicability proof for citing cross-arch evidence from SHA `9afaa0a` against this branch/base, or rerun the evidence at the applicable SHA.
- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:14] Make DH-1 / hypervisor Linux direct-boot evidence a package-04 closeout dependency, while allowing only asset-prep work before it.
- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/02-harness-state-machine.md:59] Replace "page-sized" with exact page-multiple region-size requirements for `wram`, `framebuffer`, `meta`, and optional regions.

### Suggestions

- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:29] Reword the verified SHA/branch line so local base, branch head, and CI evidence SHA are not conflated.
- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:28] Record the sibling control-plane worktree clean/dirty state alongside the proto rev.
- [ ] [.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:76] Define the unstamped WorkloadImage state without inventing fields outside API.md.
