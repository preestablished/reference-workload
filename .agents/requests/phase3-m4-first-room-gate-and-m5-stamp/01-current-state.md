# Current State (Evidence-Based)

Repo `main` at `2ea42ad` (2026-07-06), assessed 2026-07-07.

## Upstream: All Green

- **guest-sdk Ms4** (region publication) delivered and independently
  verified; **boot-scheduling deadlock** fixed and verified on the real
  worker — first real emulator+game READY under the deterministic worker,
  2026-07-05 (`../guest-sdk/.agents/requests/phase3-boot-scheduling-deadlock/04-verification.md`).
- **hypervisor** M9/capture/D7 contract accepted; the no-frame trail ended
  in your repo (`refwork-4qj`: harness ran `NoopPlatform`, publishing no
  `frame_mark`) and you fixed it at `40eaf4f`. The hypervisor's remaining
  test-cap retune is filed against that repo
  (`../determinism-hypervisor/.agents/requests/phase3-frame-cap-retune-and-run-wallclock-backstop/`).
- **snapshot-store M7 GC** — done and verified 2026-07-03 (Phase 3 exit
  gate 4 is green; some of your older notes still call it unowned — it is
  not).
- **Emulator accuracy**: the SNES black-screen chain (APU boot handshake,
  SPC700 IPL, PPU color math) landed 2026-07-06 (`84933d9`, `8eff8d9`,
  `2ea42ad`); a real ROM renders non-black
  (`.agents/plans/snes-rom-black-screen-compat/03-implementation-results.md`).
  One P0 remains open on the bridge side against this trail
  (`rom-operator-bridge-9xo`) — its remaining chain is exactly this
  request's steps 1–2 plus the operator cutover, so it closes with them.

## In This Repo: Built But Not Proven

- **M3** done (harness + protocol vs mock agent; beads `refwork-d7t.2–.6`
  closed).
- **M4 ~80%**: image pipeline reproducible (`dist/workload-image-0.1.0/`,
  clean-root double-build byte-identical; kernel/agent artifact split per
  `.agents/decisions/2026-07-02-kernel-agent-artifact-split.md`); harness
  links `detguest-sdk::register_region` for the three regions before Ready.
  **Missing:** READY snapshot regenerated from the current image; the
  in-VM first-room run. Beads open: `refwork-gp9` (the only `bd ready`
  item), `refwork-d7t.11`.
- **M5 tooling built, unstamped**: `refwork-verify vm-first-room` and
  `vm-suite` (double-run + restore-continuity + `--nondet-test`) exist and
  pass their staged-fixture tests over a UDS gRPC mock worker (6 in
  `vm_first_room.rs`, 4 in `vm_suite.rs`); workspace 419→452 tests green. **Missing:** the 20× zero-flake lab run against a
  real image + snapshot. Beads open: `refwork-d7t.12/.13/.14`, closeout
  `.15`. `dist/workload-image-0.1.0/determinism.unstamped.yaml` still
  points at "package 06" for the stamp.
- **M2 paper trail gap** (secondary): `gaps.md` (2026-06-15, project docs)
  declared M2 not achieved — no host-side first-room evidence, no
  build-vs-vendor decision record. The tracked home for this is bead
  **`refwork-d7t.1`** (P1, blocked — and it blocks the `refwork-d7t`
  epic), with partial evidence already at
  `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md`.
  The subsequent accuracy fixes likely satisfy the substance (the known
  open question is M2's cross-arch aarch64 double-run), but the gate is
  currently closed only by implication.

## Operator-Gated Items (No Agent Can Supply These)

1. **Lab fields**: operator ROM BLAKE3, first-room padlog BLAKE3, run
   owner — required by the evidence schema for the M4/M5 lab records.
2. **Cutover**: writing `BRIDGE_REAL_SNAPSHOT_REF` restarts the live
   bridge/worker and invalidates slot leases (`rom-operator-bridge-72o`);
   unconditionally operator-coordinated.

(One item your plan flagged is now moot: the branch-reconciliation
question. As of this filing, `phase3/m4-first-room-unblock`,
`phase3/kernel-agent-artifact-split`, and `codex/phase4-corpus-guide` are
all ancestors of `main` at `2ea42ad` — the build base is `main`, no
decision needed; branch deletion is optional housekeeping.)

## What Waits Behind This

- Phase 3 exit gates 1 (M5 20× zero-flake) and 3 (first room in-VM through
  worker gRPC) — the last open gates *owned by this repo*. Gate 2's
  guest-sdk half (the Ms5 `determinism_replay` CI gate) remains open on
  their side, behind a hypervisor handoff filed separately.
- Phase 4 entry: "real RAM/framebuffer captures from the in-VM emulator
  (golden-test corpus)" (`phase-4-scoring-and-inputs.md`). Both project
  requests against you (`pad-alphabet-and-phase4-context-fixtures`,
  `phase-4-scorer-golden-artifacts`) are stuck at the same real-capture
  floor; state-scorer M1 and input-synthesizer M2 consume what you produce.
- The bridge's first visible real frame (browser) — pipeline verified
  end-to-end; the deployed snapshot `22dc5b40` predates the emulator fixes
  and the deployed `game.img` is ~96% zeros, so the frame is blank until
  your regenerated snapshot + a real ROM cut over.
