# Positive Notes

- `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:5-7` preserves the clean-room boundary clearly: the evidence note allows command results, hashes, revisions, and artifact pointers while excluding ROM bytes, framebuffer goldens, WRAM dumps, and padlog semantics.

- `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:11-14` correctly states that real operator-game/in-VM evidence remains blocked until lab artifacts are supplied. That distinction is important and should be preserved after the waiver provenance is strengthened.

- `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:86-106` does a good job separating ownership boundaries between reference-workload, guest-sdk, determinism-hypervisor, and control-plane instead of restating cross-repo schemas as if this repo owned them.

- `.agents/plans/guest-sdk-unblock-reference-workload/02-harness-state-machine.md:91-104` and `.agents/plans/guest-sdk-unblock-reference-workload/05-in-vm-first-room-gate.md:41-46` capture the subtle READY timing distinction: harness `Ready { frame: 0 }` precedes `Start`, while guest-sdk READY/root snapshot happens after `Start` with `meta.status=ready` and `meta.frame=0`.

- `.agents/plans/guest-sdk-unblock-reference-workload/06-full-determinism-suite.md:60-72` preserves the "test the tester" negative gate while keeping the intentionally nondeterministic build out of normal source and CI. That is the right maintainability boundary.

- `.agents/plans/guest-sdk-unblock-reference-workload/07-ci-evidence-closeout.md:80-86` is a useful evidence hygiene rule set: chat/scrollback does not close blockers, mismatched revisions invalidate lab evidence, and flaky gates must not green-stamp an image.
