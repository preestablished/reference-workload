# 06 - Full Determinism Suite

**Upstream package:** RW-4.

**Purpose:** implement `refwork-verify` full-stack determinism validation:
boot/run/reboot double-run, mid-run snapshot/restore continuity, in-guest
hash cross-checks, host region reads, and `meta.frame` verification. This
closes `guest-sdk-ext-refwork-m5-full-suite`.

## External Dependencies

- Package 05 in-VM first-room gate.
- guest-sdk GS-8 replay hooks.
- determinism-hypervisor DH-6 Linux `determinism_replay` and replay evidence.
- snapshot-store SS-1 or equivalent snapshot/restore API for fresh-worker
  restore.

## Deliverables

1. Suite command:

   ```sh
   refwork-verify suite --image <workload-image.yaml> --rom <operator>.rom --script <fixed>.padlog --frames <N> --snapshot-at <k> --report <out.json>
   ```

   Required profiles:
   - `--quick`: emulator-only or reduced-stack profile, `N = 600`, suitable for
     PR-time synthetic runs.
   - `--full`: real hypervisor, real image, `N = 36000` unless the lab owner
     overrides it in the evidence note.
2. Double-run test:
   - Boot image fresh.
   - Feed fixed input log for `N` frames through the hypervisor path.
   - Capture per-frame host hashes over `wram` and `framebuffer`.
   - Request in-guest `HashReport` at frame boundaries and compare with host
     reads.
   - Reboot fresh and repeat.
   - Require byte-identical per-frame hash sequences.
3. Snapshot/restore test:
   - Run to frame `k`.
   - Take a snapshot through snapshot-store/hypervisor APIs.
   - Continue uninterrupted to `N`, recording hashes.
   - Restore the snapshot in a fresh worker.
   - Feed `script[k+1..N]`.
   - Require hashes from `k+1` onward to match the uninterrupted run.
4. Cross-checks:
   - `meta.frame` equals the hypervisor frame table at every checked boundary.
   - `meta.last_pad` equals the low 16 bits of the scheduled pad word.
   - `HashReport.wram` and `HashReport.fb` equal host-read hashes.
   - Region sizes and framebuffer format match the WorkloadImage manifest.
   - Region `layout_version` values match the package-04 `boot.toml` or
     expected-region handoff file. Do not treat layout versions as
     WorkloadImage fields unless the reference-workload API is updated.
5. Divergence diagnostics:
   - Report first divergent frame.
   - Report divergent surface: event stream, drop counter, `wram`, framebuffer,
     `meta`, input landing, or in-guest/host hash mismatch.
   - For region byte mismatches, include the first offset window and a compact
     hex diff in the lab report. Do not commit game-derived bytes.
6. Negative gate:
   - Preserve the existing host-side `refwork-verify double-run --nondet-test`
     check.
   - Add the owner-doc full-stack negative test: run the suite against an
     intentionally nondeterministic workload test build that performs a
     wall-clock read, and require the suite to fail with the first divergent
     frame plus first divergent region/offset window.
   - Because the normal deny gate intentionally token-scans `refwork-emu` and
     `refwork-harness`, the nondeterministic build must be isolated from normal
     source and CI. Acceptable approaches are a generated patch in a throwaway
     lab checkout or a separately generated test source tree. The evidence must
     record the patch/source hash and prove the normal checkout remains
     deny-clean.
7. Green stamp:
   - On success, write or update `determinism.last_green` for the image version
     in the manifest or a sidecar that package 04's
     `cargo run --locked -p xtask -- image register` can consume.
   - Include suite version, git rev, timestamp from CI/lab metadata, and report
     hash. Do not make the harness read wall-clock time.

## Report Schema

The JSON report should be stable enough for CI and guest-sdk blocker closeout:

- `suite_version`
- `profile`
- `image_manifest_hash`
- `reference_workload_git_rev`
- `guest_sdk_rev`
- `hypervisor_rev`
- `snapshot_store_rev`
- `operator_rom_blake3`
- `padlog_blake3`
- `frames`
- `snapshot_at`
- `double_run`: pass/fail, first divergent frame, chain hashes
- `snapshot_restore`: pass/fail, first divergent frame, chain hashes
- `cross_checks`: `meta_frame`, `last_pad`, `hash_report`, region sizes
- `artifacts`: lab paths to logs and large reports

## Lab Evidence Configuration

Before running the full suite, record these fields in the evidence note:

- owner responsible for the run;
- runner label or machine name, including the Intel box used for the 20-run
  zero-flake acceptance;
- artifact root for JSON reports, logs, and large byte-window diagnostics;
- guest-sdk, hypervisor, snapshot-store, and control-plane revisions;
- image manifest hash, operator ROM BLAKE3, and padlog BLAKE3;
- exact command used for the full suite and exact command used for the
  nondeterministic-build negative test.

## Acceptance

- Full suite passes 20 consecutive runs on the Intel lab box with zero flakes.
- A single divergence fails the suite; do not classify it as a flaky pass.
- Double-run proves byte-equal per-frame `(wram, framebuffer)` hash sequences.
- Snapshot/restore proves restored hashes match the uninterrupted run from
  `k+1` onward.
- In-guest `HashReport` matches host region reads.
- `meta.frame` matches the hypervisor frame table.
- The nondeterministic wall-clock test build fails and localizes the first
  divergent frame plus first divergent region/offset window.
- `cargo run --locked -p xtask -- image register` or equivalent refuses to
  stamp/register the image without a fresh green suite report.

## Stop Conditions

- If snapshot restore does not use a fresh worker, the test is not sufficient
  for RW-4.
- If host capture can occur away from `FrameMark` boundaries, stop and fix the
  hypervisor gate.
- If reports contain game-derived bytes, rewrite the report format before
  publishing artifacts.
