# Step 03 — Rebuild Package-04 Against New guest-sdk; Regenerate READY

Goal: a WorkloadImage whose in-guest stack uses the **real** guest-sdk
registration path, and a new READY snapshot built from it, replacing the
staged-fixture snapshot the deployed runtime currently serves.

## Why A Rebuild Is Mandatory, Not Optional

- The checked-in `dist/workload-image-0.1.0/` manifest embeds
  `meta.built_from.git_rev = 38fa190…` — a rev that predates the real
  `register_region` path. A workload built against the old guest-sdk either
  carries the fake-handle semantics or (if partially rebuilt) fails at the
  new `AgentUnavailable`/IPC boundary. The audit itself flagged this: "If
  the lab run consumes current HEAD, rebuild the image and record the new
  manifest BLAKE3."
- The harness must hold its region handles for process lifetime now
  (drop ⇒ manifest DEAD ⇒ host reads fail). Audit the harness's handle
  ownership before building.

## Work

1. Bump the guest-sdk dependency/rev used by the image pipeline to
   guest-sdk `main` (`604cd41` or later), rebuild
   (`cargo run --locked -p xtask -- image validate|register …` per the
   existing package-04 flow, documented in
   `../guest-sdk-unblock-reference-workload/04-image-handoff-assets.md`),
   prove reproducibility with `xtask image double-build`, and record new
   artifact BLAKE3s.
2. Confirm the workload publishes `framebuffer` at exactly 229,376 bytes
   under `layout_version 1` (it should already — `FB_BYTES` — but the
   check is now enforced by the hypervisor, so make it an explicit image
   test rather than an assumption).
3. Regenerate the READY snapshot via the hypervisor M9 handoff flow
   (`~/git/preestablished/determinism-hypervisor/docs/ops/rom-bridge-o73-ready-snapshot.md`),
   **coordinating on the in-flight `m9_handoff.rs` edits noted in
   `01-…`**. Record: snapshot ref, machine config hash, state hash,
   ready icount, region count/manifest generation — same fields the M9
   evidence recorded.
4. Update the private handoff env channel
   (`BRIDGE_REAL_SNAPSHOT_REF` in the operator-private env file — location
   documented in the ops doc above; never commit the value) so the bridge
   restores the new snapshot.

## Exit Criteria

- New image manifest BLAKE3 recorded; double-build reproducibility shown.
- New READY snapshot in the snapstore with recorded hashes.
- `GetFramebuffer` against a freshly restored slot returns **229,376
  bytes, XRGB8888 256×224** — a black frame is acceptable and expected
  pre-first-render. (The bridge side will independently confirm this the
  moment the env channel is updated; see `06-…`.)
