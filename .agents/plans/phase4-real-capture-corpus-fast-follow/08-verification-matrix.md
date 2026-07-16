# 08 - Verification Matrix

## Source And Synthetic Gates

Run from the reference-workload checkout used by the manifest:

```sh
cargo fmt --all -- --check
cargo test --locked -p refwork-featuremap
cargo test --locked -p refwork-dh-client
cargo test --locked -p refwork-verify phase4_capture_export -- --nocapture
cargo test --locked -p refwork-verify phase4_artifact -- --nocapture
cargo test --locked -p refwork-verify phase4_context_export -- --nocapture
cargo test --locked -p refwork-verify phase4_fallback -- --nocapture
cargo test --locked -p refwork-verify phase4 -- --nocapture
cargo test --locked -p refwork-verify checksum -- --nocapture
cargo test --locked -p refwork-verify redaction -- --nocapture
cargo test --locked -p refwork-verify
cargo test --locked -p xtask
git diff --check
```

If the new exporter test filter has a different final name, record the actual
command in the resolution.

## Private Full-Corpus Gates

| Gate | Required proof |
|---|---|
| Intake | Single approved ROM, private root outside checkout, operator policy recorded |
| Worker safety | Deployed worker includes `c0337ab`+ or bounded-run caps recorded |
| Map/scoring | Pair validates; no placeholder offsets; stable/discretize semantics reviewed |
| Real layout | Real map-check/region-layout pass; layout version 1 and exporter commit recorded |
| Export | >=1,000 unique frame-coherent rows; exact layout hash/length; framebuffer on every primary row |
| Artifacts | Reusable artifact checker proves contained refs, stored-byte hashes, pixel hashes, and decompressed length |
| Dedup | Same-canonical volatile-only and distinct-stable examples both present |
| Score plan | K=32, fixed unique batch ids, valid checkpoint/restore refs, mandatory labels covered |
| Trajectory | Start/upgrade/boss anchors, first-boss true, goal true and false rows |
| Bundle | `phase4-bundle-check` passes after final validation evidence is present |
| Integrity | Recursive external freeze manifest generated last and verified non-mutating from a freshly retrieved copy |
| Context | `phase4-context-check` passes with `evidence_type: live` |
| Freeze | One immutable opaque id in manifest, both fulfillments, handoffs, and resolution |

## Fallback Gates

When the explicitly approved fallback is selected, execute
`04a-first-room-fallback.md` and replace only the inapplicable full-corpus gates
with:

- approved fallback decision and named follow-on owner/task;
- truthful exact capture count and first-room-only scope;
- decode, framebuffer, dedup, stable/volatile, integrity, retrieval, and privacy
  evidence;
- scorer fulfillment remains partial;
- no first-boss/goal coverage claims and no fabricated rows;
- separate typed fallback schema/validator with synthetic tests and exact
  artifact/freeze verification commands;
- no global weakening of `phase4-bundle-check` full-corpus rules.

## Handoff Gates

- Fresh private retrieval works via the primary command and documented fallback.
- Access group/token owner role and retention expectation are actionable.
- Regeneration and validation commands run without relying on terminal history.
- State-scorer and input-synthesizer smoke instructions reflect their current
  repositories; unimplemented tests are tracked rather than reported green.
- `console16-12btn-v1` is consistent everywhere.
- Both fulfillment files and the resolution cite the same corpus version.

## Privacy Gates

- Private bundle and context fixture are not beneath a git worktree.
- `git status` and `git ls-files` show no raw captures, framebuffers, real maps,
  scoring labels, trajectories, padlogs, ROM/save data, or private manifests.
- Redaction scan passes on every intended public note using the private
  forbidden-literal list.
- Manual review finds no decoded real vectors, capture ids, exact private paths,
  access tokens, operator secrets, or unapproved game/revision metadata.
- Public commands use placeholders or approved opaque ids and do not expose
  credentials through arguments or logs.

## Independent Reproduction

Before declaring completion, use a clean checkout plus a freshly retrieved
private bundle to rerun:

1. non-mutating recursive freeze-manifest verification;
2. reusable artifact verification with its report outside the frozen bundle;
3. feature-map/scoring validation;
4. the full `phase4-bundle-check` or separately typed fallback checker;
5. `phase4-context-check` on the live fixture when produced;
6. every available downstream smoke;
7. redaction scans on final public evidence.

Record commands, commits, report hashes, pass/fail, runner/owner role, and UTC
timestamp in durable evidence. The work is not complete if proof exists only in
terminal output or chat.
