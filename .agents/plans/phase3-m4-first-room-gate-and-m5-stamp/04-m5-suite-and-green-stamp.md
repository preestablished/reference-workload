# Step 04 — M5 Suite, 20× Zero-Flake, Green Stamp (`refwork-d7t.12/.13/.14`)

Phase 3 exit gate 1. Suite tooling exists (`refwork-verify vm-suite`:
in-VM double-run + restore-continuity + `--nondet-test` negative +
`--iterations` for the 20× stamp; 4 staged-fixture tests green). This
step is the lab campaign plus the stamp conversion. Prior-plan reference:
`.agents/plans/phase3-m4-first-room-unblock/05-determinism-suite.md`.

## `refwork-d7t.12` — readiness record (small, do first)

Write the M5 readiness evidence note (`m5-suite-evidence.md` or a linked
section): all external revisions (guest-sdk pin, hypervisor rev, this
repo's rev), Intel runner identity, artifact root, run owner, image
manifest hash, operator ROM BLAKE3, padlog BLAKE3, and the **exact**
suite + negative-test invocation commands. Most fields fall out of steps
01–03; this bead is bookkeeping, close it as soon as the record exists.

## `refwork-d7t.13` — the full-suite lab run

Against the real image + step 02 snapshot on the **local** worker
(scratch UDS — the deployed worker has 4 slots shared with the bridge):

1. **Double-run leg**: boot → N frames with the fixed log, twice (cold
   boots); per-frame `wram`+`framebuffer` (+ `meta` counters) host-side
   hashes bitwise-identical across runs.
2. **Restore-continuity leg**: run to mid-game frame k, `TakeSnapshot`,
   `RestoreSnapshot` in a fresh worker, resume `script[k+1..N]`;
   continued hashes equal the uninterrupted run's from k on.
3. Hashing host-side only (region reads / CaptureSpec) — no guest round
   trips in the verification path.

Suite report must include: suite_version, profile, image/repo/external
revs, ROM + padlog hashes, frames, snapshot_at, per-leg results, first
divergence diagnostics on failure.

## `refwork-d7t.14` — negative test, 20×, and the stamp

1. **Negative**: `vm-suite --nondet-test` (perturbs one pad word of run
   2) against the real image — the suite must FAIL with divergence
   localized to the perturbed frame. Record the demonstration. (The
   bead's original wall-clock-workload variant is satisfied in spirit by
   `--nondet-test`; if the reviewer wants the literal throwaway-checkout
   wall-clock build too, note the decision either way in the evidence.)
2. **20× zero-flake**: 20 consecutive full-suite runs (both legs each
   run) on the Intel lab runner, zero flakes. `--iterations 20` exists
   for this. Record per-run hashes in an evidence.json-style artifact
   under the artifact root from `.12`.
3. **Convert the stamp**: replace
   `dist/workload-image-0.1.0/determinism.unstamped.yaml` with the green
   stamp (`kind: determinism-green` or the schema the xtask expects —
   check `xtask/src/image.rs`, which already references `last_green`).
   Stamp carries: revs (repo, guest-sdk pin, hypervisor), image manifest
   hash, host, date, run owner + ROM/padlog BLAKE3s, 20/20 result,
   negative-test demonstration pointer.
4. **Register-gate bar** (IMPLEMENTATION-PLAN's own M5 bar, restated in
   the request's acceptance §3): `xtask image --register` refuses
   without a fresh green stamp, and the manifest's
   `determinism.last_green` is populated. Both are partially wired in
   `xtask/src/image.rs` — verify, finish whatever's missing, and add a
   test for the refusal path. If genuinely deferring either, record the
   reason and carry it to step 05 explicitly.

## Exit Criteria

- `.12` closed with the readiness record; `.13` closed with the passing
  double-run + restore report; `.14` closed with negative demo, 20/20
  record, and the green stamp in `dist/`.
- `xtask image --register` gate verified (or deferred with recorded
  reason).
- No game-derived content in any committed artifact — hashes and
  references only.
