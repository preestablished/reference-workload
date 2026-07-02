# 04 - Scorer Artifact Bundle Contract

**Purpose:** deliver the lab-private artifact bundle requested by
state-scorer for Phase 4 real-capture and first-boss readiness.

This package implements the request in
`/home/infra-admin/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/`.
It should follow that request exactly when there is any conflict with this plan.

## Bundle Location

The raw bundle must live in lab-private storage, not in the source repo. The
source repo may contain only a sanitized evidence note with:

- opaque artifact id or registry ref;
- top-level hashes;
- file counts;
- capture counts;
- role-based access requirement;
- retention expectation;
- redacted validation command templates;
- reference-workload commit.

Put exact direct paths, token-owner names, retrieval steps, and private command
arguments in lab-private storage unless the request owner explicitly marks the
request location private enough for those details.

## Required Private Bundle Shape

```text
manifest.json
workload-image.yaml or workload-image-ref.txt
feature-map.yaml
scoring-program.yaml
layout.json
captures/index.jsonl
dedup-groups.jsonl
score-plan.json
trajectory/<name>.jsonl
validation/
```

`layout.json` and `score-plan.json` are evidence and consumer-aid files. They do
not transfer canonical schema ownership to reference-workload.

## Public Vs Private Map Files

Reference-workload owns the feature map and scoring program, but publication is
an operator decision:

- If the real offsets and scoring labels are approved for source publication,
  update `feature-maps/demo-game.yaml` and `scoring/demo-game.yaml` and pin those
  committed files in the bundle manifest.
- If they are not approved for source publication, keep the repo placeholders
  clearly marked as placeholders and provide private `feature-map.yaml` and
  `scoring-program.yaml` inside the lab-private bundle.

Do not silently replace placeholder files with real private data.

## Manifest Checklist

`manifest.json` must include:

- artifact schema version;
- reference-workload commit hash;
- workload image identity, revision, and opaque private artifact id;
- operator-approved game or ROM revision metadata, or a statement that it is
  available only inside the private bundle;
- feature-map hash;
- scoring-program hash;
- WorkloadImage manifest hash;
- compiled layout hash or layout evidence reference;
- capture count;
- framebuffer format metadata for the primary corpus;
- private storage artifact id, role-based access requirement, retention
  expectation, bundle compression format, and max expected size;
- clean-room/provenance note for operator-supplied artifacts;
- image validation stamp or `determinism.last_green` evidence required by
  reference-workload.

## Implementation Steps

1. Establish artifact root, owner, ACL, retention, and maximum expected size.
   Record these before capture begins.

2. Pin the reference-workload commit, WorkloadImage manifest hash, feature-map
   hash, scoring-program hash, and capture/export tool version.

3. Provide the real feature map:

   - no placeholder offsets;
   - real region names and offsets validated against the Phase 3 in-VM capture
     path;
   - region sizes matching the WorkloadImage manifest;
   - stable features intended for canonical hashing are explicitly marked
     `stability: stable`, while transition/garbage-prone features are marked
     `stability: volatile`;
   - `discretize` hints sufficient for count-based novelty;
   - operator approval before committing real offsets anywhere outside the
     private bundle.

4. Provide the real scoring program:

   - staged milestones through the first-boss path;
   - goal predicate inside the scoring program;
   - prune penalties where needed;
   - no scorer-specific top-level novelty block unless the reference-workload
     API formally adopts that field.

5. Provide `layout.json` describing the exact compiled extraction layout used to
   pack every `feature_bytes` record:

   ```json
   {
     "ranges": [
       { "region": "wram", "layout_version": 1, "offset": 0, "len": 4096 }
     ],
     "total_len": 4096,
     "blake3": "blake3:<hex>",
     "compiled_from_feature_map_hash": "blake3:<hex>",
     "capture_spec_hash": "blake3:<hex>",
     "compiler_or_exporter_commit": "<commit-or-tool-version>"
   }
   ```

6. Provide at least 1,000 real captures satisfying the Real Capture Evidence
   Floor from `00-overview.md`. `captures/index.jsonl` must include, for every
   capture:

   - `capture_id`;
   - `node_ref` or equivalent source id;
   - capture source or RPC/tool name;
   - frame counter or frame index;
   - layout hash;
   - private artifact id, byte length, and BLAKE3 for packed `feature_bytes`;
   - full decoded feature vector in feature-map order;
   - framebuffer private artifact id and metadata: encoding, width, height, stride,
     pixel format, uncompressed length, and BLAKE3.

   Minimal JSONL row example:

   ```json
   {"schema_version":1,"capture_id":"cap-000001","node_ref":"node:root","capture_source":"hypervisor.capture_region","frame_index":120,"layout_hash":"blake3:<hex>","feature_bytes":{"ref":"artifact:<opaque-ref>","len":4096,"blake3":"blake3:<hex>"},"decoded_order":["room_id","area_id","player_x"],"decoded_values":[1,0,144],"framebuffer":{"ref":"artifact:<opaque-ref>","encoding":"fb_lz4","width":256,"height":224,"stride":1024,"pixel_format":"xrgb8888","uncompressed_len":229376,"blake3":"blake3:<hex>"}}
   ```

