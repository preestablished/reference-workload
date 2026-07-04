# Step 2: Keep The Agent's fd-3 Peer Open For The Workload's Lifetime

Symptom 2, root-caused. Repo: **guest-sdk**. Independent of step 1 —
can be done in parallel.

## The bug

`crates/detguest-agent/src/runtime.rs::autostart_and_ready`:

```rust
let (sock, child_fd) = control::socketpair()...;
sup.start_unit_with_control(unit, child_fd.as_raw_fd())...;
drop(child_fd);
control::drive_refwork_start(&sock, control, game_path, ...)?;
...
emit_ready(sup, unit, snapshot);
Ok(())      // <-- `sock` dropped here; agent's SEQPACKET end closes
```

The harness holds the other end as inherited fd 3 and polls it at every
frame boundary
(`reference-workload/crates/refwork-harness/src/frame.rs::recv_boundary_msg`).
Once the agent's end closes, `ctl.rs::try_recv_datagram` sees
`recv() == 0` → `UnexpectedEof("control socket closed")` → the frame
loop errors → `main.rs` exits 1. This is the probe's observed death
directly after `Ready`, and it would equally strike under the real
worker once symptom 1 is fixed. The frame-loop's EOF-is-fatal behavior
is CORRECT (agent death must be loud) — the fix is agent-side only.

## The fix

Store the boot-leg `ControlSocket` in the `Supervisor` so it lives as
long as the workload:

- Add a field to `supervise.rs::Supervisor` (e.g.
  `pub(crate) workload_control: Option<control::ControlSocket>`),
  `None` by default.
- In `autostart_and_ready`, after `drive_refwork_start` succeeds, move
  `sock` into `sup.workload_control` instead of letting it drop.
- Lifetime contract, documented on the field: the agent holds the fd-3
  peer open for the supervised workload's lifetime; today no post-Start
  messages are exchanged on it by the agent (host-driven `HashRequest`/
  `Shutdown` legs are future work), but the workload's frame loop
  treats EOF as agent death, so early close is a protocol violation.
- Clear it wherever the workload is reaped/replaced (find the
  workload-exit path in `supervise.rs` — likely where `sup.workload` is
  cleared) so a dead workload's socket doesn't linger across restarts.
- Consider whether the supervise epoll loop should register the fd to
  detect harness-side close/faults; NOT required for this fix — do not
  scope-creep. A `Fault` datagram sent by the harness post-Start
  currently has no reader; leave that as a documented TODO unless the
  parity test (step 04) forces the issue.

## Tests (negative-tested per convention)

1. Unit test in guest-sdk: after a scripted `autostart_and_ready`
   happy path (see existing `runtime.rs` tests for the harness-less
   scaffolding — you'll need a fake workload that speaks the fd-3 leg,
   or refactor `autostart_and_ready` so the socket-retention decision is
   testable without a real child), assert the workload-side fd still
   reads WouldBlock (not EOF) after the function returns. Shown to fail
   when the retention line is reverted to a drop.
2. The real proof is the probe: after this fix, the probe boot from
   step 1 §3 must get PAST `Ready` without `control socket closed`, and
   the harness free-runs frames until the probe deadline (the probe
   "never asserts guest success" — read the serial/event dump). Record
   before/after probe output in the guest-sdk commit message or request
   resolution.
3. Step 04's VM-tier test is the durable regression guard ("held Ready
   past the first frame boundary").

## Exit criteria

- Fix + unit test merged in guest-sdk.
- Probe shows `Ready` followed by continued frame execution (no
  workload exit 1, no `control socket closed` LogLine).
