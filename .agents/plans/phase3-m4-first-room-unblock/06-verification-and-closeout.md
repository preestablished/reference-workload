# Step 06 — Verification, Closeout, And The Bridge's Standing Offer

## CI + Evidence Closeout (`refwork-d7t.15`)

- Wire the staged-fixture legs of steps 04–05 into CI. **This repo has no
  self-hosted/KVM lane yet** (current workflows run on GitHub-hosted
  ubuntu runners only) — this step creates one. Follow guest-sdk's
  `DETGUEST_VM_TESTS`-style env gating so laptop `cargo test` stays fast,
  and pick the runner label deliberately: guest-sdk uses
  `[self-hosted, intel, kvm]` while determinism-hypervisor uses
  `[self-hosted, kvm-intel]` — confirm with the operator which label the
  shared `infra-control` runner should serve for this repo.
- Append the final evidence to
  `../guest-sdk-unblock-reference-workload/m4-in-vm-first-room-evidence.md`
  with every field its "Required Replacement Evidence" section lists.
- Close the `refwork-d7t` chain per tracker convention; hand any remaining
  operator-run scheduling (ROM BLAKE3, padlog) to the operator explicitly.

## What The Bridge Will Do (Standing Offer, No Request Needed)

The moment step 03 updates the private snapshot-ref channel, the
rom-operator-bridge side will independently:

1. coordinate and execute the worker/bridge restart (we own that
   procedure and its lease-invalidation caveat, `rom-operator-bridge-72o`);
2. run `RestoreSnapshot → GetFramebuffer` through the bridge API and
   verify 229,376 bytes / XRGB8888 / 256×224;
3. open the browser preview and confirm the first real frame renders —
   the human-visible half of Phase 3 exit gate 3 — and file the result
   back into this plan directory as a numbered verification note (same
   pattern as
   `~/git/preestablished/guest-sdk/.agents/requests/phase3-ms4-region-publication-acceptance/06-verification.md`);
4. rehearse scheduled pad input from the operator surface against the
   real workload if useful for the first-room gate.

Diagnostics if anything misbehaves: every worker RPC failure is logged
with its gRPC code and message at
`journalctl -u rom-operator-bridge` (WARN); the hypervisor names offenders
precisely (layout version, byte counts) since `5698d7e`.

## Phase 3 Exit-Gate Checklist As Of This Plan's Filing

| Gate | State |
|---|---|
| 1. refwork M5 suite 20× zero-flake | this plan, step 05 |
| 2a. guest-sdk Ms4 100× readability | ✅ green 2026-07-02 |
| 2b. guest-sdk Ms5 `determinism_replay` CI gate | guest-sdk side, sequenced next there |
| 3. first room in-VM via worker gRPC | this plan, steps 03–04 + operator ROM run |
| 4. snapshot-store M7 GC property tests | independent; not started to our knowledge — flag to the operator if it stays unowned |

Reference-workload is the long pole now; steps 02–04 are pure engineering
with no external blockers left.