7. Provide `dedup-groups.jsonl` with labels, not precomputed scorer hashes:

   - same-canonical-state groups where only volatile bytes/features changed;
   - distinct-stable-state pairs where stable features changed;
   - capture ids for every member;
   - changed feature names or offset ranges when known;
   - expected relation.

   Minimal JSONL row example:

   ```json
   {"schema_version":1,"group_id":"dedup-001","expected_relation":"same_canonical_state","capture_ids":["cap-000010","cap-000011"],"changed_features":["frame_counter"],"notes":"volatile-only change"}
   ```

8. Provide `score-plan.json` as the deterministic consumer-aid requested by
   state-scorer. This file gives state-scorer stable smoke-test inputs; it does
   not define scorer archive semantics or transfer ownership of checkpoint,
   restore, hash, or latency behavior.

   - ordered K=32 batch capture ids;
   - fixed `client_batch_id` values;
   - checkpoint point;
   - restore/control comparison batch ids;
   - labels for first-boss, goal-positive, and goal-negative states.

   Minimal example:

   ```json
   {
     "schema_version": 1,
     "batches": [
       { "client_batch_id": "phase4-k32-0001", "capture_ids": ["cap-000001"] }
     ],
     "checkpoint_after_batch": "phase4-k32-0001",
     "restore_control_batch_ids": ["phase4-k32-0002"],
     "labels": {
       "first_boss": ["cap-000900"],
       "goal_positive": ["cap-goal-0001"],
       "goal_negative": ["cap-000901"]
     }
   }
   ```

9. Provide at least one labeled hand-played trajectory in `trajectory/`:

   - one JSONL record per frame;
   - `frame_index`;
   - `capture_id`;
   - decoded feature vector;
   - active stage names;
   - expected highest stage name or index;
   - prune truth label;
   - goal truth label;
   - first-boss coverage;
   - credits or late-game goal-positive fixture;
   - negative examples where the goal predicate must not fire.

   Minimal JSONL row example:

   ```json
   {"schema_version":1,"frame_index":120,"capture_id":"cap-000120","decoded_order":["room_id","area_id","credits_flag"],"decoded_values":[2,1,0],"active_stages":["left_start_area"],"expected_highest_stage":"left_start_area","prune":false,"goal":false,"first_boss_coverage":false}
   ```

10. Add validation reports under `validation/`. At minimum, include
    feature-map/scoring validation and map-check evidence. Package 05 defines
    suggested commands.

## Acceptance

- The bundle has every required top-level file or an explicit opaque private
  artifact id for the file.
- `manifest.json` satisfies the manifest checklist above, including workload
  image identity/revision, private artifact id, operator metadata policy,
  framebuffer format metadata, compression format, max expected size, and image
  validation stamp.
- The primary corpus contains at least 1,000 real in-VM captures with
  framebuffer data and decoded feature vectors.
- `feature-map.yaml` has no placeholder offsets and marks stable canonical-hash
  fields with `stability: stable`.
- `scoring-program.yaml` contains the goal predicate and first-boss staged
  milestones.
- `layout.json.total_len` equals every packed `feature_bytes` length.
- Dedup labels include both same-canonical-state and distinct-stable-state
  cases.
- `score-plan.json` gives deterministic K=32 batches and stable
  `client_batch_id` values as consumer-aid inputs.
- At least one trajectory reaches first boss.
- At least one goal-positive and one goal-negative fixture exist.
- No raw private bytes, screenshots, decoded feature vectors from real captures,
  trajectory JSONL, operator labels, padlog tails, private capture ids, exact
  private paths, or unapproved private identifiers are committed to the source
  repo or public/semi-public request markdown.

## Stop Conditions

- If the Real Capture Evidence Floor is unavailable, do not close this request
  with RAM-only or synthetic captures.
- If the feature map still uses placeholder offsets, do not label the bundle
  "Phase 4 golden".
- If operator approval for private identifiers is unclear, store them only
  inside the private bundle and keep the public evidence opaque.
