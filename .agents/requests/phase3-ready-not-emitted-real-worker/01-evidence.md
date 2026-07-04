# Evidence (Observed 2026-07-04)

## Real Worker — `dh-m9-ready-handoff`, scratch paths

The M9 handoff (now instrumented — determinism-hypervisor `44c44f5`
dumps buffered guest events on a non-Ready stop) failed with:

```text
stop reason 4 (HARD_CAP), expected NextSdkEvent;
  icount=10000000000 vns=10000000000 frames=0
  event stream=1 icount=640974488 payload[16]=................   # Hello
  event stream=9 icount=642805868 payload[8]=........            # WorkloadStarted
  event stream=2 icount=642805868 payload[16]=........wram....   # NameIntern wram
  event stream=7 icount=642805868 payload[16]=................   # RegionRegister wram
  event stream=2 icount=643013837 payload[24]=....framebuffer.. # NameIntern framebuffer
  event stream=7 icount=643013837 payload[16]=................   # RegionRegister framebuffer
  event stream=2 icount=643039308 payload[16]=........meta....   # NameIntern meta
  event stream=7 icount=643039308 payload[16]=................   # RegionRegister meta
  (… then 9.3 billion instructions, no further events, no Ready …)
```

All three regions register through the real `register_region` path
(`manifest_generation 6`), then the guest goes silent. **No guest-sdk
`Ready` (stream 8) event — 0 occurrences.** Full trail preserved at the
scratch root's `real-worker-trail.txt`.

## Device-Less Probe — guest-sdk VM harness, same image

`guest-sdk/tests/vm/tests/boot_probe.rs` with `BOOT_PROBE_GAME` set
(the pv-blk model attached) boots the *same* rebuilt image and reaches
one step further:

```text
… (Hello, WorkloadStarted, wram/framebuffer/meta register, gen 6) …
GuestEvent Ready { unit: 0, region_count: 3, manifest_generation: 6 }   # emitted!
GuestEvent LogLine (stream 2): "refwork-harness: frame loop failed:
                                 control I/O error: control socket closed"
GuestEvent WorkloadExited { guest_pid: 15, exit_code: 1, term_signal: 0 }
```

So under the probe the agent **does** emit `Ready` right after `meta`,
and the harness then dies in its frame loop.

## The Two Linked Symptoms

1. **Agent side (guest-sdk):** under the real worker, `Ready` is never
   emitted despite regions being registered and (presumably) Start being
   sent — the `ready_after = "regions-registered-and-start-sent"` gate
   never fires, or the emission stalls. Under the probe it fires. The
   environmental delta is the real vs stubbed device set (pv-pad real vs
   RAZ, real pv-blk vs model) and the epoch-structured run control.
2. **Harness side (refwork-harness):** immediately after `Ready`, the
   frame loop (`crates/refwork-harness/src/frame.rs::FrameLoop::run` →
   `recv_boundary_msg`) hits `control socket closed`
   (`crates/refwork-harness/src/ctl.rs:212/241`) and `main.rs:51`
   exits 1. Whether this is a probe-only artifact or would also strike
   under the real worker once Ready is reached is unknown — the real
   worker never gets past symptom 1 to find out.

## Ownership

- Symptom 1 (Ready emission timing) is guest-sdk agent territory
  (`detguest-agent` control leg + SDK Ready gate). The staged M9 fixture
  (`m9_refwork_contract.rs`) emits Ready under the real worker fine, so
  the difference is the *real refwork-harness* control leg vs the
  fixture's minimal one.
- Symptom 2 (frame-loop control-socket teardown) is refwork-harness
  territory outright.

Both need to be green under the real worker for step 2 to snapshot.
