# Step 05 — M5 Full Determinism Validation Suite (`refwork-d7t.12`–`.14`)

Goal: Phase 3 exit gate 1. Per the phase plan
(`~/.agents/projects/determinism/phases/phase-3-workload-in-the-box.md`):

> boot→N frames with a fixed log twice → per-frame RAM+framebuffer hashes
> identical; snapshot mid-game → restore → continue → identical to
> uninterrupted run; 20× zero-flake.

## Work

1. **Double-run leg:** same image, same input log, two cold boots; hash
   `wram` + `framebuffer` (+ `meta` counters) at every FrameMark via host
   region capture; assert bitwise-identical hash sequences.
2. **Restore-continuity leg:** run to a mid-game frame, `TakeSnapshot`,
   `RestoreSnapshot`, continue with the remaining log; the continued run's
   per-frame hashes must equal the uninterrupted run's from that frame on.
3. **Negative test** (`refwork-d7t.14`): deliberately perturb one input
   frame (or one byte of the log) and assert the suite *fails* — a suite
   that cannot fail proves nothing. Guest-sdk's Ms4 acceptance used the
   same pattern (distinct seeds ⇒ distinct wram); mirror it.
4. **20× zero-flake stamp:** 20 consecutive suite runs, no flakes, on the
   Intel runner; evidence.json-style artifact with per-run hashes and the
   green stamp (`refwork-d7t.12` evidence discipline).
5. Hash cross-checks host-side only (region reads / CaptureSpec
   `feature_bytes` + `fb_lz4`) — no guest round trips in the verification
   path, same rule the Ms4 acceptance enforced.

## Notes

- **This is not the existing `refwork-verify double-run`** (an in-process,
  host-side `Core` hash comparison). That command stays as the fast
  pre-VM check; this step is a separate in-VM suite driven through the
  hypervisor worker via step 04's gRPC infrastructure — structurally
  similar assertions, different execution substrate. A suite that never
  leaves the host process does not satisfy M5.

- The suite must re-run cleanly against both the staged fixture (CI) and
  the operator image (lab), like step 04's command — determinism claims
  that only hold for the fixture are not M5.
- Budget slot usage: the deployed worker has 4 slots and other consumers
  (the bridge). Prefer a locally-launched worker for suite runs; reserve
  the deployed one for the joint verifications in `06-…`. To launch one:
  build `dh-workerd` from a clean hypervisor worktree (see `01-…` on
  `.dh-clean-ff1e88c`) and run it with `serve --uds <scratch>.sock
  --image-cache … --snapstore-uds …` pointed at scratch paths — never at
  `/run/dh/grpc.sock`, which belongs to the deployed worker.

## Exit Criteria

- Suite implemented, negative test proves it can fail, 20× green stamp
  recorded with artifact root + hashes.
- `refwork-d7t.12`, `.13`, `.14` closed.
