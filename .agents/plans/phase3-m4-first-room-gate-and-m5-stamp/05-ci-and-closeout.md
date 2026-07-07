# Step 05 — CI Real-Worker Legs + Guest-SDK Handoff (`refwork-d7t.15`)

Prior-plan reference:
`.agents/plans/phase3-m4-first-room-unblock/06-verification-and-closeout.md`.
`.github/workflows/vm-gates.yaml` already has the staged-fixture legs
AND a `live-worker smoke` leg (builds `dh-workerd` from the sibling
checkout, runs `refwork-dh-client --test live_worker_smoke`), gated by
`REFWORK_VM_TESTS=1` on runner label `[self-hosted, intel, kvm]` — the
label was confirmed by the operator 2026-07-02 per the file's own inline
comment. This step extends that, it does not start from scratch.

## Work

1. **Real-worker CI legs** in `vm-gates.yaml`:
   - Add `vm-first-room` and `vm-suite` legs (single-iteration profile —
     the 20× stamp stays a lab-manual event, that's expected), reusing
     the existing live-worker-smoke recipe: same runner label, same
     `REFWORK_VM_TESTS=1` gating, same sibling scratch-worker build.
   - Nightly/manual trigger, not per-PR.
   - If the operator decides the new legs stay lab-manual entirely,
     record that decision + reason in the evidence note instead — the
     request's acceptance §4 explicitly allows it.
2. **Deferred items from step 04**: if the `--register` refusal gate or
   `determinism.last_green` manifest population was deferred, finish it
   here — acceptance §3 requires either done-in-04 or
   done-here-with-recorded-reason; it cannot silently drop.
3. **Guest-sdk handoff**: update their handoff file so
   `guest-sdk-ext-refwork-m5-full-suite` (external bead, guest-sdk repo)
   can close — point it at the green stamp, the 20/20 evidence artifact
   root, and the closed `refwork-d7t.12–.14` beads. Follow the existing
   handoff-file location/format used for prior guest-sdk↔refwork
   exchanges (see `../guest-sdk/.agents/requests/` for the pattern).
4. **Final evidence append** to
   `../guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`
   covering every field its "Required Replacement Evidence" section
   lists.
5. **Bead closeout**: close `refwork-d7t.15`, then check the epic —
   after step 06 closes `refwork-d7t.1`, `refwork-d7t` itself should have
   no open children; close the epic per tracker convention.

## Exit Criteria

- CI shows the real-worker legs (or the recorded lab-manual decision).
- **Acceptance §2 fully satisfied before declaring done**: confirm the
  bridge's browser-side verification note actually landed in the request
  directory (step 03 allowed it to be "pending"; closeout does not). If
  it is still pending, say so explicitly in the closeout record instead
  of silently closing.
- Guest-sdk handoff updated; their external bead unblocked.
- `refwork-d7t.15` closed; epic closeout done or blocked only on step 06.
- `bd ready` shows no remaining unblocked child work for the epic.
