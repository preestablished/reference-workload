# 02 - Capture Exporter Implementation

## Purpose

Add the missing producer that compiles `layout.json.ranges` into a hypervisor
`CaptureSpec`, drives sequential frame-boundary captures, and writes the exact
private artifact/index shape already enforced by `phase4-bundle-check`.

## Recommended Source Shape

- Add a `phase4-capture-export` subcommand in
  `crates/refwork-verify/src/main.rs`.
- Add reusable `phase4-artifact-check` and `phase4-context-export` subcommands;
  keep their shared artifact/ref parsing below the CLI layer.
- Extend `phase4-checksum-manifest` with a non-mutating `--verify` mode and a
  canonical freeze-manifest contract described below.
- Put orchestration and row writing in a focused module such as
  `crates/refwork-verify/src/phase4_capture_export.rs`.
- Reuse `refwork-dh-client::WorkerSession`, `proto::CaptureSpec`,
  `proto::ExtractRange`, and `decompress_fb_lz4`; do not duplicate the worker
  protocol or rebuild the engine.
- Reuse the existing feature-map decoder/order logic rather than maintaining a
  second interpretation of feature types.
- Extend the mock worker only as needed to exercise the real `Run` plus capture
  response shape. Keep all checked-in fixtures synthetic.

The exact CLI may evolve, but it must accept a worker endpoint, READY snapshot
ref, private padlog/trajectory input, `feature-map.yaml`, `layout.json`, output
bundle root, capture cadence/count, and a bounded-run instruction cap. Record
the final invocation in the runbook and fulfillment notes.

## Implementation Steps

1. Parse and validate `layout.json` before contacting the worker:

   - require a nonempty range list and positive `total_len`;
   - require layout version 1 for this image;
   - recompute/verify the layout hash using the same contract as
     `phase4-layout`;
   - reject range overflow, unknown regions, and a total length unequal to the
     sum of range lengths;
   - compile ranges in the recorded order to flat `ExtractRange` entries.

2. Parse the real feature map privately and establish one canonical decoded
   feature order. Assert that its compiled ranges and total width agree with
   `layout.json`. Reject placeholder/demo input for a production run using an
   explicit production flag or provenance check rather than filename alone.

3. Restore the approved READY snapshot, inject scheduled pad events, and make
   sequential `Run` calls that stop at frame boundaries. Every primary-corpus
   request must set `CaptureSpec.framebuffer = true`. Do not pair a feature
   response from one boundary with a later `GetFramebuffer` response.

4. For every successful response:

   - require `feature_bytes.len == layout.total_len`;
   - require framebuffer metadata for 256x224, stride 1024, xrgb8888 and
     uncompressed length 229,376;
   - decompress `fb_lz4` once to validate its length; preserve the compressed
     artifact with encoding `fb_lz4`;
   - standardize hash semantics in the contract: `framebuffer.blake3` hashes
     the stored compressed artifact bytes, while a new
     `framebuffer.uncompressed_blake3` hashes the decoded xrgb8888 pixels;
     update the checker, synthetic fixtures, artifact contract, and runbook so
     consumers cannot interpret one field both ways;
   - decode feature values in map order and fail on decode width/type errors;
   - generate a non-semantic private capture id and retain node/source plus
     frame provenance;
   - write feature bytes and framebuffer to private artifact files using
     atomic temp-file-plus-rename behavior;
   - append the JSONL row only after both artifacts are durable and hashed.

5. Make restart/resume safe. On startup, verify every existing row's artifact
   refs, lengths, hashes, layout hash, and unique capture id before resuming.
   Refuse conflicting rows or a changed map/layout/source. Do not silently
   truncate or overwrite a partial corpus.

6. Emit a private export report containing command/tool/source provenance,
   worker/image refs, layout/map hashes, requested and completed capture counts,
   frame range/cadence, framebuffer summary, and pass/fail errors. Redact local
   absolute paths and never inline captured values or secret refs in console
   output.

