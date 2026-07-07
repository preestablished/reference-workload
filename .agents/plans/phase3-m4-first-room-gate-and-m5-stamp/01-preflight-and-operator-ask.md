# Step 01 — Preflight, Bead Hygiene, And The Consolidated Operator Ask

Small, do first. Nothing here needs a lab window.

## 1. Verify the bead graph (do not blindly add edges)

The request says the sequencing edges are missing; **that claim was
verified stale on 2026-07-07** — the chain already reads
`gp9 → .11 → .12 → .13 → .14 → .15` (and `.11 → .10 ✓`). Run:

```bash
bd dep tree refwork-d7t
bd ready
```

Expected: `refwork-gp9` is the only ready item under this chain. If an
edge is genuinely absent, add it with `bd dep add <child> <parent>`;
otherwise move on. Record the `bd dep tree` output in your working notes
so the closeout can cite it.

## 2. Confirm the build base

```bash
git -C /home/infra-admin/git/preestablished/reference-workload fetch --all --prune
git log --oneline -5 main
```

Build from `main` (request confirms all prior branches are ancestors).
If `main` has moved past `1295414`, skim the new commits — request-doc
commits don't matter; code commits mean re-checking the "state as
verified" section of `00-overview.md`.

## 3. File the consolidated operator ask — NOW, not later

The moment this plan starts, post a single message to the operator
(Matt) listing everything human-gated, so nothing trickles out
mid-session:

1. **Lab evidence fields** (required by the evidence schema for the
   M4/M5 lab records):
   - operator ROM BLAKE3
   - first-room padlog BLAKE3
   - run owner
2. **Real feature-map + expect goldens** (hard precondition for step 03):
   `refwork-verify vm-first-room` requires `--map <feature-map.yaml>` and
   `--expect <vm-expect.yaml>`. The only committed map,
   `feature-maps/demo-game.yaml`, is an explicit placeholder — its own
   comment says real values must be discovered with `ramdiff` /
   `refwork-verify map-check` and committed by the operator for their ROM
   revision. Ask the operator to produce (or pair on producing) the real
   map and the framebuffer-checkpoint expect goldens. Running step 03
   against the demo placeholder would produce a bogus room_id decode —
   never substitute it silently.
3. **Cutover window**: a same-day window with the bridge team to write
   `BRIDGE_REAL_SNAPSHOT_REF` after step 02 produces the new snapshot
   ref (they execute the restart procedure; we only hand over the ref).
4. **M2 build-vs-vendor record** (step 06): confirm whether a
   build-vs-vendor decision record exists anywhere, or grant an explicit
   waiver to be recorded in `m2-floor-evidence.md`.
5. **M2 aarch64 cross-arch double-run** (step 06): run it, or explicitly
   defer with a recorded reason.

(The CI runner-label question the request raises is already settled:
`vm-gates.yaml` carries `[self-hosted, intel, kvm]` with an inline
comment "confirmed by the operator 2026-07-02", and a live-worker smoke
leg gated by `REFWORK_VM_TESTS=1` already exists. Step 05 reuses that;
no ask needed unless the operator wants the new legs lab-manual.)

Steps 02 and 04–06 can proceed while waiting on answers; only step 03's
cutover and the final stamp fields hard-block on them.

## 4. Fix the known build break, then sanity checks

- **The workspace does NOT currently build with the `mock` feature**
  (verified 2026-07-07): `crates/refwork-dh-client/src/mock.rs:529`
  constructs `proto::GetWorkerInfoResponse` without the `build_profile`
  field that the sibling `dh-proto`
  (`../determinism-hypervisor/crates/dh-proto`) has since added. Fix the
  mock (and audit for any other proto drift), commit, and only then trust
  test baselines — don't assume the ~452-test count from `2ea42ad` holds.
- Then: `cargo test --workspace` green, and the staged-fixture tests pass
  (6 in `vm_first_room.rs`, 4 in `vm_suite.rs`).
- Local-probe gotcha carried from the READY-fix work: a zero-filled game
  image no longer boots (`BadResetVector`). Local probes need a NOP ROM —
  0xEA fill, reset vector 0x8000 (`rom[0x7ffc]=0x00`,
  `rom[0x7ffd]=0x80`). The real lab run uses the operator's ROM instead.

## Exit Criteria

- Dep tree verified (edges present or added), output recorded.
- Consolidated operator ask filed as one message; answers tracked.
- Workspace + staged-fixture tests green on the chosen build rev.
