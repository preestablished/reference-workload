# 06 - Context Fixture And Downstream Handoffs

## Purpose

Derive the live Phase 4 context fixture from the same frozen evidence and leave
cold-agent instructions for state-scorer and input-synthesizer.

## Live Context Fixture

1. Use the package-02 `phase4-context-export` producer to derive a private
   context bundle from one or more frozen corpus captures; do not hand-author
   rows or recapture against a different map/layout/image. Include:

   ```text
   manifest.json
   contexts.jsonl
   validation/context-export-report.json
   recent-input.padlog        # only if approved and declared available
   ```

2. Set `evidence_type` to `live`, cite the frozen corpus id privately, and make
   its manifest agree on reference-workload commit, workload image hash/ref,
   `console16-12btn-v1` version 1 and table hash, feature/scoring/layout hashes,
   capture count, capture/export provenance, and recent-input availability.

3. Record the exact selected capture refs privately and run the producer before
   the checker. The producer must resolve and verify artifacts, preserve decoded
   order/values and image/map/layout provenance, and emit its durable export
   report. Then run:

   ```sh
   cargo run --locked -p refwork-verify -- phase4-context-check \
     --bundle "$CONTEXT_BUNDLE" \
     --report "$CONTEXT_BUNDLE/validation/phase4-context-check.json"
   ```

4. Freeze/upload the context fixture under the approved access and retention
   policy. Record its opaque id and its relationship to the corpus id. If recent
   input is unavailable or disallowed, say so explicitly rather than creating a
   synthetic tail.

## State-Scorer Handoff

Inspect the current state-scorer repository before finalizing command names.
Write a smoke document that a cold agent can follow with only bundle access:

- how to obtain credentials or the access group (never embed a token);
- primary retrieval command and registry-unavailable fallback;
- how to verify checksum manifest and frozen corpus id;
- expected directory environment variable/interface;
- decode-golden command;
- K=32 real-capture gate command;
- expected coverage: canonical equality for volatile-only pairs, inequality for
  stable changes, checkpoint/restore repeatability, staged labels, and goal
  truth selectivity;
- ownership boundary: scorer owns scores, canonical hashes, archive behavior,
  and latency after the reference-workload bundle passes.

If consumer tests are not implemented yet, provide exact proposed commands plus
a tracked downstream task; do not claim a passing smoke.

## Input-Synthesizer Handoff

Inspect the current input-synthesizer repository before finalizing commands.
The cold-agent smoke must identify:

- `console16-12btn-v1`, layout version 1, exact mixed-case button names, and
  reserved-bit behavior;
- live context fixture retrieval and integrity verification;
- whether `recent-input.padlog` is available;
- a macro-pack/context load command or a tracked test to implement it;
- a negative test rejecting all-caps aliases;
- ownership boundary for macro synthesis versus reference-workload capture
  truth.

## Privacy And Consistency Checks

- Use opaque ids and approved access-role names in public handoffs.
- Keep decoded real vectors, capture ids, padlog content, exact private paths,
  retrieval secrets, and game/revision identifiers private.
- Confirm the same corpus id and hashes appear in both handoffs and fulfillment
  records.
- Run `redaction-scan` on the final smoke documents.

## Exit Criteria

- Live context fixture passes `phase4-context-check` and is retrievable.
- Both consumer handoffs are executable or clearly identify still-unimplemented
  downstream commands with owners/tasks.
- Pad layout contract and recent-input availability are unambiguous.
- No downstream smoke is reported as passed unless it was actually executed.
