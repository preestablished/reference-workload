# Step 02 — Refresh The Readiness Evidence (`refwork-d7t.10`)

Goal: move `refwork-d7t.10` out of BLOCKED honestly, by re-running the
audit's external-surface verification against the new upstream state and
recording it in
`../guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`
(append a new dated verification-log section; do not rewrite history).

Note: that file carries a large **uncommitted** section in this checkout —
a complete prior verification pass dated 2026-06-22 (inspecting
`guest-sdk@08abbbc` / `hypervisor@b973753`) that reached a BLOCKED
verdict. It predates and is **superseded by** the 2026-07-02 upstream
changes in `01-…`. Do not extend or edit it: commit it as-is (honest
historical record), then append your new dated section with the refreshed
verdict.

## Re-Verify, With Current Revisions Recorded

1. **GS-5 (real path):** in guest-sdk, run the host tier
   (`cargo test -p detguest-agent -p detguest-sdk -p detguest-host --locked`)
   and cite the Ms4 acceptance artifact root + `evidence.json` BLAKE3 for
   the VM-tier proof. The previous PARTIAL verdict's missing piece ("real
   reference-workload path" acceptance) is the Ms4 acceptance itself —
   cite it rather than re-running if fresh enough, or re-run
   (`DETGUEST_VM_TESTS=1 cargo test -p detguest-vmtest --locked --test
   m4_acceptance regions_readable_and_stable_across_100_snapshot_restore_branches
   -- --ignored --test-threads=1`; ~31 s on this host).
2. **GS-6:** same evidence covers host-side reads of `wram`/`framebuffer`/
   `meta` through the manifest across restore/fork. Record the closed
   guest-sdk bead IDs.
3. **DH-2 / DH-5:** re-cite against hypervisor `5698d7e` or later — update
   the stale test names (see `01-…`). The capture-path determinism fix in
   `5698d7e` is *stronger* than what the audit required (captured FbInfo
   no longer frame-content-dependent).
4. Record the revision table (all five repos) as the audit's format
   requires. Note guest-sdk and determinism-hypervisor both carry local
   unpushed commits — record the SHAs you actually used.

## What Stays MISSING (Human Assignment)

The operator-run fields (run owner, operator ROM BLAKE3, first-room padlog
BLAKE3) remain a human decision. Do not fabricate them; the bead can move
to READY-for-lab-run with those fields explicitly assigned to the operator
(Matt) rather than BLOCKED-on-upstream.

## Exit Criteria For This Step

- New dated section in the evidence note with the re-verification table:
  GS-5 PRESENT, GS-6 PRESENT, DH-2/DH-5 PRESENT (fixture-level+), operator
  fields MISSING-by-assignment.
- `refwork-d7t.10` updated (claim it, note the refresh, close or re-scope
  per your tracker's convention).
