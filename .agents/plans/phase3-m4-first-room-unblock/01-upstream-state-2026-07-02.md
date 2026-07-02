# Upstream State As Of 2026-07-02

Everything below is verified, not aspirational — sources are cited so you
can re-check rather than trust.

## guest-sdk: Ms4 Is Real And Accepted

Commits `683527f`/`cdb1cf6`/`604cd41` on guest-sdk `main`; full request/
resolution/verification trail in
`~/git/preestablished/guest-sdk/.agents/requests/phase3-ms4-region-publication-acceptance/`
(files 00–06). What it means for this repo:

- `detguest-sdk::register_region` is the **real** path now: mlock +
  per-page prefault in the workload, registration with the agent over
  `/run/detguest/agent.sock` (AF_UNIX SOCK_SEQPACKET). Standalone mode
  returns `AgentUnavailable` instead of the old validate-and-no-op handle.
  Handles unregister on drop — hold them for process lifetime.
- The agent is the **sole manifest writer** (SO_PEERCRED-bound pid,
  pagemap GVA→GPA extent walk); `ReverifyRegions` detects drift and
  unmapping with P0 alarms.
- The 100× snapshot/restore readability acceptance is green on
  `infra-control` with durable evidence
  (`~/git/preestablished/guest-sdk/target/m4-acceptance-20260702T135319Z/`,
  `git_rev 604cd41`, 100 children × 60 frames) — this is your audit's
  GS-6 blocker, closed. guest-sdk beads
  `guest-sdk-m4-platform-readability-vm`, `-agent-ipc-protocol`,
  `-agent-ipc-server`, `-agent-manifest-writer`,
  `-agent-pagemap-pid-extents` are all closed; verify with `bd list` there.
- Your audit's GS-5 caveat ("acceptance requires VM tests and
  reference-workload image handoff integration") is now satisfiable: the
  acceptance boots a real workload through
  `Hello -> LoadGame -> Ready -> Start` with Ready gated on expected
  regions.

## determinism-hypervisor: Framebuffer Contract Changed (`5698d7e`)

Decision record:
`~/git/preestablished/determinism-hypervisor/docs/decisions/framebuffer-region-geometry.md`;
full trail in
`~/git/preestablished/determinism-hypervisor/.agents/requests/rom-bridge-getframebuffer-region-contract/`.

- `GetFramebuffer` and `CaptureSpec.framebuffer` derive geometry from the
  manifest entry's `layout_version`. **layout_version 1 = raw pixels,
  XRGB8888, 256×224, stride 1024, exactly 229,376 bytes** — the D7
  contract this repo already implements (`FB_BYTES` in
  `crates/refwork-emu/src/timing.rs`). No in-region descriptor; wrong
  length or unknown version is `FailedPrecondition` naming the offender;
  an all-zero region is a valid black frame.
- **Stale references to stop chasing:** your audit's DH-5 section cites
  the hypervisor test
  `descriptor_framebuffer_fixture_feeds_capture_and_get_framebuffer` and
  says "GetFramebuffer reads descriptor-backed framebuffer regions" —
  that test, the descriptor fixture, and the descriptor behavior were
  **deleted** in `5698d7e`. The replacement regression test is
  `framebuffer_layout_contract_is_enforced`.

## Deployed Runtime Facts (You Will Touch These In Step 03)

- The deployed `dh-workerd` on this host already runs the fixed contract:
  binary built from hypervisor `ff1e88c` in the clean worktree
  `~/git/preestablished/.dh-clean-ff1e88c` — **do not remove that worktree
  while it is the deployed binary**. UDS `/run/dh/grpc.sock`, 4 slots,
  snapstore per the rom-bridge-o73 runtime
  (`~/git/preestablished/determinism-hypervisor/docs/ops/rom-bridge-o73-ready-snapshot.md`).
- The hypervisor checkout's working tree has in-flight uncommitted edits
  to `crates/dh-worker/src/m9_handoff.rs` (an async refactor of the READY
  handoff generator — exactly the machinery step 03 uses). **Coordinate
  before rebuilding or running the handoff from that tree**; build from a
  clean worktree at a known rev if in doubt.
- Bridge restarts and worker restarts orphan live slots
  (`rom-operator-bridge-72o`); if your runs share the deployed worker,
  coordinate timing with the bridge side rather than assuming free slots.
- The current deployed READY snapshot's guest is the **staged M9 fixture**
  with (as deployed) a 4 KiB stub framebuffer; guest-sdk has since bumped
  the fixture to the D7 length, but the deployed snapshot predates that.
  Step 03 replaces it with the real workload.
