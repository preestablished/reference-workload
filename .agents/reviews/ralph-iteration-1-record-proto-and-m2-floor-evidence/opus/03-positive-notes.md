# Positive Notes

- `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:11` clearly separates repo-side synthetic M2 evidence from absent operator-game first-room evidence and keeps packages 05/06 blocked until lab artifacts exist.
- `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:55` records the nightly run ID, URL, head SHA, job names, artifact paths, and matching x86_64/aarch64 synthetic hashes. That is the right shape for reproducible evidence.
- `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:86` preserves clean-room constraints explicitly: no game names, ROMs, framebuffer goldens, or lab dumps in the repo.
- `.agents/plans/guest-sdk-unblock-reference-workload/00-overview.md:92` cleanly separates ownership across reference-workload, guest-sdk, determinism-hypervisor, and control-plane, which reduces future protocol drift.
- `.agents/plans/guest-sdk-unblock-reference-workload/02-harness-state-machine.md:35` keeps the fd-3 `SOCK_SEQPACKET` transport and strict request/response protocol front and center.
- `.agents/plans/guest-sdk-unblock-reference-workload/02-harness-state-machine.md:137` explicitly rejects `UnixDatagram` and per-frame pad traffic over the control socket, preserving the intended harness/agent boundary.
- `.agents/plans/guest-sdk-unblock-reference-workload/04-image-handoff-assets.md:79` correctly keeps region `layout_version` in the guest-sdk handoff files unless the reference-workload API is updated to own that field.
- `.agents/plans/guest-sdk-unblock-reference-workload/07-ci-evidence-closeout.md:80` gives useful stop conditions for evidence durability, especially rejecting chat/scrollback-only evidence and flaky green stamps.
