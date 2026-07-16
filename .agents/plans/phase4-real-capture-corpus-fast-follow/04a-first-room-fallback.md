# 04a - First-Room-Only Fallback

## Purpose And Authority

This branch is executable only when package 01 records explicit operator/epic
owner approval because the hand-play trajectory cannot be scheduled. It
produces useful decode/dedup/context evidence without claiming the full Phase 4
scorer gate.

## Typed Contract

Implement and document `phase4-fallback-check` (or an equivalent explicit
`--bundle-kind first-room-fallback-v1` mode) before producing the fallback. The
manifest must use:

- `kind: phase4-first-room-fallback`;
- `schema_version: 1` and `scope_version: first-room-v1`;
- exact positive `capture_count` with no full-corpus minimum claim;
- image/map/scoring/layout/exporter and private storage provenance identical in
  quality to the full bundle;
- `coverage.first_room: true`, `coverage.decode_goldens: true`, and truthful
  dedup/stable/volatile flags;
- `coverage.first_boss: false`, `coverage.goal_positive: false`, and
  `coverage.trajectory: false`;
- a follow-on task id and owner role for the missing full corpus;
- `fulfillment_claim: partial`.

Required files are `manifest.json`, workload image ref, real feature map,
scoring program, layout, capture index and artifacts, dedup groups, and
validation evidence. `score-plan.json` and trajectory files are absent unless
they contain only truthful first-room consumer aids; their presence must never
satisfy full mandatory-label checks.

The fallback validator must reject:

- manifests typed as the full scorer bundle;
- zero captures, missing framebuffers, missing/corrupt artifacts, placeholder
  maps, or absent same-canonical/distinct-stable evidence;
- any true first-boss/goal/trajectory claim without corresponding approved
  evidence;
- a `fulfilled` scorer claim;
- missing follow-on task/owner.

Keep the existing `phase4-bundle-check` full path unchanged. Add synthetic
positive and negative tests for this separation.

## Production And Validation

1. Export approved first-room captures against the same real image, worker
   safety rules, map/layout, frame-coherence, and framebuffer requirements as
   the primary exporter.
2. Create real decode goldens and both dedup relation types, including named
   volatile-only and stable-change examples.
3. Run and retain reports for real map/layout validation, export,
   `phase4-artifact-check`, `phase4-fallback-check`, and redaction scanning.
4. Derive a live context fixture only if it truthfully meets the context
   contract; type it as sourced from the fallback corpus and do not imply
   first-boss/goal coverage.
5. Use the same canonical freeze-manifest generation and non-mutating
   verification protocol as package 05.
6. Retrieve into a fresh private directory and run:

   ```sh
   refwork-verify phase4-checksum-manifest --verify "$FREEZE_MANIFEST" --bundle "$RETRIEVED_BUNDLE"
   refwork-verify phase4-artifact-check --bundle "$RETRIEVED_BUNDLE" --report "$PRIVATE_EXTERNAL_REPORT"
   refwork-verify phase4-fallback-check --bundle "$RETRIEVED_BUNDLE" --report "$PRIVATE_EXTERNAL_REPORT_2"
   ```

   Use the final CLI spelling implemented in package 02 and record it in the
   resolution.

## Handoff Status

- Scorer golden-artifact fulfillment remains **partially fulfilled** and names
  the follow-on task.
- Pad/context fulfillment may become fulfilled only if the independently
  validated live context acceptance is met; otherwise it remains partial.
- State-scorer handoff may advertise decode and dedup smoke only. First-boss,
  staged trajectory, goal predicate, and full real-capture gate stay blocked.
- Input-synthesizer handoff may use the live first-room context and pad contract
  if context validation passes.

## Exit Criteria

- Separate fallback schema/validator and tests exist.
- Positive capture count, framebuffer artifacts, decode goldens, and both dedup
  relations pass artifact/fallback/freeze verification after fresh retrieval.
- Fulfillment and handoff claims are partial and consistent.
- Full-corpus follow-on is durable, owned, and open.
