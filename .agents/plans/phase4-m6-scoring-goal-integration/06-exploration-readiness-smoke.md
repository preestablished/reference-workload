# Package 06 — Exploration-Readiness Smoke (Item 5)

**Gate:** GATE-RECORD.md exists AND scorer M4 running AND stack confirmed up
AND window coordinated. This is the most operationally entangled package —
the checklist below is sequential and none of it is skippable.

## Pre-flight checklist

1. **Stack up:** bridge systemd unit alive; dh-workerd (expected `6e348e5`
   or successor — record actual) responding; snapstore durable copy
   reachable (`~/.rbo73/m4-regen-20260707/` or successor). Worker + snapstore
   are user processes and die on reboot — verify now, not from memory. If
   dangling-intent 503s appear, recover via the bridge's audited
   `clear-dangling-intents` subcommand and note it in the evidence.
2. **Window coordination:** `rom-operator-bridge-l1w` (hypervisor RSS-leak
   live verification) — if still open, the smoke must not overlap live Play;
   agree the window with the operator/program flags and record the
   agreement. Also dedupe with the scorer packet's M4 joint smoke: the two
   smokes may share a window but are **different tests recorded
   separately** — this one counts `Fault`s and determinism-hash mismatches;
   theirs proves the scoring loop.
3. **Input source:** input-synthesizer v1 if its packet closed (better
   evidence), else a trivial random-input loop (explicitly sufficient per
   the plan). Never wait on the synthesizer — record which was used.

## The smoke

Orchestrator dev loop pointed at the real stack + live scorer, against the
`dist/` workload image:

- **1,000 bursts** (the orchestrator's burst unit, per their dev-loop docs —
  coordinate the definition with their side and write it down; a "burst"
  miscount makes the evidence unusable).
- **Zero `Fault`s.**
- **Zero determinism-hash mismatches on spot-replayed branches** — record
  the spot-replay policy used (how many branches, how chosen) with the
  counts; "zero mismatches on zero replays" proves nothing, so the policy
  must guarantee a meaningful sample (double-digit replays at minimum;
  agree the number with the orchestrator side).

Any `Fault` or mismatch stops the run for diagnosis — a smoke that "passes"
by ignoring faults is a fabrication. Rerun-from-zero after a fix; partial
tallies don't concatenate.

## Evidence

Record: burst count, Fault count, spot-replay count + mismatch count,
orchestrator/scorer/worker build SHAs, image id, window-coordination note,
input source, wall-clock span. This block is quoted verbatim in the
resolution and read by the phases track (their check 4).

## Exit Signal

Smoke evidence recorded with all counts and the coordination note; separate
from (but window-cross-referenced with) the scorer packet's joint smoke.
