# 03 - Private Feature Map, Layout, And Scoring

## Purpose

Replace placeholder offsets with an operator-ROM-specific private feature map
and scoring program, and prove that the resulting layout decodes the real
registered region layout.

## Steps

1. Work only beneath the approved private root. Seed from the schema and
   checked-in examples for structure, never for offsets or golden evidence.
   Create:

   - `feature-map.yaml` with actual registered region names/sizes, real offsets,
     feature types, semantics, `stability`, and count-novelty discretization;
   - `scoring-program.yaml` with staged milestones for leaving the start area,
     first upgrade, first boss, goal predicate, and prune rules;
   - `workload-image.yaml` or an opaque `workload-image-ref.txt` with image,
     region/framebuffer, pad layout, and green-stamp provenance.

2. Discover offsets through approved private `ramdiff`, `map-check`, early real
   captures, and controlled hand-play comparisons. Include at least the fields
   needed to distinguish the staged trajectory and goal predicate. Validate
   stability claims with repeated/restored states; do not mark volatile fields
   stable merely because one trace did not change.

3. Validate the map/scoring pair:

   ```sh
   cargo run --locked -p refwork-featuremap -- validate \
     "$BUNDLE/feature-map.yaml" \
     --scoring "$BUNDLE/scoring-program.yaml"
   ```

   Save a private machine-readable pass report with a generic filename that
   `phase4-bundle-check` recognizes as feature/scoring validation evidence.

4. Run the real `map-check` (or its documented successor) against the approved
   ROM and operator script. Store expectations and report privately:

   ```sh
   cargo run --locked -p refwork-verify -- map-check \
     --rom "$PRIVATE_ROM" \
     --map "$BUNDLE/feature-map.yaml" \
     --script "$PRIVATE_SCRIPT" \
     --expect "$BUNDLE/validation/map-check.expect.yaml"
   ```

   Ensure the durable report proves the real region layout and feature changes,
   not merely schema validity. If `map-check` does not emit a suitable report,
   add a non-secret report mode before claiming this gate.

5. Generate the definitive layout with the actual exporter commit and a hash
   or opaque ref identifying the exact compiled `CaptureSpec` contract:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-layout \
     --map "$BUNDLE/feature-map.yaml" \
     --out "$BUNDLE/layout.json" \
     --capture-spec-hash "$CAPTURE_SPEC_HASH_OR_REF" \
     --layout-version 1 \
     --compiler-or-exporter-commit "$EXPORTER_COMMIT"
   ```

6. Independently review the generated `layout.json`:

   - every range addresses a registered region and is in bounds;
   - range order equals feature-map order;
   - `total_len` equals the sum of range lengths;
   - map hash, capture-spec hash, exporter commit, and layout hash are present;
   - no checked-in demo map hash or placeholder offset is present.

7. Re-run a small private capture probe through the exporter and compare
   decoded values with direct `ReadGuestMemory` reads for the same ranges where
   practical. This is a consumer-side wiring cross-check, not a reinvention of
   the already-completed engine proof.

## Exit Criteria

- Private map/scoring validation passes.
- Real map-check/region-layout evidence passes and is stored under
  `validation/`.
- `layout.json` is version 1, nonempty, tied to the real map and exporter
  commit, and accepted by the exporter.
- Stable/volatile classifications support both dedup relation types.
- No private offset, decoded vector, ROM/script identifier, or goal label is
  added to the repository.

## Stop Conditions

- Discovery only supports placeholder or ambiguous offsets.
- Registered region sizes/layout versions disagree with the image record.
- The scoring program cannot express the mandatory stages and final goal.
- Map/layout changes after production capture has begun. In that case discard
  or version the incompatible draft rows and restart; never relabel them under
  the new layout hash.
