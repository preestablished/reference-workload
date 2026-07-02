# Phase 4 Requests Implementation Evidence

**Date:** 2026-06-23 UTC

## Source Work Completed

- Added `pad_layout.layout_id: console16-12btn-v1` to generated WorkloadImage
  manifests in `xtask/src/image.rs`.
- Extended WorkloadImage validation to reject missing or wrong layout id, wrong
  mixed-case button names, wrong bits, and wrong reserved bits.
- Updated `.padlog` format docs to name layout id `console16-12btn-v1`, layout
  version `1`, exact mixed-case names, and reserved-bit parse behavior.
- Updated determinism reference-workload API/integration docs to include
  `pad_layout.layout_id` and exact-name consumer behavior.
- Added `refwork-verify trace` for decoded capture indexes plus operator labels.
  It validates feature order against the feature map, evaluates the scoring
  program, and emits trajectory JSONL plus a private report.
- Added `refwork-verify phase4-context-check` for the context-smoke fixture
  contract. It validates manifest provenance, `console16-12btn-v1`, decoded
  feature shape, framebuffer metadata hashes, region metadata, optional
  `.padlog` syntax, recent-input metadata, and validation report status.
- Added `refwork-verify phase4-bundle-check` for the private scorer bundle
  contract. It validates manifest shape, feature-map/scoring pairing, layout
  evidence, capture metadata, framebuffer metadata, dedup labels, K=32 score
  plan batches, trajectory label coverage, concrete validation report files,
  map-check evidence, feature-map/scoring validation evidence, and absence of
  inline raw capture/framebuffer payload fields in capture rows. It also
  enforces decoded feature-map order and validates capture-id references across
  dedup groups, score plans, and trajectories.
- Added synthetic tests for the Phase 4 checker and trace emitter. No real ROM,
  framebuffer, save RAM, raw capture, private capture id, operator label, or
  decoded real feature vector is committed.

## Validation Run

```sh
cargo test --locked -p xtask
cargo test --offline -p refwork-verify phase4 -- --nocapture
cargo test --locked -p refwork-verify phase4 -- --nocapture
cargo test --locked -p refwork-featuremap
cargo test --locked -p refwork-verify
cargo fmt --all -- --check
git diff --check
```

The Phase 4-focused verifier run passed 8 tests under both the earlier offline
probe and the locked Cargo probe. `xtask` and `refwork-featuremap` locked test
runs also passed after the lock sync. `Cargo.lock` is synced to the clean
`../control-plane` commit `2a97392` so this repo records the committed
`determinism-proto` prost/tonic/protoc dependency closure. The earlier full
`refwork-verify` locked run passed 11 tests in 525.78s.

## Request Notes Written

- `/home/infra-admin/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`
- `/home/infra-admin/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`
- `/home/infra-admin/.agents/projects/exploration-orchestrator/requests/input-synth-v1-client-context/REFERENCE-WORKLOAD-NOTE.md`

These notes are sanitized. They record source/tooling progress without claiming
that live context fixtures or scorer golden artifacts exist.

No request directories were present under
`/home/infra-admin/.agents/projects/input-synthesizer/requests/` or
`/home/infra-admin/.agents/projects/state-scorer/requests/` at implementation
time.

## Remaining Lab-Private Work

The Real Capture Evidence Floor is not met in this worktree. The following
remain unfulfilled:

- live Phase 4 context fixture artifact id or private bundle;
- private context-smoke bundle passing `phase4-context-check`;
- recent pad tail availability;
- validated private scorer golden bundle;
- at least 1,000 real in-VM captures with framebuffer metadata;
- real operator-approved feature map/scoring program, or private hashes/refs for
  them;
- private `trace-report.json` and `phase4-bundle-check.json` from lab artifacts;
- downstream input-synthesizer/state-scorer/orchestrator smoke evidence.

The existing `dist/` bundle is untracked generated output and was not updated,
per package 01 guidance.
