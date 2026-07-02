# 05 - Capture Export And Validation

**Purpose:** add or drive the reference-workload-owned tooling that proves the
Phase 4 context and scorer bundles are valid.

This package has two parts:

- **05A export tooling:** implement against synthetic fixtures before real
  captures exist.
- **05B bundle validation:** run against the lab-private context/scorer bundles
  after packages 03 and 04 have concrete opaque private artifact ids.

## Tooling Direction

Prefer adding subcommands to `refwork-verify` because it already owns
`play`, `map-check`, and `double-run`.

Suggested commands:

```sh
refwork-verify trace \
  --captures <private-capture-index> \
  --map feature-map.yaml \
  --scoring scoring-program.yaml \
  --labels <operator-labels.yaml> \
  --out <lab-private-output>/trajectory/first-boss.jsonl \
  --report <lab-private-output>/validation/trace-report.json

refwork-verify phase4-bundle-check \
  --bundle <private-bundle-dir> \
  --report <lab-private-output>/validation/phase4-bundle-check.json
```

If final names differ, record the exact names in package 06 closeout notes and in
the request fulfillment note.

## Implementation Steps

1. Extend or wrap existing decode helpers.

   Current useful code:

   - `crates/refwork-verify/src/decode.rs`
   - `crates/refwork-verify/src/map_check.rs`
   - `crates/refwork-verify/src/play.rs`
   - `crates/refwork-featuremap/src/lib.rs`

   Reuse these rather than creating an independent feature decoder.

2. Implement `trace` or equivalent trajectory emitter:

   - load feature map and scoring program;
   - read capture index or platform capture export;
   - decode feature values in map order;
   - join operator labels by frame/capture id;
   - emit JSONL trajectory records only under a lab-private output root;
   - emit a validation report with input hashes, output hashes, command, and
     pass/fail status.

3. Implement or script `phase4-bundle-check`:

   - verify required top-level files exist;
   - verify `manifest.json` has artifact schema version, reference-workload
     commit, workload image identity/revision/private artifact id, operator ROM
     metadata policy, feature-map hash, scoring-program hash, WorkloadImage
     manifest hash, compiled layout hash or evidence ref, capture count,
     framebuffer format metadata, private storage artifact id, role-based access
     requirement, retention expectation, compression format, max expected size,
     clean-room note, and image validation stamp;
   - verify `feature-map.yaml` and `scoring-program.yaml` parse and validate
     together;
   - verify `feature-map.yaml` has no placeholder offsets and marks stable
     canonical-hash fields with `stability: stable`;
   - verify `layout.json.total_len` equals every `feature_bytes` length;
   - verify each capture row has decoded features and framebuffer metadata;
   - verify primary capture count is at least 1,000;
   - verify dedup labels cover same and distinct cases;
   - verify score plan has K=32 deterministic batches and fixed client batch ids;
   - verify trajectory has first-boss, goal-positive, and goal-negative labels;
   - scan public output/evidence locations for raw bytes, decoded real feature
     vectors, trajectory JSONL, operator labels, padlog tails, private capture
     ids, exact private paths, screenshots, save RAM, ROM bytes, or forbidden
     private blobs before closeout.

4. Keep test fixtures synthetic in the source repo.

   Add unit/integration tests that use tiny synthetic feature maps and captures.
   They should prove command behavior and schema checks without shipping real
   game-derived data.

5. Preserve existing validation gates:

   ```sh
   cargo test --locked -p refwork-featuremap
   cargo run --locked -p refwork-featuremap -- validate feature-map.yaml --scoring scoring-program.yaml
   cargo run --locked -p refwork-verify -- map-check --rom <operator-private-rom> --map feature-map.yaml --script <operator-private-script.padlog> --expect <lab-private-output>/validation/map-check.expect.yaml
   ```

   The real `map-check` command is lab-only because it uses operator-private ROM
   and script artifacts.

## Bundle Validation Report

The lab-private `validation/phase4-bundle-check.json` should include:

- schema version;
- command and arguments;
- reference-workload commit;
- bundle root and opaque artifact id;
- top-level file hashes;
- capture count;
- decoded feature count per capture;
- framebuffer metadata summary;
- dedup group counts;
- score plan batch ids;
- trajectory file hashes and coverage summary;
- pass/fail status and errors.

The source repo or broadly visible request notes may include only a sanitized
summary of this report: report hash, opaque artifact id, counts, pass/fail
status, role-based access requirement, and redacted command template.

## Acceptance

- Export/check tooling has synthetic tests in repo CI.
- Lab validation commands produce private JSON reports with hashes and command
  lines, plus sanitized summaries suitable for source/request notes.
- The real bundle passes all reference-workload-owned checks before handoff.
- The validation report is sufficient for state-scorer to reproduce its own
  downstream smoke without asking reference-workload for ad hoc context.

## Stop Conditions

- If the export tool has to know state-scorer archive internals, stop. The
  scorer owns scoring, hashing, archive, and latency behavior.
- If the export path decodes features in any order other than feature-map order,
  stop and fix the exporter.
- If any report writes raw captured bytes, decoded real feature vectors,
  framebuffer images, operator labels, trajectory JSONL, padlog tails, private
  capture ids, exact private paths, or ROM metadata into a public repo path,
  rewrite the report format before publishing.
