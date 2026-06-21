# Guest SDK Unblock - Reference Workload Plan

**Goal:** implement the `reference-workload` portion of
`/home/infra-admin/git/preestablished/.agents/plans/guest-sdk-unblock/` in this
repo. This plan is a coding-agent handoff for the reference-workload work that
unblocks guest-sdk M3-M5:

- `guest-sdk-ext-refwork-m3-mock-agent`
- `guest-sdk-ext-refwork-m4-image-handoff`
- `guest-sdk-ext-refwork-m5-full-suite`

The plan follows the owner docs in
`/home/infra-admin/.agents/projects/determinism/docs/reference-workload/` and
the Phase 3 doc. If a local implementation choice conflicts with those docs,
stop and reconcile the docs first; do not silently invent a new protocol.

## Source Docs

- `/home/infra-admin/git/preestablished/.agents/plans/guest-sdk-unblock/README.md`
- `/home/infra-admin/git/preestablished/.agents/plans/guest-sdk-unblock/reference-workload/plan.md`
- `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/IMPLEMENTATION-PLAN.md`
- `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md`
- `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/ARCHITECTURE.md`
- `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/INTEGRATION.md`
- `/home/infra-admin/.agents/projects/determinism/phases/phase-3-workload-in-the-box.md`

## Current State Verified

Verified on 2026-06-21, branch `main`, `HEAD` `9afaa0a`.

| Surface | State |
|---|---|
| M0/M2 host tooling | Present: `refwork-featuremap`, `refwork-protocol`, `ramdiff`, `refwork-script`, `refwork-hash`, `refwork-verify`, host-side synthetic CI gates |
| `refwork-protocol` | `CtlMsg`, `FaultCode`, postcard encode/decode, golden wire tests, oversize checks already exist |
| `refwork-harness` | Stub only: `hello_ack()` helper, no binary, no fd-3 loop, no region allocation, no frame loop |
| `refwork-verify` | Host-side `play`, `map-check`, `double-run` exist; no mock-agent fixture, no in-VM full-stack suite |
| `xtask` | Has `build-rom`, `deny`, corpus runners, `hash-chain`; no `audit-syms`, no `image` pipeline |
| CI | Per-PR synthetic cross-arch hash compare and nightly deep synthetic determinism gates exist; no M3 harness/mock gate, no image double-build gate, no lab full-stack suite lane |
| `feature-maps/demo-game.yaml` | Still treated as a blocker for packages 05-06 until real offsets are evidenced or explicitly waived; do not assume placeholder offsets are usable for in-VM first-room evidence |
| Control-plane proto floor | Workspace has a path dependency on `../control-plane/crates/determinism-proto`; implementation runs require the sibling checkout or an explicitly recorded replacement source |

## Work Packages

| File | Package | Upstream work |
|---|---|---|
| `01-confirm-m2-floor.md` | Confirm or waive the M2 host-side floor before consuming it | RW-0 |
| `02-harness-state-machine.md` | Build the harness binary, fd-3 control loop, region buffers, meta page, frame loop | RW-1 |
| `03-mock-agent-fixtures.md` | Add mock-agent integration and protocol-abuse fixtures | RW-1 |
| `04-image-handoff-assets.md` | Build image handoff assets: `workload-image.yaml`, `boot.toml`, region list, deterministic image pipeline | RW-2 |
| `05-in-vm-first-room-gate.md` | Validate the real agent and hypervisor path with the first-room script | RW-3 |
| `06-full-determinism-suite.md` | Implement the full double-run and snapshot/restore suite | RW-4 |
| `07-ci-evidence-closeout.md` | Wire gates, produce handoff evidence, and close guest-sdk blockers | closeout |

## Dependency Graph

```text
control-plane proto checkout/pin
        |
        v
01 M2 floor confirmation
        |
        v
02 harness state machine -----> 03 mock-agent fixtures
        |                         |
        +------------+------------+
                     |
                     v
04 image handoff assets
        |
        v
05 in-VM first-room gate  <---- external: guest-sdk GS-5/GS-6, hypervisor DH-2/DH-5
        |
        v
06 full determinism suite <---- external: guest-sdk GS-8, hypervisor DH-6, snapshot-store SS-1
        |
        v
07 CI/evidence closeout
```

Packages 02 and 03 are the first implementation priority. Package 04 can start
once the harness binary shape is known, but it should not claim real READY
validation until package 05. Packages 05 and 06 are integration gates and must
wait for the guest-sdk, hypervisor, and snapshot-store surfaces named in their
files.

## Standing Constraints

- Clean-room boundary remains in force: no game names, no game-derived content,
  no ROMs, no framebuffer goldens, and no lab dumps in the repo.
- CI uses only the synthetic ROM. Operator-game evidence lives on lab runners;
  repo-side evidence records hashes, paths, dates, and owners only.
- `reference-workload` owns feature maps, scoring YAML, harness-agent control
  protocol, WorkloadImage manifest, region names, and the full determinism suite.
- `guest-sdk` owns detchannel rings, `poll_input`, `frame_mark`, region manifest,
  READY point, `boot.toml` format, and the real agent behavior.
- `determinism-hypervisor` owns input logs, `PAD_SET`, capture, region reads,
  replay verification, and frame-budget stopping.
- `control-plane` owns the shared proto source. This repo must record the
  sibling checkout or pinned replacement used for `determinism-proto`; do not
  vendor an incompatible local copy.
- Do not block package 04 on the full control-plane artifact registry. The owner
  docs allow direct `dist/` plus manifest handoff until the registry exists.
- Preserve ARCHITECTURE.md D1-D9 in every harness/core change: single-threaded
  production loop, no wall-clock reads, no RNG, no floats in core/harness, plain
  memory state, one pad read per frame, stable published regions, no per-frame
  allocation, and fault loudly.

## Exit Criteria

This plan is complete when:

- M3 mock-agent fixture and abuse tests pass in CI and guest-sdk can cite the
  fixture path.
- M4 image handoff publishes a deterministic image manifest, `boot.toml`,
  expected-region list, region vocabulary, pad layout, and image double-build
  evidence.
- M4 real in-VM first-room run proves the agent reaches READY with regions live
  and the first-room transition is observable through host region capture.
- M5 suite passes 20 consecutive Intel lab runs with zero flakes and records
  double-run, snapshot/restore, in-guest hash, host-read hash, and `meta.frame`
  cross-check evidence.
- The three guest-sdk external blockers above have durable artifact paths or CI
  jobs that guest-sdk can cite.
