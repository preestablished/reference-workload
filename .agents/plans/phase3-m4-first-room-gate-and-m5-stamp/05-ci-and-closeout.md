# Step 05 — CI Real-Worker Legs + Guest-SDK Handoff (`refwork-d7t.15`)

Prior-plan reference:
`.agents/plans/phase3-m4-first-room-unblock/06-verification-and-closeout.md`.
The staged-fixture CI legs already exist (`.github/workflows/vm-gates.yaml`);
this step adds the real-worker legs and closes the paper trail.

## Work

1. **Real-worker CI legs** in `vm-gates.yaml`:
   - Runner label per the operator's answer from step 01 (guest-sdk:
     `[self-hosted, intel, kvm]`; hypervisor: `[self-hosted, kvm-intel]`
     — do not guess; if no answer yet, this is the one item allowed to
     block).
   - Gate behind an env var (guest-sdk's `DETGUEST_VM_TESTS` pattern) so
     laptop `cargo test` stays fast; nightly/manual trigger, not per-PR.
   - Legs: `vm-first-room` and `vm-suite` (single-iteration profile —
     the 20× stamp stays a lab-manual event, that's expected).
   - If the operator decides the legs stay lab-manual entirely, record
     that decision + reason in the evidence note instead — the request's
     acceptance §4 explicitly allows it.
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
- Guest-sdk handoff updated; their external bead unblocked.
- `refwork-d7t.15` closed; epic closeout done or blocked only on step 06.
- `bd ready` shows no remaining unblocked child work for the epic.
