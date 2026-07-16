# 04 - Operator Capture And Labeling

## Purpose

Run the approved private session once, produce the primary capture corpus, and
derive source-owned labels and deterministic consumer aids.

## Full-Corpus Procedure

1. At session start, re-confirm the operator checklist, deployed worker build,
   READY snapshot/image refs, map/layout hashes, exporter commit, storage free
   space, and private-root permissions. Record ROM BLAKE3 and run owner only in
   approved private evidence.

2. Record one coherent hand-play padlog/trajectory that includes:

   - start area and leaving-start-area transition;
   - first upgrade;
   - first boss and post-boss evidence;
   - ordinary states that must not satisfy the goal;
   - a credits or late-game state that must satisfy the goal.

   If the late-game fixture must come from a separately approved snapshot or
   segment, record that source explicitly and preserve the same image, ROM,
   map, scoring, and layout identities.

3. Run the exporter with framebuffer capture enabled for every primary row.
   Capture at least 1,000 real states. Choose cadence to cover transitions and
   stable/volatile comparisons rather than collecting 1,000 nearly identical
   frames. Retain bounded-run caps whenever worker provenance does not prove
   the OOM fix.

4. Immediately validate the draft rows and artifacts:

   - unique capture ids and monotonic/source-consistent frame provenance;
   - identical layout hash in every row;
   - feature length equals `layout.total_len`;
   - every framebuffer decompresses to 229,376 bytes and matches its hash;
   - decoded order equals feature-map order;
   - minimum count is satisfied.

   Make this durable rather than a manual assertion:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-artifact-check \
     --bundle "$BUNDLE" \
     --report "$BUNDLE/validation/artifact-check.json"
   ```

5. Create private `dedup-groups.jsonl` with multiple examples of both:

   - `same_canonical_state`, where only named volatile features/ranges change;
   - `distinct_stable_state`, where at least one named stable feature changes.

   Do not precompute or assert state-scorer hash values.

6. Create private operator labels joined by capture id. Include staged anchors,
   expected highest stage, prune truth, goal truth, and first-boss coverage.
   Have the operator or domain owner review the selected mandatory examples.

7. Generate the K=32 deterministic score plan:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-score-plan \
     --captures "$BUNDLE/captures/index.jsonl" \
     --out "$BUNDLE/score-plan.json" \
     --first-boss "$FIRST_BOSS_CAPTURE_ID" \
     --goal-positive "$GOAL_POSITIVE_CAPTURE_ID" \
     --goal-negative "$GOAL_NEGATIVE_CAPTURE_ID"
   ```

   Ensure fixed unique `client_batch_id` values, checkpoint and restore/control
   batch references, and coverage of all three mandatory labels.

8. Emit the trajectory and private trace report:

   ```sh
   cargo run --locked -p refwork-verify -- trace \
     --captures "$BUNDLE/captures/index.jsonl" \
     --map "$BUNDLE/feature-map.yaml" \
     --scoring "$BUNDLE/scoring-program.yaml" \
     --labels "$PRIVATE_LABELS" \
     --out "$BUNDLE/trajectory/first-boss.jsonl" \
     --report "$BUNDLE/validation/trace-report.json"
   ```

   Confirm the trajectory reaches the first boss and contains both goal truth
   values, including the late-game positive.

## Explicit Fallback Branch

Use this only after package 01 records approval, then follow
`04a-first-room-fallback.md`. Do not fabricate first-boss or goal labels and do
not force limited data through the full validator by adding false data.

## Exit Criteria

- Full path: at least 1,000 primary real captures with framebuffers; first-boss,
  goal-positive, and goal-negative labels; staged anchors; both dedup relation
  types; K=32 plan; passing trace report.
- Fallback path: explicitly approved, honestly scoped frozen corpus and durable
  follow-on, with no claim of full scorer fulfillment.
- All private artifacts remain outside git and under approved access controls.
