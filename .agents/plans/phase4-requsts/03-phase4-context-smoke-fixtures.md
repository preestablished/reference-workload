# 03 - Phase 4 Context Smoke Fixtures

**Purpose:** provide reference-workload-owned live-context fixtures that
input-synthesizer and exploration-orchestrator can use for Phase 4 smoke tests
without taking ownership of workload feature discovery.

## Scope

This package satisfies the fixture portion of
`pad-alphabet-and-phase4-context-fixtures`. It does not implement
`NodeContext`, `ProposeBursts`, macro pack validation, or orchestrator request
envelopes.

## Required Fixture Shape

Produce either:

- a lab-private bundle with the files below, plus a sanitized evidence note in
  this repo; or
- a link to the validated scorer M6/Phase 4 bundle from package 04 if that bundle
  is the source of truth.

Recommended lab-private bundle shape:

```text
phase4-context-smoke/
  manifest.json
  contexts.jsonl
  recent-input.padlog              # optional, only if available and approved
  validation/context-export-report.json
```

`manifest.json` should include:

- schema version;
- reference-workload commit;
- WorkloadImage manifest hash or opaque artifact id;
- `pad_layout.layout_id`, `layout_version`, and pad table hash;
- feature-map ref and hash;
- scoring-program ref and hash, if available;
- layout hash or capture spec hash;
- capture count;
- opaque storage artifact id, role-based access requirement, and retention
  expectation;
- `recent_input_available: true|false` and the reason if false;
- clean-room/provenance note.

Minimal example:

```json
{
  "schema_version": 1,
  "kind": "phase4-context-smoke",
  "reference_workload_commit": "<40-hex>",
  "workload_image": {
    "manifest_hash": "blake3:<hex>",
    "artifact_id": "<opaque-ref>"
  },
  "pad_layout": {
    "layout_id": "console16-12btn-v1",
    "layout_version": 1,
    "table_hash": "blake3:<hex>"
  },
  "feature_map_hash": "blake3:<hex>",
  "scoring_program_hash": "blake3:<hex>",
  "layout_hash": "blake3:<hex>",
  "capture_count": 4,
  "recent_input_available": false,
  "recent_input_unavailable_reason": "not retained by capture source"
}
```

Each `contexts.jsonl` row should include:

- `capture_id`;
- `node_ref` or equivalent source id;
- frame counter or frame index;
- workload image ref/hash;
- feature-map hash;
- layout hash or capture spec hash;
- decoded feature values in feature-map order and as a name/value map;
- framebuffer metadata and hash, not framebuffer bytes;
- region metadata needed to identify the source, such as region names, sizes,
  layout versions, and region content hashes;
- recent input tail reference if available, either `recent-input.padlog` plus
  frame range or inline canonical u16 pad words if the operator approves that
  metadata.

Minimal JSONL row example:

```json
{"schema_version":1,"capture_id":"cap-000001","node_ref":"node:root","frame_index":120,"workload_image_manifest_hash":"blake3:<hex>","feature_map_hash":"blake3:<hex>","layout_hash":"blake3:<hex>","decoded_order":["room_id","area_id","player_x"],"decoded_values":[1,0,144],"decoded_by_name":{"room_id":1,"area_id":0,"player_x":144},"framebuffer":{"encoding":"fb_lz4","width":256,"height":224,"stride":1024,"pixel_format":"xrgb8888","blake3":"blake3:<hex>"},"regions":[{"name":"wram","size":131072,"layout_version":1,"blake3":"blake3:<hex>"}],"recent_input":{"available":false,"reason":"not retained by capture source"}}
```

If `recent-input.padlog` is available, it should use
`crates/refwork-script/FORMAT.md` and must keep bits 12-15 zero. It should be
recent enough for synthesizer tail-continuation and refractory-rule smoke tests,
but it does not need to be a full trajectory. If it is unavailable, record the
reason in `manifest.json` and in each context row.

## Implementation Steps

1. Wait for the Real Capture Evidence Floor from `00-overview.md`. Do not
   fabricate a "live" fixture from placeholder YAML.

2. Select a small set of captures that is representative for synthesizer smoke:

   - root or near-root state;
   - ordinary movement state;
   - state with at least one stage-related feature changed, if available;
   - state with recent input tail containing directions and at least one face or
     menu button, if the capture source retained recent input.

3. Export decoded features through a reference-workload-owned tool or lab script.
   Prefer package 05A's `refwork-verify context-fixture` or
   `refwork-verify trace` subcommand. If that tooling does not exist yet, use an
   exact lab-private script path and command, and record only its opaque id,
   commit/hash, and a redacted command template in public notes.

4. Ensure the exported decoded feature order matches the feature map order. Do not
   let input-synthesizer define an alternate feature order.

5. Produce `validation/context-export-report.json` under the lab-private fixture
   root with:

   - command;
   - input opaque artifact ids;
   - output file hashes;
   - feature-map hash;
   - WorkloadImage manifest hash;
   - capture count;
   - pass/fail status.

6. Add a sanitized handoff note in this plan directory or the request directory.
   It may name opaque artifact ids, hashes, counts, roles, and redacted command
   templates. It must not contain raw WRAM, framebuffer data, decoded feature
   vectors from real captures, trajectory JSONL, operator labels, padlog tails,
   private capture ids, screenshots, save RAM, ROM bytes, exact private paths, or
   unapproved private identifiers unless separately approved for publication.

## Consumer Smoke Expectations

Input-synthesizer should be able to:

- load a macro pack that cites `console16-12btn-v1`;
- reject all-caps button names without aliases;
- consume the decoded feature map as context through its own `NodeContext`
  conversion;
- consume the recent pad tail as canonical u16 pad words or a `.padlog` if one
  is available.

Exploration-orchestrator should be able to:

- construct its own node context/request fixture from the decoded feature values;
- preserve capture provenance and hashes;
- avoid decoding workload bytes itself.

## Acceptance

- A private fixture bundle or opaque private artifact id exists.
- The bundle names the WorkloadImage manifest, feature map, layout/capture spec,
  capture ids, frame indices, and recent input tail availability.
- Decoded feature values are present and ordered by the canonical feature map.
- `pad_layout.layout_id` is present and equals `console16-12btn-v1`.
- The public repo/request evidence contains only sanitized metadata.
- The fixture is explicitly labeled as live evidence or synthetic smoke. Synthetic
  smoke may help local tests, but it must not close the Phase 4 live-context gate.

## Stop Conditions

- If the Real Capture Evidence Floor is unavailable, stop and record that live
  context fixtures are blocked on that floor.
- If the capture path lacks frame-coherent framebuffer metadata, stop and fix the
  capture path before producing a live fixture.
