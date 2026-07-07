# Step 06 — M2 Paper Trail: Resolve `refwork-d7t.1`

Small, independent of the lab chain — interleave anytime. The bead is P1,
currently BLOCKED, and it blocks the `refwork-d7t` epic, so the epic
cannot close without it. "Closed", not "a record exists", is the bar
(request acceptance §5).

## Context

`gaps.md` (2026-06-15) declared M2 not achieved: no host-side first-room
evidence, no build-vs-vendor decision record. Partial evidence already
exists at
`.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md`
(synthetic floor recorded by Ralph iteration 1; reviewers found no
durable operator waiver or lab artifacts). The subsequent emulator
accuracy fixes and the real first-room run (step 03) likely satisfy the
substance.

## Work — extend `m2-floor-evidence.md` with:

1. **Host-side first-room evidence pointer**: cite the step 03 lab
   evidence section (and/or the host-side `refwork-verify double-run`
   evidence at the current rev). Pointer + hashes, not content.
2. **Build-vs-vendor decision record**: per step 01's operator ask —
   either link the existing decision (the kernel/agent artifact split
   decision doc `.agents/decisions/2026-07-02-kernel-agent-artifact-split.md`
   may already BE the substance; if the operator agrees, record that
   equivalence explicitly) or record the operator's waiver verbatim with
   date and owner.
3. **aarch64 cross-arch double-run** — the known open question. Two
   allowed outcomes, per the request:
   - run it (host-side `double-run` on an aarch64 host, hashes recorded), or
   - an explicit recorded deferral with the operator's reason and a
     tracking pointer (new low-priority bead) so it isn't silently lost.
4. If some M2 substance genuinely remains beyond the above, say so in the
   file and leave the bead open with a precise remaining-work note —
   don't force-close.

## Bead mechanics

The bead has no open dependency edges (it's status-BLOCKED, not
dep-blocked). Once the evidence file is extended and the operator inputs
recorded:

```bash
bd update refwork-d7t.1 --status open   # or the tracker's unblock verb
bd close refwork-d7t.1 -r "M2 floor evidence completed: host-side first-room pointer, build-vs-vendor record/waiver, aarch64 run-or-deferral recorded in m2-floor-evidence.md"
```

(Verify the exact unblock command with `bd --help`; do not `--force`
through a pin without checking why it's pinned.)

## Exit Criteria

- `m2-floor-evidence.md` extended with items 1–3 (each either satisfied
  or explicitly waived/deferred with owner + date).
- `refwork-d7t.1` closed (or left open with a precise recorded reason —
  which must then be surfaced to the operator, since it holds the epic).