7. Add synthetic tests covering:

   - layout-to-`CaptureSpec` order and layout-version propagation;
   - exact feature packing and decoded order;
   - feature length mismatch and wrong layout hash rejection;
   - missing/malformed framebuffer and decompression failure;
   - frame coherence/provenance propagation;
   - atomic artifact-before-index behavior and safe resume;
   - duplicate capture ids and conflicting resume state;
   - bounded instruction caps passed through to worker calls;
   - no inline byte fields and validator-compatible row refs/hashes;
   - failure on a mock `FAILED_PRECONDITION` layout mismatch.

8. Implement `phase4-artifact-check --bundle ... --report ...` as a reusable,
   read-only verification pass used after export and after retrieval. It must:

   - resolve every feature/framebuffer ref beneath the bundle root and reject
     absolute paths, traversal, symlinks, duplicate refs, and escaping refs;
   - require every artifact to exist and match row length plus stored-byte hash;
   - decompress every framebuffer, require 229,376 bytes, and check
     `uncompressed_blake3` plus declared geometry/format;
   - emit only counts, approved provenance, hashes, and errors without captured
     bytes, decoded values, ids, or absolute private paths;
   - have negative tests for missing/corrupt/truncated/swapped artifacts, path
     escape/symlink attacks, compressed-hash mismatch, and pixel-hash mismatch.

9. Extend checksum tooling so the frozen seal recursively covers all regular
   bundle files, including `artifacts/`, with deterministic relative paths,
   lengths, and hashes. Define an acyclic contract:

   - `manifest.json.bundle_checksum` is a canonical **payload root** computed
     over the immutable contract/payload tree (including recursive artifacts),
     with that field normalized to a fixed sentinel and `validation/` excluded;
     its exact include/exclude rules are versioned and tested;
   - the final freeze manifest is written outside the bundle (or to an excluded
     sidecar), records that payload root, and separately hashes every file in
     the finalized bundle including `validation/`;
   - `--verify <freeze-manifest>` reads without modifying the bundle, recomputes
     the canonical root and every file entry, and fails on missing, extra, or
     changed files;
   - tests prove deterministic output, manifest-field normalization, recursive
     artifact coverage, extra-file rejection, and no self-reference.

10. Implement `phase4-context-export` to map selected frozen capture rows and
   artifacts into `contexts.jsonl`, `manifest.json`, optional approved recent
   pad tail, and `validation/context-export-report.json`. Define the mapping in
   the guide and test decoded order/values, feature/layout/image provenance,
   framebuffer/region refs and hashes, `console16-12btn-v1`, and recent-input
   availability using synthetic fixtures.

11. Add a separately typed fallback validator mode or command as specified in
   `04a-first-room-fallback.md`. Its output must say `first-room-fallback`, must
   reject a manifest claiming the full scorer-golden kind/status, and must not
   change the >=1,000/trajectory rules of `phase4-bundle-check`.

12. Update `docs/phase4-corpus-guide/03-capture-export.html` from “missing
   producer” to the final supported command, safety model, resume semantics,
   artifact/hash semantics, verification commands, context derivation, and
   private-output rules.

## Verification

```sh
cargo fmt --all -- --check
cargo test --locked -p refwork-dh-client
cargo test --locked -p refwork-verify phase4_capture_export -- --nocapture
cargo test --locked -p refwork-verify phase4_artifact -- --nocapture
cargo test --locked -p refwork-verify phase4_context_export -- --nocapture
cargo test --locked -p refwork-verify phase4_checksum -- --nocapture
cargo test --locked -p refwork-verify phase4_fallback -- --nocapture
cargo test --locked -p refwork-verify phase4 -- --nocapture
cargo test --locked -p refwork-verify
git diff --check
```

Run one entirely synthetic end-to-end export into a temporary directory and
prove that `phase4-bundle-check` accepts the exporter-generated capture rows as
part of its synthetic fixture bundle.

## Exit Criteria

- Exporter produces atomic, resumable, frame-coherent capture rows through the
  existing client API.
- Primary rows always contain framebuffer artifacts and metadata.
- Synthetic exporter output passes the existing bundle contract.
- Artifact verification, recursive freeze-manifest generation/verification,
  context export, and the separately typed fallback path have synthetic tests.
- No real game-derived material or private paths are added to git.
- Exporter bead records tests and implementation SHA.
