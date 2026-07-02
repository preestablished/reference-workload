# Step 04 — Implement `refwork-verify vm-first-room` (`refwork-d7t.11`)

Goal: the audit's "MISSING: exact first-room command/API entry point"
becomes an implemented, runnable command. The design sketch already exists
in `../guest-sdk-unblock-reference-workload/05-in-vm-first-room-gate.md` —
implement against it, updated for the current worker API surface.

## Shape (Per Phase Exit Gate 3)

Driven entirely through the worker daemon's gRPC API
(`~/git/preestablished/determinism-hypervisor/proto/hypervisor.proto`,
`determinism.hypervisor.v1.HypervisorWorker`, UDS or TCP :7400):

```text
RestoreSnapshot(ready_ref)                        -> lease, slot paused
InjectInputs(lease, first-room padlog as PadSet @frame events)
Run(lease, until: frame budget / FRAME_MARK,
    capture: CaptureSpec{ranges, framebuffer})     -> feature_bytes, fb_lz4
ReadGuestMemory(lease, region_ranges)              -> wram features, room_id
GetFramebuffer(lease)                              -> checkpoint hash proof
DestroyVm(lease)

(Note: `CaptureSpec` is not an RPC — it is the `capture` field on
`RunRequest`/`TakeSnapshotRequest`.)
```

## Infrastructure Reality Check

This repo has **no gRPC client today** — no crate depends on tonic/prost
and there is no proto copy. Budget step 04 as new infrastructure, not a
thin wrapper: proto codegen + a tonic client + UDS transport + lease and
error-code plumbing. The working pattern to follow is
`rom-operator-bridge/service` (its `dh` proto module and
`connect_real_worker`). Also note the design sketch
(`05-in-vm-first-room-gate.md`)'s own `../guest-sdk/...` relative links
are one level short from its location — the sibling checkouts live at
`~/git/preestablished/<repo>`; don't debug your checkout when a link 404s.

## Requirements

1. Input goes through the hypervisor-owned scheduled path (`InjectInputs`),
   never the harness control socket or detchannel (audit DH-2 requirement).
2. Room-transition proof via host region capture (`meta`/`wram` reads at
   FrameMark boundaries), framebuffer checkpoint proof via
   BLAKE3-of-pixels compared against operator-supplied checkpoint hashes —
   the report contains hashes, never pixels (clean-room).
3. Output: a JSON report (schema of your choosing, but include the fields
   the audit's "Required Replacement Evidence" lists: revisions, manifest
   BLAKE3, padlog BLAKE3, READY proof, room-transition proof, framebuffer
   checkpoint hashes, report BLAKE3 recorded alongside) plus logs under an
   artifact root.
4. The command must run against a **staged/synthetic game fixture** in CI
   (no game content in the repo) and accept the operator ROM/padlog via
   paths+hashes at lab-run time. Design the fixture path so the command's
   CI test is meaningful (state machine, report generation, failure modes)
   without the operator image.
5. Failure modes are first-class: wrong snapshot ref, `session/lease`
   errors, `FailedPrecondition` from the framebuffer length check, capture
   alarms — each should produce a distinct, sanitized report entry (the
   deployed worker names offenders precisely now; surface them verbatim,
   they are already clean-room-safe).

## Exit Criteria

- `refwork-verify vm-first-room --help` documents the invocation; CI runs
  it against the staged fixture green.
- A dry run against the deployed worker + new READY snapshot (step 03)
  produces a well-formed report with READY proof and framebuffer hash —
  room-transition fields pending the operator ROM run.
- `refwork-d7t.11` closed; the operator lab run (with ROM) is then a
  scheduling matter, not an engineering one.
