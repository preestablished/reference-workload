# Step 4: VM-Tier Test — Real Harness To Held Ready, Worker-Parity Devices

Green criterion 1 from the request's `02-repro.md`: a VM-tier test that
boots the **real refwork-harness** (not the `m9_refwork_contract`
fixture) through to a *held* guest-sdk `Ready` and past the first frame
boundary, under a device set matching the real worker. Repo:
**guest-sdk** (`tests/vm`), following `boot_probe.rs` /
`m4_acceptance.rs` patterns.

## Why this test exists

The staged M9 fixture emits Ready fine; the probe reaches Ready but
with stubbed devices and a continuously-draining host. Neither covers
the environment that actually failed. This test must close that gap so
the class of bug in steps 02/03 can never silently regress.

## Shape

New gated test target in `guest-sdk/tests/vm/tests/` (e.g.
`refwork_ready_hold.rs`), env-gated like the probe
(`REFWORK_READY_INITRAMFS=` path to the reference-workload cpio,
skip-with-message when unset) so guest-sdk CI without the sibling image
still passes. Reference-workload's step-05 verification runs it with
the freshly built image.

Sequence:

1. Boot the real initramfs (kernel from `image/build/bzImage` as the
   probe does) with a synthetic 512-aligned 32 KiB game attached via
   pv-blk (`vm.attach_pv_blk`, matching `game_source = "pv-blk"`).
2. **Device parity:** real pv-pad device model (not the RAZ/latch
   stub) and real pv-blk — audit what `VmHarness`/`VmConfig` in
   `tests/vm/src/harness/` offer vs what the production worker
   configures, and extend the test harness where it falls short. If
   full parity with the worker's epoch-structured run control is not
   reachable in this tier, replicate the deltas that matter and were
   implicated by step 03's root cause — at minimum: (a) do NOT drain
   ring A while running (buffer like the worker; drain only at
   checkpoints), (b) run-until-event semantics rather than
   free-running with continuous service. Document any residual
   parity gaps in the test's module doc so nobody mistakes it for full
   worker coverage.
3. Assert, in order:
   - guest-sdk `Ready` event observed with `region_count == 3` and a
     validated `manifest_generation` (don't hardcode 6 unless the
     boot is deterministic enough to pin it — the trails suggest it
     is; if you pin it, comment why);
   - the boot-leg breadcrumbs from step 01 appear in order (cheap
     ordering guard);
   - **held:** after Ready, run further (bounded: N more epochs or a
     wall deadline) and assert the workload has NOT exited, no `Fault`
     / agent LogLine containing `frame loop failed` appears, and the
     `meta` region's frame counter (offset 0x08) has advanced past the
     first frame boundary. Reading `meta` from the host side proves
     "past the first frame boundary" without any new protocol.
4. **Negative test** (convention): prove the test catches the symptom-2
   class. Options, pick the cheapest honest one: run the same assertion
   body against an initramfs built with the pre-fix agent rev (awkward
   across repos), OR unit-level: keep step 02's unit negative test as
   the reversion guard and have this VM test assert the observable
   (workload alive past first boundary), documenting that it fails on
   `322c331`-era agents — verify that claim once by actually running it
   against the old image and record the failure output in the commit
   message.

## Exit criteria

- Test merged in guest-sdk, green against the fixed image, skip-clean
  without the env var.
- Verified-failing against the pre-fix agent (recorded evidence).
- Runbook line added for reference-workload: exact env + command to run
  it from this repo's dist image (goes into step 05's verification and
  the request's `03-resolution.md`).
