# Step 04 — Run First Room Privately And Reconcile The KVM Guest Number

## Private Intake

Follow `docs/phase4-corpus-guide/01-private-intake.html` and the existing
operator-approved first-room evidence. The operator supplies the ROM and
script/log inside the private lab boundary. Public evidence may contain hashes,
frame ranges, aggregate counts/times, emulator revision, and clean-room-safe
case labels only.

Do not commit or print ROM bytes, script contents/semantics, screenshots,
framebuffers, WRAM, SRAM, exact private paths, capture IDs that disclose private
layout, or game-derived goldens. Review raw command output before copying any
aggregate into the repository.

Run the same short/long authoritative and sampling procedure used for synthetic
steady state. The final attribution must cover >=90% of the first-room host lane
independently. If no busy-scene script exists, confirm the predefined fallback
and file one explicit follow-up bead tied to the round-2/hand-play trajectory.

## Comparable Guest Calibration

The upstream 27.8M instructions/frame and 90–115 ms/frame are KVM guest-mode
whole-guest measurements, not host-process measurements. Obtain or reproduce a
guest run only through the existing hypervisor/lab workflow. Use:

- the same emulator revision and `--locked --release` profile;
- musl target where practical and the same ROM/script/frame window;
- `PERF_COUNT_HW_INSTRUCTIONS` with `exclude_host`, as in the upstream lane;
- exact attributes recorded: `exclude_host=1`, `exclude_guest=0`, guest user and
  guest kernel both included, all relevant vCPU threads counted, and no relevant
  agent/harness execution outside the counted scope;
- enough warmup and frames to separate boot from steady state;
- >=3 repetitions and raw total guest instructions/frames;
- guest image, kernel, agent, harness, and emulator revision identities.

Record event enabled/running time and require unscaled, non-multiplexed counts.
An in-guest `perf stat` is not automatically equivalent to dh-detclock's
host-side KVM event. Record the enable/disable interval and frame-boundary source.

If exact upstream inputs/window/build cannot be reproduced, do not subtract the
numbers directly. First record every mismatch, reproduce a closest-comparable
host case, and present the old `38b6` number as historical context rather than a
paired calibration.

## Residual Accounting

Before subtraction, match or account for target artifact, feature set, ROM/input
schedule, exact start/end frames, blit/hash/proof work, fault/output path, and
vCPU count. Host direct-`Core` execution and guest harness execution are not
comparable merely because they share a revision. For a genuinely comparable pair
calculate:

```text
guest residual/frame = whole-guest instructions/frame
                     - host emulator-process instructions/frame
```

Then measure or bound guest kernel, agent/harness, and benchmark-driver portions
where the guest tooling allows; otherwise leave them explicitly unresolved.
Build/target differences are confounders unless independently measured, not
invented additive rows.
Do not force residual cost into emulator subsystem rows. Report both absolute
instructions/frame and percentage of the whole guest.

A negative residual automatically triggers a denominator/path mismatch
investigation.

The acceptance target is a quantified host↔guest gap within roughly 15–20%.
Interpret that as unexplained residual after known components/build differences,
not permission to hide a 20% “other” row. If unexplained residual remains above
the range, the acceptance criterion fails: investigate denominator/window/event
configuration and leave the request open with the exact blocker.

Also compare wall time only on like hardware/configuration. The host process
wall number is not algebraically subtractable from the upstream KVM wall number;
use it to discuss throughput and overhead, with hardware differences explicit.

## Exit Criteria

- First-room boot/steady authoritative results and >=90% host attribution exist
  without leaking private material.
- Busy-scene is either measured from a real scripted log or explicitly deferred
  in its own bead.
- The KVM guest and host denominators are matched and reconciled, or the request
  is explicitly left incomplete with mismatch evidence.
- The residual has named rows and no unexplained portion above the accepted
  range.
