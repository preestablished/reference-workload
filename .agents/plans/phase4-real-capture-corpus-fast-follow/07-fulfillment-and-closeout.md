# 07 - Fulfillment And Closeout

## Purpose

Close the evidence floors to their own acceptance standards and leave a
sanitized resolution that the phases track can independently verify.

## Scorer Golden-Artifacts Fulfillment

Update
`$HOME/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`
with:

- truthful status: fulfilled only for the complete corpus; partial for the
  approved first-room fallback;
- opaque bundle id/registry ref and checksum-manifest hash;
- approved top-level hashes and reference-workload commit;
- WorkloadImage, map, scoring, and layout hashes/opaque refs as publication
  policy allows;
- capture count and categorical label/dedup/K=32 coverage;
- validation commands and private report hashes/status;
- retrieval command and registry-unavailable fallback, redacted or delegated to
  a named private runbook when details cannot be public;
- access group/token owner role, retention, regeneration commands, compression,
  and expected size;
- operator approval for published game/revision metadata or explicit
  private-only disposition;
- state-scorer cold-agent smoke location and actual result/status.

Do not replace required retrieval/access details with “contact owner.” If
secrets cannot be public, name the precise private handoff document and the role
allowed to read it.

## Pad/Context Fulfillment

Update
`$HOME/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`
with:

- live context fixture opaque id and its frozen corpus id;
- evidence type, validation report hash/status, and capture/export provenance;
- feature/layout/image hashes or approved opaque refs;
- recent pad tail availability;
- retrieval/fallback, access owner/group, retention, and regeneration commands;
- input-synthesizer cold-agent smoke location and actual result/status;
- unchanged `console16-12btn-v1` contract.

## Repo-Local Resolution

Append
`.agents/requests/phase4-real-capture-corpus-fast-follow/04-resolution.md`
containing only sanitized evidence:

- implementation and plan SHAs;
- exporter/map/production bead ids and close reasons;
- selected full/fallback branch;
- opaque corpus/context ids and checksum-manifest hash;
- capture count and categorical coverage table for start-area, first-upgrade,
  first-boss, goal-positive, goal-negative, same-canonical, and
  distinct-stable evidence;
- fulfillment files changed and their resulting statuses;
- downstream smoke document locations/results;
- operator publication/private-only disposition without private metadata;
- exact public-safe verification commands.

Do not overwrite request file `05-current-status-2026-07-10.md`; the resolution
is a new point-in-time record.

## Bead Closeout

- Close exporter and real-map beads only after their code/private evidence exits
  pass.
- Close production/freeze bead only after fresh retrieval verification.
- Close or update `refwork-d7t.1` according to package 01 evidence.
- Close reasons must cite durable source paths, private opaque refs/report
  hashes, commands, and commits without leaking private literals.
- If using the fallback, leave the trajectory follow-on open and owned.

## Final Public-File Audit

Run redaction scanning separately against the resolution, both fulfillment
files, both downstream smoke documents, and any bead close-reason draft. Then:

```sh
git status --short
git diff --check
git diff -- . ':(exclude)target'
git ls-files | rg 'captures|framebuffer|padlog|trajectory|feature-map|scoring-program'
```

Manually inspect every newly tracked file. Pattern matches can be legitimate
source/tests, but no private payload, real offset map, operator label, or secret
may be tracked.

## Exit Criteria

- Both fulfillment records meet their own handoff-note requirements and show
  truthful statuses.
- Resolution contains the requested handback shape and one consistent corpus
  id.
- Beads have evidence-based close reasons; fallback follow-on remains open if
  applicable.
- Final public files pass automated and manual privacy review.
