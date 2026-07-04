# Plan: Guest-SDK Ready Emitted And Held Under The Real Worker

Filed 2026-07-04, answering
`.agents/requests/phase3-ready-not-emitted-real-worker/` (read it first —
especially `01-evidence.md` for the two trails and `02-repro.md` for the
exact repro and green criteria). Written for a coding agent working
across this repo and the sibling `~/git/preestablished/guest-sdk`
checkout.

## The Two Symptoms (From The Request)

1. **Real worker:** guest-sdk `Ready` (stream 8) never emitted. All three
   regions register (`manifest_generation 6`, icount ~643 M), then the
   guest silently burns 9.3 B instructions to HARD_CAP.
2. **Device-less probe:** same image emits `Ready` — then the harness
   frame loop dies with `control I/O error: control socket closed` and
   the workload exits 1.

## What This Plan Already Knows (Code-Study Findings, 2026-07-04)

These findings came from reading both repos at reference-workload `main`
(`5f293af`) / guest-sdk lock rev `322c331`. Re-verify against the actual
code before acting — do not implement from this summary alone.

### Symptom 2 root cause — CONFIRMED by inspection

In `guest-sdk/crates/detguest-agent/src/runtime.rs`,
`autostart_and_ready()` creates the fd-3 socketpair via
`control::socketpair()` and binds the agent side to a **local**
`sock: ControlSocket`. Nothing stores it: when `autostart_and_ready`
returns (right after `emit_ready`), `sock` is dropped and the agent's
end of the SEQPACKET pair closes. The harness frame loop
(`reference-workload/crates/refwork-harness/src/frame.rs::recv_boundary_msg`
→ `ctl.rs::try_recv_datagram`) then gets `recv() == 0` →
`UnexpectedEof "control socket closed"` → `main.rs:51` exits 1. This is
deterministic and environment-independent — it will strike under the
real worker too, the instant symptom 1 is fixed. Fix in
`02-fix-agent-control-socket-lifetime.md`.

### Symptom 1 wedge window — NARROWED, needs one instrumented run

The real-worker trail's three `NameIntern`+`RegionRegister` pairs are
emitted at registration-service time
(`detguest-agent/src/region_ipc.rs` `Request::Register` arm — `emit` +
`emit_with_doorbell` per region). But the image's boot.toml
(`image/boot.toml`) lists three `[[expected_region]]` entries, so a
successful boot emits a **second** set of three pairs from
`runtime.rs::emit_expected_region_evidence`, then `Ready`. None of those
appear in the real-worker dump. Therefore the agent wedged **after
servicing the meta registration and before the evidence loop emitted**,
i.e. inside one of:

- the tail of `control.rs::drive_refwork_start` — the fd-3 recv loops
  for `GameLoaded` / harness-`Ready`, or the `Start` send;
- `runtime.rs::wait_for_expected_regions` (manifest gate poll);
- (less likely) `remove_file(GAME_IMG_PATH)` or
  `copy_manifest_stable` seqlock spin at the top of the evidence pass.

Both candidate loops are bounded spins (`CONTROL_RECV_POLL_LIMIT` =
200 M, `READY_REGION_POLL_LIMIT` = 50 M) whose per-iteration cost is
several syscalls (recv + region-IPC service + sched_yield ≈ thousands of
guest instructions), so their timeout budgets vastly exceed the 10 B
instruction hard cap — a wedge there is *exactly* a silent HARD_CAP with
no boot fault, which is what was observed. `01-diagnosis-breadcrumbs.md`
bisects this with one instrumented run.

## Execution Order

| Step | File | Repo | Depends on |
|---|---|---|---|
| 1 | `01-diagnosis-breadcrumbs.md` | guest-sdk (+ bridge run) | — |
| 2 | `02-fix-agent-control-socket-lifetime.md` | guest-sdk | — (can run parallel to 1) |
| 3 | `03-fix-ready-emission.md` | guest-sdk (maybe refwork) | 1 |
| 4 | `04-vm-tier-parity-test.md` | guest-sdk | 2, 3 |
| 5 | `05-adoption-and-handback.md` | reference-workload | 2, 3, 4 |

## Ground Rules

- **Cross-repo layout:** the agent lives in
  `~/git/preestablished/guest-sdk` (its own git repo — commit there
  following that repo's conventions; it has its own CLAUDE.md/AGENTS.md).
  This repo adopts fixes by bumping `image/guest-sdk.lock` `rev` and
  rebuilding (`cargo run -q --locked -p xtask -- image build` verifies
  the sibling checkout is at exactly the pinned rev).
- **Negative-test convention** (both repos): every behavioral guard gets
  a test shown to fail when the fix is reverted; note that in the test
  doc comment as existing tests do (see
  `runner.rs::hard_registration_failure_faults_before_ready`).
- **The bridge session owns the real-worker scratch environment.** You
  cannot run the M9 handoff yourself; per `02-repro.md`, hand candidate
  builds back via the request's `03-resolution.md` and they re-run
  quickly. The device-less probe (`boot_probe.rs`) you CAN run locally —
  use it as the fast inner loop.
- **Clean-room discipline** carried from prior plans: evidence records
  revisions, command shapes, hashes, artifact paths — never ROM bytes or
  framebuffer/WRAM dumps. (The synthetic 32 KiB zero-fill game is fine
  to use and mention.)

## Files In This Plan

| File | Contents |
|---|---|
| `01-diagnosis-breadcrumbs.md` | Boot-leg breadcrumb instrumentation + drop-counter check; decision table from one real-worker run |
| `02-fix-agent-control-socket-lifetime.md` | Symptom 2: keep the agent's fd-3 peer open for the workload's lifetime |
| `03-fix-ready-emission.md` | Symptom 1: ranked hypotheses, fix shapes, and wedge-to-fault hardening |
| `04-vm-tier-parity-test.md` | Green criterion 1: real harness → held Ready → past first frame boundary, worker-parity devices |
| `05-adoption-and-handback.md` | Lock bump, image rebuild, request handback, bridge verification |
