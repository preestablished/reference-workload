# Implementation Progress — 2026-07-11

Commit `ccaf1b4` lands the host-only profiling layer and synthetic findings.
This is a progress record, not `04-resolution.md`: the request remains open on
operator-private and matched KVM evidence.

## Landed

- dependency-light `refwork-emu-bench` with synthetic and private-padlog inputs,
  fixed warmup/measurement windows, privacy-safe JSON, faults-as-failure, and
  acknowledged perf interval control;
- authoritative and sampling drivers under `tools/refwork-emu-perf/`;
- synthetic boot/steady measurements and a 4,047-sample, zero-loss attribution;
- findings at `docs/emulator-performance-profile.md` with Amdahl pricing,
  determinism blast radii, re-baseline ownership, A1 sequencing advice, and the
  qualified `38b6` answer;
- before/after byte-identical benchmark proof against pre-request `6cdeb3e`;
- determinism, zero-allocation, 5.12M CPU-corpus, and 256k SPC700-corpus gates
  green as recorded in the verification appendix;
- follow-ups: `refwork-4nv` (busy-scene fallback), `refwork-rbz` (PPU),
  `refwork-0um` (CPU/bus), and `refwork-hbh` (APU);
- hypervisor `38b6` pointer comment
  `019f4f53-cab3-7365-a5cd-07ae83ae3b1d`.

## Required Before Resolution

1. Restore the operator-private ROM and 5,000-frame first-room padlog through the
   documented private intake and run the same authoritative/sampling windows.
2. Reproduce a matched host-side KVM guest event with recorded attributes,
   artifact/features, vCPU scope, and exact frame interval; reconcile the
   residual or fail if unexplained cost exceeds approximately 15–20%.
3. Obtain final-revision x86_64/aarch64 CI hash-chain evidence.
4. Append `04-resolution.md` only after those criteria pass.

The retained first-room JSON contains hashes and aggregate success evidence but
not the private inputs, so it cannot be replayed or profiled. No completion claim
or fabricated calibration is made.
