# 06 - Handoff And Closeout

**Purpose:** turn the implemented packages into durable request fulfillment
records that state-scorer, input-synthesizer, and exploration-orchestrator can
cite.

## Deliverables

1. Add a sanitized fulfillment note for the pad/context request:

   Preferred path:

   ```text
   /home/infra-admin/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md
   ```

   Required contents:

   - layout id decision;
   - exact pad table;
   - source commit;
   - manifest hash or opaque generated artifact id showing `pad_layout`;
   - validation command outputs or report hashes;
   - opaque context fixture artifact id or synthetic/live status;
   - recent pad tail availability, or unavailable reason;
   - role-based access requirement and retention for any private fixture;
   - downstream macro-pack smoke result link or downstream task record proving a
     consumer can cite `console16-12btn-v1` without case aliases.

2. Add a sanitized fulfillment note for the scorer artifact request:

   Preferred path:

   ```text
   /home/infra-admin/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md
   ```

   Required contents:

   - opaque private bundle artifact id or registry ref;
   - role-based access requirement;
   - retention expectation;
   - compression format and max expected size;
   - bundle checksum or checksum manifest hash;
   - top-level file hashes;
   - reference-workload commit hash;
   - WorkloadImage manifest hash;
   - feature-map hash;
   - scoring-program hash;
   - layout hash;
   - capture count;
   - validation command templates and report hashes;
   - retrieval command template and registry fallback behavior;
   - commands used to regenerate or validate the bundle, redacted as needed;
   - known downstream commands for state-scorer smoke.

   Exact direct paths, token-owner names, retrieval commands with private
   arguments, and private artifact-registry fallback steps belong in a
   lab-private runbook or private appendix. If the request owner explicitly
   approves the request directory for those details, record that approval in the
   note before including them there.

3. Update project docs only where the contract changed:

   - `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md`
     for `pad_layout.layout_id`.
   - `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/INTEGRATION.md`
     for exact pad layout consumption if not already clear.
   - Any M6 implementation/evidence note that names the private scorer bundle.

4. Record implementation evidence in repo-local plan/evidence notes if this repo
   is the durable place for coding-agent handoff. Do not store private payloads.

5. Notify or update downstream task records:

   - state-scorer: no request directory was found under
     `/home/infra-admin/.agents/projects/state-scorer/requests/` during
     planning; record manual owner notification or add the request path if one
     is created.
   - input-synthesizer: no request directory was found under
     `/home/infra-admin/.agents/projects/input-synthesizer/requests/` during
     planning; record manual owner notification or add the request path if one
     is created. The notice must include `console16-12btn-v1`, exact casing,
     fixture artifact id, and recent pad tail availability.
   - exploration-orchestrator: fixture ref, capture provenance fields, and
     WorkloadImage/feature-map hashes. Existing request path:
     `/home/infra-admin/.agents/projects/exploration-orchestrator/requests/input-synth-v1-client-context/README.md`.

## Final Verification Checklist

- [ ] `cargo test --locked -p xtask` passes.
- [ ] Pad manifest validation rejects wrong/missing layout id.
- [ ] Pad manifest validation rejects wrong mixed-case names, wrong bits, and
      wrong reserved bits.
- [ ] `crates/refwork-script/FORMAT.md` names the layout id and exact casing.
- [ ] Determinism reference-workload docs include the layout id.
- [ ] The context fixture is marked live or synthetic and includes decoded
      features, provenance, layout/capture hashes, and recent pad tail if
      available.
- [ ] The scorer private bundle includes all files from
      `01-artifact-contract.md`.
- [ ] `manifest.json` includes workload image identity/revision, private
      artifact id, operator metadata policy, framebuffer format metadata,
      compression format, max expected size, and image validation stamp.
- [ ] `feature-map.yaml` has no placeholder offsets and marks stable
      canonical-hash fields with `stability: stable`.
- [ ] The primary scorer corpus has at least 1,000 real in-VM captures with
      framebuffer metadata.
- [ ] The trajectory labels include first-boss, goal-positive, and
      goal-negative cases.
- [ ] Reference-workload validation reports are recorded with hashes.
- [ ] A downstream macro-pack smoke result or task record proves a consumer can
      load a pack citing `console16-12btn-v1` without aliases.
- [ ] Public notes contain no ROM bytes, raw captures, framebuffer images,
      decoded feature vectors from real captures, trajectory JSONL, operator
      labels, padlog tails, private capture ids, exact private paths,
      screenshots, save RAM, or unapproved private identifiers.

## Downstream Smoke Handoff

State-scorer should be able to run its eventual equivalents of:

```sh
DETERMINISM_PHASE3_CORPUS=/path/to/phase3-scorer-corpus \
  cargo test -p scorer-features --test phase3_decode_goldens

DETERMINISM_PHASE3_CORPUS=/path/to/phase3-scorer-corpus \
  cargo test -p scorer-service --test phase4_real_capture_gate
```

Input-synthesizer should be able to run a macro-pack smoke that cites
`console16-12btn-v1` and rejects case aliases.

Exploration-orchestrator should be able to build its own `NodeContext` or
equivalent request fixture from the decoded context bundle without decoding raw
workload bytes.

## Stop Conditions

- If evidence exists only in terminal scrollback or chat, do not close the
  request. Write a durable note with hashes and opaque artifact ids.
- If public notes accidentally include private bytes, decoded feature vectors,
  screenshots, operator labels, padlog tails, private capture ids, exact private
  paths, or unapproved identifiers, remove them before sharing the handoff.
- If validation used a different WorkloadImage, feature map, scoring program, or
  reference-workload commit than the manifest claims, discard the evidence and
  rerun.
