# Verification (rom-operator-bridge side, 2026-07-03)

Three independent tracks: a code-claims review against commits
`5c11911..6b78fb0`, an evidence audit with full reproduction, and a
provenance deep-check with the operator.

## Verdict: Confirmed. The engineering claims are real and honestly caveated.

- **Reproduced exactly:** `cargo test --workspace --locked` → 452 passed /
  0 failed (1 standing `#[ignore]` slow test); clippy clean;
  `xtask image double-build` → OK, byte-identical roots, bzImage and
  initramfs BLAKE3s matching the evidence table exactly. The
  `workload-image.yaml` hash differs at HEAD **solely** via the embedded
  `meta.built_from.git_rev` — substituting `2a8c68a` reproduces the
  recorded `53a94695…` exactly. Reproducibility holds as designed.
- **Code claims verified in source:** `refwork-dh-client` is a real
  tonic/prost/UDS client; `vm-first-room` injects input only via
  `InjectInputs`, proves transitions via host region reads, reports
  hashes never pixels; the `vm-suite` is genuinely in-VM and distinct
  from the host-side `double-run`, with a working negative test; the
  harness registers regions through the real `register_region` before
  `Ready` with deliberate handle leaks and distinct
  standalone-degrade/hard-fault branches; both lock files gate the build
  with live refuse-on-mismatch code; `boot.toml` matches the agent's
  real parser field-for-field. Staged-fixture legs (6 + 3 tests) run in
  every plain workspace test, not env-gated.
- **Kernel provenance chain closed end-to-end** (initially flagged by a
  reviewer, then disproven with the operator): the reviewer searched
  guest-sdk's *committed* content, but the kernel is a deterministic
  build **output** — `b3sum ../guest-sdk/image/build/bzImage` =
  `595466…` matching `image/kernel.lock` exactly, and
  `../guest-sdk/image/build/kernel.provenance` carries the same
  `build_key` (`16d7cbc8…`) derived from the pinned tarball/config/
  patches/build-script hashes. The hypervisor M9 table carrying the same
  bzImage hash is consistent (M9 booted the same guest-sdk-built kernel),
  not contradictory.

## One Actionable Blocker

- **`image/guest-sdk.lock` pins rev `c03e90b`, which is unpushed** in the
  guest-sdk checkout (`origin/main` = `604cd41`). Local builds pass the
  rev check; any fresh clone/CI build refuses until guest-sdk `main` is
  pushed. Operator decision pending on pushing these repos.

## Minor Notes

- The evidence note's "runner label needs an operator decision" is stale:
  `e08e522` locked `vm-gates.yaml` to `[self-hosted, intel, kvm]`
  (operator-confirmed) after that section was written. Reality is ahead
  of the note.
- "wired into clap definitions" — the verifier uses a hand-rolled arg
  parser, not clap. Cosmetic.

## Known Gaps (Disclosed, Worth Follow-Up Beads)

1. The live (non-mock) worker gRPC path in `vm-first-room`/`vm-suite` has
   zero automated coverage — exercised next by the coordinated boot/READY
   step.
2. No negative tests for: the harness `RegionRegFailed` hard-fault-before-
   Ready branch, either lock-file mismatch-refusal branch, or the
   restore-continuity leg (the nondet negative test covers double-run
   only).
3. Capture alarms share the generic `"run"` failure stage in reports
   rather than a distinct name.
4. `refwork-d7t.11`'s bead acceptance criteria (Ready-beacon comparison,
   READY-under-2s timing) go further than the implementation; bead
   correctly remains open.

## What Remains (Operator-Coordinated, As The Evidence Note States)

Boot the image under a locally-launched worker → regenerate the READY
snapshot via the M9 handoff (hypervisor tree had in-flight `m9_handoff.rs`
edits; coordinate) → `BRIDGE_REAL_SNAPSHOT_REF` cutover with the bridge
side → bridge runs `RestoreSnapshot → GetFramebuffer → browser preview`
(our standing offer, unchanged) → operator lab-run fields for the
first-room gate (ROM BLAKE3, padlog BLAKE3, run owner).

## Addendum 2026-07-03 — Follow-Ups Executed

The `phase3-followups-closeout` plan (rom-operator-bridge repo) resolved
this note's actionable blocker and known gaps:

- **Pushes landed:** guest-sdk `c03e90b`, determinism-hypervisor
  `4c44263`, reference-workload `0a9726c` all reached `origin/main` —
  the `image/guest-sdk.lock` rev check is now satisfiable from a fresh
  clone.
- **Gaps closed** (each new test shown to fail with its guard
  reverted): Gap A `fe91261` (hard-fault-before-Ready seam + tests),
  Gap B `209b241` (both lock mismatch-refusal branches, parameterized
  cores), Gap C `ef59c73` (restore-continuity negative via mock
  post-restore divergence). Gap D (capture-alarm stage naming) was
  deliberately not implemented — it needs an error-taxonomy decision
  (structured error field vs fragile string matching); disposition
  recorded in bead `refwork-otv`.
- **Live-worker smoke** `d61e300` (bead `refwork-asn`): the previously
  untested live gRPC path now has transport/codec/error-mapping
  coverage against a real scratch `dh-workerd` (`--no-snapstore`),
  green locally (bogus-ref class observed: FailedPrecondition), wired
  into `vm-gates.yaml` with CI building the worker from the sibling
  checkout. The full vm-first-room/vm-suite real-worker legs still wait
  on the coordinated step-03 boot/READY sequence and will reuse this
  gate and launch recipe.
- The stale runner-label wording in the M4 evidence note got its
  addendum (`bae3bed`).

Remaining, unchanged: the operator-coordinated boot → READY snapshot →
`BRIDGE_REAL_SNAPSHOT_REF` cutover → bridge browser verification, and
the operator lab-run fields.
