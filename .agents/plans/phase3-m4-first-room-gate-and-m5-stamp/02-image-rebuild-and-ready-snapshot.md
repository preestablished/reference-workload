# Step 02 — Rebuild Package-04 Image + Regenerate READY Snapshot (`refwork-gp9`)

Full procedure:
`.agents/plans/phase3-m4-first-room-unblock/03-image-rebuild-and-ready-snapshot.md`.
This file records only the deltas since that was written and the exact
exit bar.

## What changed since the prior plan's step 03

- The scope findings in `refwork-gp9`'s comments are all RESOLVED: the
  kernel/agent artifact split landed
  (`.agents/decisions/2026-07-02-kernel-agent-artifact-split.md` — kernel
  = hash-pinned guest-sdk 6.12.93 artifact, agent built from sibling at a
  pinned rev), and the harness registers `wram`/`framebuffer`/`meta` via
  real `detguest_sdk::register_region` before Ready.
- The boot-scheduling deadlock and the agent control-socket lifetime bug
  are fixed and verified on the real worker (first real emulator+game
  READY 2026-07-05). Do not re-diagnose a failure to reach READY from
  scratch — the boot now emits `boot: *` breadcrumb LogLines; a wedge
  names its last leg in the dump (decision table:
  `.agents/plans/phase3-ready-not-emitted-real-worker/01-diagnosis-breadcrumbs.md` §5).
- The emulator accuracy chain (`84933d9`, `8eff8d9`, `2ea42ad`) is in
  `main`; the image must be rebuilt to pick it up — the deployed snapshot
  `22dc5b40` predates these fixes and is NOT a baseline.
- READY icount will shift slightly vs older recorded values (~643M) —
  pre-Ready breadcrumb emissions were added. Expected; record the new
  value, don't chase the delta.

## Work

1. **Rebuild** via the `xtask image` flow from current `main`. Verify the
   agent pin: check whether the pinned sibling guest-sdk rev in the image
   locks (`c03e90b` at last build) needs bumping to include the deadlock
   fix (`914dbde` or later) — if the fix postdates the pin, bump the pin
   and note it in the evidence. Kernel artifact stays hash-pinned 6.12.93
   unless guest-sdk republished.
2. **Prove double-build reproducibility at the new rev**: clean-root
   double build, byte-identical artifact hashes (the fixed-dir
   build+rename workaround for the out-of-workspace dep is already in the
   flow). Record all artifact BLAKE3s.
3. **Regenerate the READY snapshot** under a **locally-launched real
   worker** via the hypervisor M9 handoff (`dh-m9-ready-handoff` flow):
   - Build `dh-workerd` from a FRESH pinned hypervisor scratch worktree —
     never `.dh-clean-ff1e88c` (deployed, read-only). Pattern: create
     `~/git/preestablished/.dh-clean-<rev>` at the deployed rev
     (`30d0cb9` unless the bridge team says otherwise).
   - Gotcha: the hypervisor repo's committed `Cargo.lock` is stale vs its
     `Cargo.toml` — regenerate the lock in the scratch worktree, then
     build `--offline`.
   - Run it with `serve --uds <scratch>.sock --image-cache … --snapstore-uds …`
     pointed at scratch paths. Never `/run/dh/grpc.sock`.
4. **Record**: new snapshot ref, image manifest BLAKE3, initramfs hash,
   git revs (this repo + guest-sdk pin + hypervisor rev), READY icount,
   host, date — appended to
   `.agents/plans/guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`
   as a new dated section (existing evidence discipline).
5. **Hand the snapshot ref to the bridge team** for the coordinated
   cutover (step 01's ask item 2). Do not write the env channel yourself.

## Exit Criteria (closes `refwork-gp9`)

- Image rebuilt from current `main`; double-build byte-identity proven at
  the new rev; hashes recorded.
- New READY snapshot ref exists and is recorded in the evidence note.
- Cutover request handed to the bridge with the ref; bead closed with
  `bd close refwork-gp9 -r "…"` citing the evidence section.
- Bridge P0 `rom-operator-bridge-9xo` note: this step plus step 03 plus
  their cutover closes it — mention the ref in the handoff so they can.
