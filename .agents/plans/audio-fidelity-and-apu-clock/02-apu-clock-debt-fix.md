# Track B — APU Clock Debt-Carry Fix

**GATED: this fix MUST NOT merge to main — nor land in any binary an
operator or lab runs — until the operator has accepted the epoch bill (03)
and the decision is recorded in `.agents/decisions/`.** Preparing it on a
branch is allowed (useful for an atomic epoch cut); merging is not. Per
`docs/emulator-performance-profile.md:97-98`, behavior-changing candidates
wait for the owners to accept the re-baseline cost. This fix changes
SPC/timer timing → CPU-visible state → WRAM/framebuffer/frame hashes.

Governance tripwire: the four committed overshoot tests assert the
CURRENT (buggy) numbers and pass; any debt-carry implementation turns them
red, so an ungated landing cannot slip through a green suite. Do not
"clean up" these tests — they are the enforcement mechanism until the
epoch decision flips them.

## The fix

Add a debt field to `Apu` (next to `spc_accum`, `apu/mod.rs:156-160`,
init 0 in `Apu::new()`):

```rust
/// SPC cycles executed beyond a previous call's budget (instruction
/// overshoot), repaid by shrinking future budgets. Bounded by the largest
/// single step() return (36, the IPL-HLE command step).
spc_debt: u64,
```

In `advance_master_cycles` (:795-828):

```rust
self.spc_accum += master_cycles * SPC_NUM;
let mut spc_to_run = self.spc_accum / SPC_DEN;
self.spc_accum %= SPC_DEN;

// Repay overshoot from earlier calls before running new cycles.
let repaid = self.spc_debt.min(spc_to_run);
self.spc_debt -= repaid;
spc_to_run -= repaid;

// ... existing while loop unchanged (timers/DSP still see exactly the
// stepped cycles — debt only shrinks FUTURE budgets) ...

// Carry the final instruction's overshoot into future budgets.
// saturating_sub: a halted early-break leaves spc_ran <= spc_to_run -> 0.
self.spc_debt += spc_ran.saturating_sub(spc_to_run);
```

Long-run SPC rate becomes exactly `SPC_NUM/SPC_DEN` of master, making the
crate's own `AUDIO_SAMPLE_RATE_HZ` contract (lib.rs) true.

Design points (consultant-verified against source):
- Units are whole SPC cycles; do NOT fold into `spc_accum` as signed ticks
  (preserves the documented `0..SPC_DEN` remainder invariant, :157).
- Halted early-break: keep dropping the un-run remainder; never negative
  debt (would burst-run on wake). Debt freezes while halted (break precedes
  `step()`); no wake path exists in-core, so no unbounded growth.
- Hard bound `spc_debt ≤ 35` (max `step()` return is the 36-cycle IPL-HLE
  command step; repayment precedes the loop so debt at loop entry is 0).
  Encode as `debug_assert!(self.spc_debt < 64)` + a unit test.
- `cycles == 0 -> 1` branch: keep as-is (defensive, dead in practice).
- No serialization impact: `spc_accum`/`spc_debt` cross no on-disk format.

## Explicitly out of scope (file follow-up beads, do not fold in)

- `service_pending_ipl_cc` (`bus.rs:389-407`) steps one instruction outside
  any budget (zero-time; no timer/DSP effect). Pre-existing, IPL-only.
- DSP halt behavior: hardware keeps the DSP (and echo) running during
  SLEEP/STOP; this core freezes it. Opposite-sign, separate decision.
- The 36-cycle IPL-HLE step granularity itself.

## Tests

- Flip the four in-tree overshoot tests to their post-fix assertions (the
  agents left the correct expected values in comments):
  chunked-vs-single-call sample counts equal within ±1; one emulated second
  yields 32,000 ± 1 samples regardless of chunking; overshoot excess ≤ 35.
- Extend the four in-tree overshoot evidence tests (there is no
  pre-existing chunk-invariance suite — do not confuse them with the
  committed `audio_tap_deterministic_across_independent_apus`, which is
  run-vs-run at identical chunking): total executed SPC cycles ==
  `master*SPC_NUM/SPC_DEN` within the debt bound over long runs and
  pathological chunkings (many 1-master-cycle calls).
- Halt-freeze test: debt does not change while halted.

## Rollout (inseparable from the fix commit)

- Bump `EMU_VERSION` (`refwork-emu/src/lib.rs:47`) — it flows into the
  harness meta region and determinism reports, making the epoch
  machine-visible.
- Amend `.agents/plans/interactive-sound-and-pad-lr/00-overview.md`'s
  "default build byte- and icount-identical" premise with a dated note
  pointing at the epoch decision.
- Then execute 03's protocol steps 5-10 (gates, package-06 re-stamp,
  discovery-01 migration, lab expect files, sibling program, session.yaml
  version field).

## Acceptance criteria

1. Real-session replay: pairs/frame is 532-533 for every frame (the
  variance collapses), total rate 32,000 ± 1 Hz against emulated time.
2. All flipped tests green; full workspace-minus-harness suite green;
   10k determinism gate and a 100k double-run green.
3. Music tempo verified stable by operator listen (timer over-clocking
   gone).
4. Epoch decision document exists in `.agents/decisions/` BEFORE the merge
   (consistent with the header: branch preparation allowed, merge gated).
