# 05 - Bundle Validation And Freeze

## Purpose

Assemble the contract-complete private bundle, prove internal integrity, create
sanitized public evidence, and freeze an immutable version.

## Assembly

For the full path, ensure the private bundle contains at least:

```text
manifest.json
workload-image.yaml | workload-image-ref.txt
feature-map.yaml
scoring-program.yaml
layout.json
captures/index.jsonl
artifacts/feature-bytes/...
artifacts/framebuffer/...
dedup-groups.jsonl
score-plan.json
trajectory/first-boss.jsonl
validation/...
```

Populate `manifest.json` with every field currently enforced by
`phase4-bundle-check`: exact reference-workload commit; image identity,
revision, private ref, manifest hash, and validation stamp; pad layout
`console16-12btn-v1`; operator metadata policy; feature/scoring/layout hashes;
capture count; framebuffer format; opaque private storage id; access
requirement; retention; compression format and maximum expected size; clean-room
provenance; and the canonical non-self-referential `bundle_checksum` defined in
package 02. The external freeze-manifest hash belongs in fulfillment/version
records after finalization, not in a field that the same manifest hashes.

## Validation Order

1. Run feature-map/scoring validation and real map-check evidence from package
   03 again against the final files.
2. Run `phase4-artifact-check` and store its final report in `validation/`.
3. Produce the final public-note drafts, run redaction scanning, and place the
   final redaction report in `validation/`. Do not edit those drafts or the
   private report afterward without restarting finalization.
4. Ensure `validation/` contains passing, machine-readable evidence whose
   filenames cover WorkloadImage, feature/scoring, map/region layout, trace,
   checksum, and redaction categories expected by the checker.
5. Generate the required internal checksum evidence using the canonical payload
   root contract. Because that root excludes `validation/` and normalizes its
   own manifest field, it is stable and non-self-referential. Then run the
   bundle checker with its report outside the bundle:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-bundle-check \
     --bundle "$BUNDLE" \
     --report "$PRIVATE_SEAL_DIR/phase4-bundle-check.json"
   ```

6. After the final checker report exists, do not write or overwrite another file
   in the bundle. Generate the external freeze manifest last:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-checksum-manifest \
     --bundle "$BUNDLE" \
     --out "$PRIVATE_SEAL_DIR/freeze-manifest.json"
   ```

   This recursively covers artifacts and uses the normalized manifest checksum
   contract from package 02; it excludes only the external seal itself.

7. Build the private forbidden-literal list from ROM/game identifiers, private
   paths/refs, capture ids, token/owner secrets, and other operator-designated
   literals. Scan every intended public file again without writing into the
   frozen bundle; retain this post-freeze report beside the external seal:

   ```sh
   cargo run --locked -p refwork-verify -- redaction-scan \
     --input "$PUBLIC_DRAFT" \
     --report "$PRIVATE_SEAL_DIR/post-freeze-redaction-scan-report.json" \
     --forbid-file "$PRIVATE_FORBIDDEN_LITERALS"
   ```

   Repeat after the files reach their final public locations.

8. Verify without mutation:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-checksum-manifest \
     --verify "$PRIVATE_SEAL_DIR/freeze-manifest.json" \
     --bundle "$BUNDLE"
   ```

   Confirm all file/artifact hashes, capture count, label coverage, and opaque
   bundle id match. Never rerun an in-bundle report after the seal is generated.

## Freeze Protocol

- Assign an opaque, versioned corpus id only after validation passes.
- Package/upload to approved private storage and verify retrieval into a fresh
  private directory.
- Run non-mutating freeze-manifest verification, `phase4-artifact-check`, and
  `phase4-bundle-check` on the retrieved copy with reports written outside the
  retrieved bundle.
- Set storage immutability/read-only controls where available.
- Write a sanitized in-repo version record containing only approved fields:
  opaque id, checksum-manifest hash, capture count, categorical label coverage,
  source commit, validation report hashes/status, access role (not secrets),
  retention expectation, and the immutable/new-version rule.
- Use the identical corpus id in the scorer fulfillment, context fulfillment,
  bundle manifest, downstream smoke handoffs, and request resolution.

Any later change to a map, scoring program, layout, capture, label, artifact,
manifest, or validation evidence requires a new id and complete revalidation.

## Exit Criteria

- Full bundle passes from a fresh retrieved copy.
- Checksums and top-level hashes match in all records.
- Public drafts and final public files pass redaction scan.
- Frozen version record is durable and contains no private payload or secret.
- Operator metadata disposition is accurately reflected.
