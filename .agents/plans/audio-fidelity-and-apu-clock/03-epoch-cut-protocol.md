# Epoch Cut Protocol (Operator Decision + Re-Baseline Bill)

Track B (and Track A only if its byte-compare gate fails) changes
deterministic emulation behavior. This file is the bill to present and the
rollout order once accepted. **The decision belongs to the operator/program
owner, not an agent** (`docs/emulator-performance-profile.md:91-98`).

## What survives unchanged (verified by consultant sweep)

All committed gates are run-vs-run or hash static data — none pin an
emulation-output hash:
xtask determinism tests (600f/10k), refwork-verify double-run and vm-suite,
the harness mock_agent fixture (its only pinned hash is of static ROM
bytes), CI/nightly determinism + cross-arch compares (both legs rebuilt),
refwork-hash, feature-maps/scoring (addresses + predicates, not values),
phase4 bundle checks (bundle-internal), `ramdiff search` over existing
dumps (never re-runs the emulator).

Only committed emulation-derived expectation found in-repo:
`refwork-verify/tests/integration.rs:123-128,154-159` pins
`frame_ctr == 59` at frame 60 for the synthetic ROM — likely APU-independent
(NMI-driven); verify, update if needed.

## What breaks (the bill)

1. **Pre-fix ramdiff sessions' resume verification** — recorded WRAM dumps
   diverge from replay under the new build; `verify_checkpoint` fails with
   its documented "emulator behavior has changed" message. Escape hatch:
   `--skip-replay-verify`; real remedy: migrate (below). Note: `session.yaml`
   has no emulator-version field — divergence is the only signal (step 10).
2. **The m6 16/16 byte-exact replay gate and downstream artifacts** —
   discovery-01's dumps, the 1,005-capture cadence-45 frozen corpus and
   its corpus id, the gate-3 labeled trajectory and its scorer evaluation
   record, the package-05 map-check 23/23 progression evidence, and the
   state-scorer / input-synthesizer handoffs keyed to them. The padlog
   itself remains replayable; the 11 discovered feature *offsets*
   (addresses) very likely survive.
3. **Operator lab expect files** (not in repo): vm-first-room framebuffer
   checkpoint hashes, map-check `--expect` frame-pinned values.
4. **Sibling-program icount assets** — absolute-icount epoch chains,
   icount-addressed snapshots, vns-budget runs, calibrated cap fixtures
   (profile.md:85-88; APU catch-up is the named icount-changing row).
   NOTE: this item applies to **Track A as well** even on a passing
   state-epoch gate — rewriting the DSP inner loops changes host retired
   instructions/frame. Also in this class: the staged `--hard-icount-cap`
   proposal in the m6 RESUME-RUNBOOK, computed from pre-fix m4/m5
   icount/frame evidence — both tracks invalidate its inputs.
5. **Package-06 determinism green stamp**
   (snes-rom-black-screen-compat/00-plan.md:171-177) — re-baseline before
   any image registration.
6. **Docs**: recorded hashes/instruction counts in
   docs/emulator-performance-profile-data/*-20260711.md become
   epoch-stamped history.

## Rejected alternatives (assessed, do not revisit without new facts)

- **Feature/config-gating the fix**: two co-existing behaviors = bifurcated
  hash universes with no version labeling anywhere in the artifact formats;
  CI would test one universe while operators record in the other. Poison.
- **Host-side audio mitigation without the clock fix**: the surplus is
  traffic-dependent (+2% quiet frames, +132% handshake frames); a fixed
  playback rate is never right, an adaptive one wanders pitch, time-stretch
  adds artifacts/latency, trims are the current "terrible". Structurally
  unfixable host-side.
- **Deferral**: every new recording (discovery-02 next) joins the
  to-invalidate pile; the fix is icount-changing whenever it lands.

## While the decision is pending

- **Recording freeze starts NOW, not at acceptance**: any session that
  proposes recording discovery-02 (the next m6 operator input per the
  RESUME-RUNBOOK) must first surface this pending decision — every new
  recording joins the to-invalidate pile. Decision latency is on the m6
  critical path.
- **The operator ask** follows the batched-verbatim style of
  `close-m6-entry-gates/02-operator-launch-decision.md`: present this file
  verbatim, record the answer in `.agents/decisions/` as a dated file
  (e.g. `2026-07-XX-apu-clock-epoch-cut.md`), and comment the Track B bead
  with the outcome. profile.md:97 says "those owners" (plural): the
  decision must record acceptance on behalf of the sibling-program owners
  (hypervisor/snapshot-store/scorer), not just this repo.

## Rollout order (once accepted)

1. Operator records the decision in `.agents/decisions/` (precedent:
   2026-07-02 kernel-agent-artifact-split) — epoch cut accepted, owner
   assigned.
2. Recording freeze (already in force per the pending clause) confirmed
   until the fix lands.
3. Land Track B (02) — fix + flipped tests + `EMU_VERSION` bump in one
   commit; Track A rides the same epoch if its gate failed.
4. Run all self-consistent gates (02 acceptance 2), incl. the ignored 10k
   determinism gate and a 100k double-run; verify/fix integration.rs
   frame_ctr pin.
5. Re-stamp package-06.
6. Migrate discovery-01: replay the padlog under the new build; regenerate
   the 16 dumps; validate feature semantics via map-check progression (NOT
   byte-compare against old dumps); re-freeze under a NEW corpus id. If the
   input log semantically derails under new timing, operator hand
   re-records.
7. Operator regenerates lab expect files (vm-expect dry-run record mode,
   then re-pin; map-check --expect regeneration).
8. Sibling program re-baselines icount assets (hypervisor epoch/cap
   fixtures; snapshot-store versions/rejects pre-epoch snapshots).
9. Update stale docs with an epoch-cut note, and every doc/bead/handoff
   record citing the OLD corpus id (state-scorer handoff, 04-resolution,
   GATE-RECORD files) to reference the new id from step 6.
10. Follow-up bead: add an emulator-version/epoch field to ramdiff
    `session.yaml` so the NEXT behavior change fails with a version message
    instead of a bare WRAM divergence.
