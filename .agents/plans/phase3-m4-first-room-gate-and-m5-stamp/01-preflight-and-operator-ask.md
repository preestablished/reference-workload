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
2. **Cutover window**: a same-day window with the bridge team to write
   `BRIDGE_REAL_SNAPSHOT_REF` after step 02 produces the new snapshot
   ref (they execute the restart procedure; we only hand over the ref).
3. **CI runner label decision** (needed by step 05, ask early): guest-sdk
   uses `[self-hosted, intel, kvm]`, determinism-hypervisor uses
   `[self-hosted, kvm-intel]` — which label should this repo's
   `vm-gates.yaml` real-worker legs use on the shared `infra-control`
   runner? (Or: recorded decision that the lab legs stay manual.)
4. **M2 build-vs-vendor record** (step 06): confirm whether a
   build-vs-vendor decision record exists anywhere, or grant an explicit
   waiver to be recorded in `m2-floor-evidence.md`.
5. **M2 aarch64 cross-arch double-run** (step 06): run it, or explicitly
   defer with a recorded reason.

Steps 02 and 04–06 can proceed while waiting on answers; only step 03's
cutover and the final stamp fields hard-block on them.

## 4. Sanity checks before the lab session

- Workspace tests green at the build rev: `cargo test --workspace`
  (expect ~452 passing as of `2ea42ad`).
- `refwork-verify vm-first-room` / `vm-suite` staged-fixture tests pass
  (6 in `vm_first_room.rs`, 4 in `vm_suite.rs`).
- Local-probe gotcha carried from the READY-fix work: a zero-filled game
  image no longer boots (`BadResetVector`). Local probes need a NOP ROM —
  0xEA fill, reset vector 0x8000 (`rom[0x7ffc]=0x00`,
  `rom[0x7ffd]=0x80`). The real lab run uses the operator's ROM instead.

## Exit Criteria

- Dep tree verified (edges present or added), output recorded.
- Consolidated operator ask filed as one message; answers tracked.
- Workspace + staged-fixture tests green on the chosen build rev.
