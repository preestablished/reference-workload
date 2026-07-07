# Progress Report + Consolidated Operator Ask (2026-07-07)

Filed by the implementing agent after executing the agent-doable half of
`.agents/plans/phase3-m4-first-room-gate-and-m5-stamp/`.

## Done Today (main at `7b0c7b2` + follow-ups)

| Item | State |
|---|---|
| Preflight build break (`refwork-dh-client` mock vs `dh-proto` `build_profile`) | fixed `34f034d`; workspace green |
| Bead graph | verified — chain `gp9 → .11 → .12 → .13 → .14 → .15` present, `gp9` sole ready item |
| `image/guest-sdk.lock` | bumped to `acb1d3e8` (docs/tests-only delta) `7b0c7b2` |
| Package-04 image rebuild at current main | done; first image carrying the emulator accuracy chain + READY fixes |
| Clean-root double-build byte-identity | **OK** — initramfs `67f1ed56…` (490,922 B), manifest `af140404…`; `dist/` synced to clean-root artifacts |
| M9 staging | new initramfs at `~/.cache/dh-m9/reference-workload/initramfs.m4-regen-7b0c7b2.cpio`; cached bzImage already matches the kernel pin |
| CI real-worker legs (`vm-first-room`/`vm-suite`) | added to `vm-gates.yaml` (`3ddf34f`), dispatch-gated on lab inputs, single-iteration profile |
| M2 paper trail (`refwork-d7t.1`) | `m2-floor-evidence.md` extended: synthetic floor re-verified at current rev (nightly `28857976642` cross-arch 100k equal), build-vs-vendor resolution proposed, aarch64 decision requested |
| Evidence | `m4-in-vm-first-room-evidence.md` 2026-07-07 section |

## The Consolidated Ask (everything human-gated, in one place)

1. **Lab session window** (closes `refwork-gp9` + `9xo`, then gates 3 and 1):
   run `dh-m9-ready-handoff` per
   `../determinism-hypervisor/docs/ops/rom-bridge-o73-ready-snapshot.md`
   with the rebuilt artifacts. Needs from you / the bridge team:
   - the real `DH_M9_GAME_IMAGE` (the cached `game.img` is the 32 KiB
     placeholder — a snapshot of it cannot satisfy the first-room gate);
   - private bridge values (private root, workload image ref, capture
     spec ref);
   - the coordinated `BRIDGE_REAL_SNAPSHOT_REF` cutover afterward
     (bridge team executes; `72o` lease caveat).
2. **Lab evidence fields**: operator ROM BLAKE3, first-room padlog
   BLAKE3, run owner.
3. **Real feature-map + expect goldens**: `feature-maps/demo-game.yaml`
   is an explicit placeholder; `vm-first-room` needs an operator-committed
   real map (`ramdiff` / `refwork-verify map-check`) and
   `vm-expect.yaml` checkpoint goldens for the ROM revision.
4. **M2 build-vs-vendor** (`refwork-d7t.1`): confirm the kernel/agent
   artifact-split decision doc as the build-vs-vendor record (one-line
   bead comment suffices), or grant an explicit waiver.
5. **M2 aarch64 operator-game double-run**: run in the lab session, or
   defer with a recorded reason (a tracking bead will be filed on
   deferral).

Items 1–3 are one lab session; the M5 20× campaign (steps 03–04, beads
`.11–.14`) follows immediately on the same snapshot. Items 4–5 are
one-line decisions.

## For The Bridge Team

The regenerated image is ready for the handoff: clean-root
`dist/workload-image-0.1.0/` at `7b0c7b2` (manifest BLAKE3 `af140404…`).
Ping us with your window; same-day works. Please confirm at cutover time
which ref `BRIDGE_REAL_SNAPSHOT_REF` currently serves, per your own
`03-verification-offer.md`.

---

## Update 2026-07-07 (later): M5 Green Stamp Landed — Exit Gate 1 Satisfied

The operator supplied the game image (`~/ROMs/SNES`, BLAKE3
`96cdaa23…`) mid-session, which unblocked the real chain:

| Item | State |
|---|---|
| READY snapshot regenerated from the real ROM (`dh-m9-ready-handoff`, restore-verified) | **DONE** — private handoff env ready for the bridge cutover |
| vm-suite AgendaNotEmpty contract bug (mid-run TakeSnapshot vs queued inputs) | found in NOP rehearsal, **fixed** `320f425`, mock now enforces it in staged CI |
| M5 suite single iteration (double-run + restore-continuity) | PASS |
| `--nondet-test` negative | FAILs as required, divergence localized at the perturbed frame |
| **20× zero-flake campaign, Intel lab runner (infra-control), real ROM** | **PASS — 20/20, single trajectory hash** (report `a06051df…`) |
| Green stamp | `determinism.last_green` written (unstamped sidecar deleted); `xtask image validate` + `register --require-green-stamp` **accept** |
| Beads | `refwork-gp9`, `refwork-d7t.12/.13/.14/.15` closed with evidence; guest-sdk `ext-refwork-m5-full-suite` handed off |

Full record: `.agents/plans/guest-sdk-unblock-reference-workload/m5-suite-evidence.md`;
artifact root `target/m5-acceptance-20260707/`.

## Remaining Ask (shrunk)

1. **Bridge cutover window** — the regenerated real-ROM snapshot's
   private handoff env is ready; ping us and execute your restart
   procedure, then run your browser-side first-frame verification
   (this closes your `9xo` and the human-visible half of gate 3).
2. **First-room inputs** (`refwork-d7t.11`, gate 3): operator first-room
   padlog + real `feature-map.yaml`/`vm-expect.yaml` goldens
   (`ramdiff`/`map-check` against ROM `96cdaa23…`). The verifier and
   snapshot are waiting.
3. **M2 one-liners** (`refwork-d7t.1`): confirm the artifact-split
   decision doc as the build-vs-vendor record (or waive), and decide the
   aarch64 operator-game run (run vs defer-with-reason).
