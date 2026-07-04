# Verification (rom-operator-bridge side, 2026-07-04)

Re-ran the real-worker `dh-m9-ready-handoff` against the fixed image
(guest-sdk `914dbde`, reference-workload `aa69558`, lock at `914dbde`),
and independently re-ran the probe on the **identical** image.

## Verdict: Symptom 2 fixed (confirmed). Symptom 1 persists → plan H1.

### Symptom 2 — CONFIRMED FIXED (independent probe)

Probe on the boot-6 image reaches
`Ready { unit: 0, region_count: 3, manifest_generation: 6 }` and stops
with **Timeout at the 30 s deadline — not `WorkloadExited`, not "frame
loop failed: control socket closed"**. The workload is alive past Ready.
The fd-3 retention fix (`Supervisor::workload_control`) does exactly what
it claims.

### Symptom 1 — PERSISTS. The real worker still hard-caps.

Instrumented handoff dump (breadcrumbs are stream=11; full trail at the
scratch root's `boot6-real-worker-trail.txt`):

```text
stop reason 4 (HARD_CAP); icount=10000000000 frames=0
  stream=1  icount=640981471  Hello
  stream=9  icount=642810314  WorkloadStarted
  stream=11 icount=642810314  "boot: helloack"          <-- last REAL breadcrumb
  stream=2/7 icount=642810314..643049118  wram, framebuffer, meta
             (SIX NameIntern/RegionRegister pairs = a COMPLETE good-boot
              region phase; symptom-2-era trails showed only 3)
  stream=11 icount=10000000000 "boot: gameloaded"        <-- at the CAP
  stream=11 icount=10000000000 "boot: rw-ready"          <-- at the CAP
```

Region registration now **completes** (6 pairs, gen 6), then the guest
runs 9.3 B instructions to the cap. The `gameloaded`/`rw-ready`
breadcrumbs carry `icount == 10 000 000 000` exactly — i.e. they are
force-stop-boundary artifacts, not genuine mid-run emissions. **The
agent never actually receives `GameLoaded` during the run.** Last real
leg completed: `helloack`. No SDK `Ready` (stream 8): 0 occurrences.

## This is H1 (fd-3 leg stall in the agent's refwork-start recv)

Per `03`'s decision table, "last breadcrumb pre-`gameloaded`" ⇒ H1. The
harness completed `publish_regions` (all 6 pairs present) and is now
blocked — either in `send_game_loaded`/its SDK reply-recv, or waiting in
`expect_start` — while the agent sits in `ControlSocket::recv` waiting
for a `GameLoaded` it never sees.

### Two sharp findings that narrow the H1 sub-branch

1. **The wedge-to-fault hardening did NOT fire.** The boot ran the full
   10 B; it did **not** boot-fault at the retuned ~1.5–3 B cap. So the
   stall is **outside** the two capped poll loops
   (`CONTROL_RECV_POLL_LIMIT` / `READY_REGION_POLL_LIMIT`) — this reads
   as a genuine **block/deadlock**, not a bounded spin. That argues
   against "agent spins in a counted poll loop" and toward either the
   region-IPC reply path (uncapped) or a mutual blocking wait
   (agent blocking-recv on fd-3 ⇄ harness blocking-recv on agent.sock).
   Whatever it is, the hardening needs to cover it — a real wedge just
   slipped through as a silent HARD_CAP again.

2. **Probe-vs-worker on the identical image is the discriminator.** The
   only material deltas are the real device set (real pv-pad vs RAZ) and
   — most suspect — the **timerless run-control cmdline**. The worker
   forces `notsc tsc=unstable clocksource=jiffies noapictimer`
   (dh-vmm `BZIMAGE_FORCED_CMDLINE`); the probe uses its own timer-ful
   `console=ttyS0,115200 … hugepages=4`. A cross-process fd-3/agent.sock
   handshake that relies on the scheduler making progress can deadlock
   under a timerless single-vCPU guest while working fine under the
   probe's timer. This is `03`'s H1 "scheduling livelock" sub-branch,
   and it fits finding 1 (a block, not a spin).

## Recommendation

Adopt H1's stated structural fix regardless of exact trigger: **replace
the `MSG_DONTWAIT recv → idle() → sched_yield()` spin with a `poll(2)`
blocking wait on BOTH the fd-3 socket and the region-IPC fds**, so the
agent makes progress without depending on preemptive scheduling — this
removes the livelock/deadlock class the timerless worker exposes. Pair
it with a wedge-to-fault cap that actually bounds *this* path (finding
1), and confirm the harness side (`detguest-sdk` register reply-recv /
`refwork-harness` `send_game_loaded`) can't block symmetrically.

## Verification loop note (important)

**The probe cannot reproduce symptom 1** — it reaches Ready on the exact
image that wedges under the real worker. So the guest-sdk VM-tier
`refwork_ready_hold` test will pass on a broken fix. Every candidate fix
must be verified by a **real-worker `dh-m9-ready-handoff` run**, which is
the bridge session's to run. Hand back with the candidate (lock bump +
commit) and we turn the real-worker run around quickly; the instrumented
dump (determinism-hypervisor `44c44f5`) will name the next leg if it
moves.

## State

Unpushed local `main` commits at time of writing: guest-sdk `678dc81`,
`914dbde`; reference-workload `ee8a083`, `aa69558`. Bridge-side pushed:
determinism-hypervisor `44c44f5` (event-dump instrumentation),
reference-workload `cdcb372` (game_source adoption) / `5f293af` (this
request). The image I tested was rebuilt locally from these; my in-tree
initramfs hash differs from `03`'s `fc64b3d4` by build-metadata only
(double-build is internally reproducible; rev-check guarantees the
`914dbde` agent).
