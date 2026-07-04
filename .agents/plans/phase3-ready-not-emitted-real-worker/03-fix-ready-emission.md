# Step 3: Make Ready Fire Under The Real Worker (Symptom 1)

Depends on step 1's decision table. This file pre-ranks the hypotheses
so the fix starts the moment the instrumented dump lands. Repo:
**guest-sdk** for all agent-side fixes; a harness-side fix (H1 variant)
would land in this repo's `crates/refwork-harness`.

Environmental deltas between probe (works) and real worker (wedges),
from the request's `01-evidence.md`: real pv-pad vs RAZ/latch stub,
real pv-blk vs model, epoch-structured run control, and — noted during
code study — the probe host drains ring A continuously while the real
worker buffers events until stop (consumer index frozen mid-run).

## H1 — fd-3 leg stall in `drive_refwork_start` (agent spins in recv)

**Signal:** last breadcrumb is pre-`gameloaded` or pre-`rw-ready`.

The agent's `control.rs::ControlSocket::recv` spins
`MSG_DONTWAIT recv → idle() (region-IPC service) → sched_yield()` up to
200 M times. If the harness never sends `GameLoaded`/`Ready` on fd 3,
the agent sits here to the hard cap.

Split by *why* the harness went quiet — it had just finished the meta
registration (its SDK `register_region` blocks on the agent.sock reply):

- **Reply never reached the harness:** inspect
  `region_ipc.rs::handle`'s reply write path for a failure mode that
  still emits the ring events first (events at line ~288 are emitted
  BEFORE the `Reply` is returned/sent — check the caller's send and its
  error handling). A dropped/failed reply send leaves the harness
  blocked in the SDK recv forever while the agent polls fd 3. Fix:
  make a failed reply send a boot fault, and check for env-dependent
  send failures.
- **Harness wedged between SDK return and fd-3 send:** the code there
  is trivial (`mem::forget`, `send_game_loaded`); if implicated, look
  at the SDK client side (`detguest-sdk`'s register/reply recv loop)
  for a spin that differs under the real device set.
- **Scheduling livelock:** agent (PID 1) spins with `sched_yield` and
  the harness never gets scheduled under the real worker's run control
  (guest timer tick behavior may differ from the probe). Diagnose via a
  breadcrumb counter (e.g. boot-fault detail includes the poll count)
  and, if confirmed, replace the sched_yield spin with a blocking wait:
  `poll(2)` on BOTH the fd-3 socket and the region-IPC fds — the idle
  callback exists precisely because the agent must service two fds; a
  poll-based wait does that without a spin and removes the livelock
  class entirely. This is the structurally right fix even if the
  trigger is elsewhere.

## H2 — `wait_for_expected_regions` gate never satisfied

**Signal:** last breadcrumb is `boot: start-sent` / `boot: game-unlinked`.

The gate re-reads the manifest each iteration
(`expected_regions_ready`) and matches name + `layout_version` + live.
Regions ARE live (gen 6), and the probe passes the same gate with the
same image — so if the dump points here, suspect
`copy_manifest_stable` behaving differently under the real worker
(seqlock generation parity, torn read retry loop) rather than a config
mismatch. Instrument the loop's `last_err` into the boot-fault path /
a breadcrumb and fix what it names.

Note: if the agent wedges HERE, the harness has already received
`Start` and is free-running the emulator with `NoopPlatform` — which
also plausibly explains where 9.3 B instructions went. The
frame-counter in the `meta` region (offset 0x08, see
`refwork-harness/src/meta.rs`) is host-readable evidence: ask the
bridge to read it from the stopped VM's `meta` region — nonzero frames
⇒ Start was sent ⇒ H2/H3/H4 territory even without breadcrumbs.

## H3 — evidence pass stall or "manifest changed" abort

**Signal:** last breadcrumb `boot: regions-gated`.

`emit_expected_region_evidence` re-reads the manifest and errors if
`fresh != snapshot` — that error path goes to `boot_fault` (LogLine +
power-off), which the dump would show, so a silent wedge here means the
`copy_manifest_stable` read spins. Same fix family as H2.

## H4 — Ready emitted but never lands / never visible

**Signal:** breadcrumb `boot: evidence-done` present, no Ready; or
ring-A drop counters nonzero.

`channel.rs::emit`: critical events spin `doorbell → retry` on a full
ring; droppable events are silently dropped with counters bumped. Under
the real worker the consumer index may be frozen (events buffered until
stop), so any pre-Ready burst that fills ring A turns the first
critical emit into an infinite doorbell spin — and the breadcrumbs
themselves add bytes, so re-check counters after instrumenting. Fixes
if confirmed: confirm `EventKind::Ready.is_critical()`, confirm the
worker services doorbell exits by draining ring A mid-run (that half
lives in determinism-hypervisor — file a request to that repo per the
series convention rather than fixing cross-repo yourself), and shrink
pre-Ready event volume if it's self-inflicted.

## Wedge-to-fault hardening (do regardless of which H wins)

The observed failure was a *silent* HARD_CAP because both spin caps
(`CONTROL_RECV_POLL_LIMIT` 200 M, `READY_REGION_POLL_LIMIT` 50 M) cost
more instructions than the worker's cap. Retune so a wedged boot
faults loudly inside a real worker's budget: size the caps against
instructions/syscalls per iteration (target: exhaust within ~1–2 B
guest instructions), or switch the waits to `poll(2)` with a real
timeout (preferred, see H1). Boot-fault details must name the leg and
the iteration/elapsed count. Negative-test: a scripted never-replying
workload must produce the named boot fault (extend
`unit_control_faults_before_ready_when_workload_does_not_reply`).

## Exit criteria

- Root cause named, fixed in the owning repo, negative-tested.
- Probe still green end-to-end (with step 02: Ready + frames running).
- Bridge re-run of the real-worker handoff reaches `Ready` (their
  confirmation, via the request thread) — final proof is step 05.
- Wedge-to-fault hardening merged with its test.
