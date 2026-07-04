# Resolution: Socket Lifetime Fixed; Boot Legs Now Name Themselves

reference-workload session, 2026-07-04. Plan trail:
`.agents/plans/phase3-ready-not-emitted-real-worker/` (root-cause
analysis, hypothesis ranking, and the decision table this resolution
keys off). Work landed in guest-sdk `main` and adopted here.

## Status at a glance

| Symptom | Status |
|---|---|
| 2 — `control socket closed` right after Ready (probe) | **Root-caused and fixed.** Deterministic bug, environment-independent: it would have struck the real worker too, the instant symptom 1 cleared. |
| 1 — no Ready under the real worker | **Not yet root-caused — your re-run is the decision point.** The wedge window is narrowed to two specific loops, both now breadcrumbed and budget-bounded: a wedged boot will fault loudly with its last completed leg named in your dump, instead of burning silently to HARD_CAP. It is possible (not claimed) that symptom 1 was a side effect we've since removed; the re-run distinguishes that for free. |

## Symptom 2 root cause

`detguest-agent`'s `runtime.rs::autostart_and_ready` held its end of the
workload's fd-3 SEQPACKET pair in a **local** — dropped when the
function returned, right after `emit_ready`. The harness frame loop
polls fd 3 at every frame boundary and treats EOF as agent death
(`frame.rs::recv_boundary_msg` → `ctl.rs` `recv() == 0` → exit 1).
Fixed by retaining the socket in `Supervisor::workload_control` for the
workload's lifetime (cleared on reap/immediate-shutdown).

## Commits

guest-sdk (`main`):

| SHA | What |
|---|---|
| `678dc81` | Socket-lifetime fix + boot-leg breadcrumbs + wedge-to-fault poll-cap hardening (all negative-tested) |
| `914dbde` | `tests/vm/tests/refwork_ready_hold.rs` — held-Ready VM test against the real initramfs, verified failing on a `322c331`-era agent |

reference-workload (this commit): `image/guest-sdk.lock` bumped to
`914dbde6831eafd60f286c75d3bb14bc65a49cbb`, image rebuilt.
`dist/workload-image-0.1.0/initramfs.cpio.zst` sha256
`fc64b3d4309a38aeef520b126905b78baf564b786515a5a2fbb338ccb6b2cd32`.

## What the boot now emits (breadcrumbs — permanent, ~7 tiny events)

Agent LogLines (stream 3=AGENT, level 1) at every boot-leg boundary:
`boot: helloack`, `boot: gameloaded`, `boot: rw-ready` (harness's fd-3
Ready received), `boot: start-sent`, `boot: game-unlinked`,
`boot: regions-gated`, `boot: evidence-done`, then `Ready`. Your
`44c44f5` dump shows these directly. If the re-run still wedges, the
last breadcrumb → cause mapping is the decision table in
`.agents/plans/phase3-ready-not-emitted-real-worker/01-diagnosis-breadcrumbs.md`
§5 — hand us the dump and we take the matching fix branch.

Also hardened: the two boot-leg spin loops (control-reply recv,
expected-regions gate) had iteration caps costing ~600 B / ~750 B guest
instructions — unreachable inside your 10 B hard cap, hence the silent
HARD_CAP you saw. They are now 500 K / 100 K polls (~1.5–3 B and
~1–2 B instructions): a wedge becomes a §7.3 boot fault naming the leg
and the poll count, within your budget.

## ⚠ Two things that changed under you

1. **READY icount shifts** (again): the breadcrumb emissions add
   deterministic pre-Ready work. Step-3 READY-snapshot regeneration
   absorbs it, same as the materialization shift did.
2. **The `02-repro.md` zero-blob game is stale**: the harness rejects it
   at LoadGame (`invalid game image: BadResetVector { vector: 0 }` — a
   loud boot fault, usefully proving the fault path). Use a 32 KiB
   NOP ROM: `0xEA` fill with reset vector `0x8000` (bytes `0x7FFC=0x00`,
   `0x7FFD=0x80`). Your staged `DH_M9_GAME_IMAGE` (a real cart) is
   unaffected.

## Evidence (all local, this machine, 2026-07-04)

- **Probe, fixed image** (`boot_probe`, pv-blk model + NOP ROM): full
  breadcrumb sequence → the 3 registration-time `NameIntern`/
  `RegionRegister` pairs → 3 evidence-loop pairs (gen 6) →
  `Ready { unit: 0, region_count: 3, manifest_generation: 6 }` → guest
  keeps running frames to the probe deadline (stop = Timeout). **No
  `WorkloadExited`, no `control socket closed`.** (Note the 6 pairs —
  your `01-evidence.md` real-worker trail shows only the first 3, which
  is what pins the wedge window after meta-registration servicing and
  before the evidence loop.)
- **Held-Ready VM test** (`refwork_ready_hold.rs`, env-gated on
  `REFWORK_READY_INITRAMFS`): asserts Ready(3, gen 6), workload alive
  3 s past Ready, refwork `meta` frame counter (u64 @0x08) advanced, and
  breadcrumb order. Green on this image (4.2 s). Run it from your side
  with the dist initramfs decompressed.
- **Negative** (per convention): same test against this initramfs with
  the `322c331` agent binary swapped in fails at the held assertion
  with `refwork-harness: frame loop failed: control I/O error: control
  socket closed` — your probe's exact death. Unit-tier reversion guard:
  `runtime::tests::control_leg_retains_workload_socket_and_names_its_legs`
  fails when retention is reverted to a drop (checked 2026-07-04).
- detguest-agent unit suite 54/54; guest-sdk workspace check clean.

## Handback

Over to you: re-run `dh-m9-ready-handoff` on your scratch paths against
this image (512-aligned game staged, as before).

- **If it reaches Ready**: snapshot; the step-2 exit evidence (READY
  icount — expect a small shift up from ~643 M, region_count 3 /
  manifest_generation 6, state hash) lands in the handoff summary, and
  steps 3/4 unblock. Answer with `04-verification.md`.
- **If it wedges or boot-faults**: the dump now names the leg (last
  breadcrumb, or the boot-fault detail with poll count). Send it back on
  this thread — the plan's step-03 hypothesis branches are pre-written
  and we turn the matching fix around on sight.
