# Decision: APU Clock Fix Epoch Cut Accepted (Both Audio Tracks)

Date: 2026-07-16. Decided by Matt (operator), verbatim instruction to the
coding agent: "We want both tracks implemented. DO it" — in response to
the bill presented per
`.agents/plans/audio-fidelity-and-apu-clock/03-epoch-cut-protocol.md`
(bead refwork-bp1).

## What is accepted

- **Track B** (`02-apu-clock-debt-fix.md`): the `spc_debt` carry fix in
  `Apu::advance_master_cycles` lands on main. This is a determinism epoch
  cut: SPC/timer timing changes → CPU-visible state → WRAM/framebuffer/
  frame hashes change. `EMU_VERSION` is bumped as the machine-visible
  epoch marker.
- **Track A** (`01-dsp-fidelity-fixes.md`): the D1-D10 DSP synthesis fixes
  land in the same epoch. With Track B accepted, Track A's standalone
  state-epoch gate is moot as a *gate* (the epoch is being cut regardless);
  it survives as a verification step only.
- **The bill** (03 §"What breaks"): pre-fix session resume verification,
  the m6 16/16 replay gate artifacts (16 dumps, 1,005-capture frozen
  corpus + corpus id, gate-3 trajectory/scorer record, map-check
  progression evidence, downstream handoffs), operator lab expect files,
  sibling-program icount assets (incl. the staged `--hard-icount-cap`
  inputs), the package-06 determinism stamp, and dated perf docs. Owner
  for re-baseline coordination: Matt (operator), on behalf of the sibling
  program owners per profile.md §4.

## Standing consequences

- The recording freeze on discovery-02 remains until both tracks are
  merged and gates are green; discovery-01 migration follows
  03 §Rollout steps 5-10 (tracked as follow-up beads).
- Pre-fix recorded sessions resume only via `--skip-replay-verify` until
  migrated.
