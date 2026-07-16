# Package 08 — Closeout & Resolution (Last)

## Goal

Wire the evidence back into every record that tracks M6: gate checker green,
GATE-RECORD current, fulfillment records written, the M6 request resolution
(`04-resolution.md`) authored, beads consistent, push asked. This largely
executes `close-m6-entry-gates/05-closeout-and-gate-verification.md` and
scoring-goal package 08 — cite them; below is only sequencing plus what is
new since they were written.

## Steps

### 1. Gate verification

`tools/m6-gate-check.sh` → expect 4/4 PASS, `branch=full-corpus` (the
package-06 corpus symlink + marker files). Update
`.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md` (created in
package 05 at first full pass) with the full-corpus upgrade: date, verbatim
output, scorer build SHA, scope.

### 2. Fulfillment records (fast-follow ownership — write, don't duplicate)

The two records fast-follow 07 specifies do **not exist yet** (verified:
`~/.agents/projects/reference-workload/requests/` is empty) — create the
directories first, then the files to its exact field lists:

```sh
mkdir -p ~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts \
         ~/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures
```

- `.../requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`
- `.../requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md` —
  note: the live context fixture derivation (fast-follow 06,
  `phase4-context-export`/`-check`) is fast-follow scope; if it has not been
  executed, record its status truthfully as pending with an owner rather
  than claiming it.

One corpus id everywhere (fast-follow 05 freeze protocol; scoring-goal
package 08 matrix row 3 checks exactly this).

### 3. The M6 resolution — deliverable (e)

Create `.agents/requests/phase4-m6-scoring-goal-integration/04-resolution.md`
from scoring-goal package 08's skeleton, after pre-running its
self-verification matrix. Honesty constraints specific to this plan's scope:

- Rows 1–3 of the matrix (trace re-run from clean checkout, scorer
  re-evaluation, corpus-id cross-check) must pre-pass — packages 06/07
  produced everything they need.
- Items OUTSIDE this plan's scope — the fixture-corpus **budget** run with
  the expected-scores sidecar (scoring-goal 05), the exploration-readiness
  smoke (scoring-goal 06), and the handoff surface (scoring-goal 07) — are
  filled in only if they actually ran under that plan by the time the
  resolution is written. Otherwise the resolution names them open with
  owners, and the M6 bead **stays open** with a comment; do not close M6 on
  a resolution with open validation rows. (The skeleton's smoke section is
  quoted verbatim by the phases track — never fabricate counts.)
- Gate assessment: copy the conclusion from `GATE3-CLAIMS.md` (package 07
  step 3) — declared in full, or the "fires" half named as blocked on the
  credits capture.

### 4. Bead hygiene

Per close-m6 05 step 3: `refwork-czi`/`refwork-20v`/`refwork-5tk` closed with
evidence (done in packages 02/05/06 — re-verify with `bd show`, serially);
comment the M6 bead with the GATE-RECORD and resolution pointers; comment
`refwork-5be` (recreated in package 02 step 3 — re-verify it exists, recreate
with `--id` + provenance line only if that somehow did not happen) that its
gate is open.

### 5. Public-file audit and push ask

- Redaction pass per fast-follow 07's final audit, with **widened scope**:
  `redaction-scan` (with the private forbidden-literal list from the freeze)
  against **all tracked files this plan touched** — not just files it
  added — including the gate-check and `tools/record-ramdiff` diffs, plan-dir
  additions (`GATE3-CLAIMS.md`, `ID-MAPPING.md` if any), GATE-RECORD, both
  FULFILLMENTs, `04-resolution.md`, handoff/smoke docs, bead close-reason
  drafts. PLUS re-run package 01 step 2b's **repo-wide tracked-file scan**
  for the private-root path component
  (`git ls-files -z | xargs -0 rg -l <component>`, pattern derived from the
  pointer file, never written publicly) — expected result now: **zero
  hits** (package 02 step 6 removed the `tools/record-ramdiff` literal). A
  file-list-scoped audit alone can never catch a pre-existing leak; the
  repo-wide scan is the backstop. Then the git checks from that package
  (`git status --short`, `git diff --check`, `git ls-files | rg ...`) and a
  manual read of every newly tracked file.
- The pushed-history occurrence of the private root (commit `5b35113`, on
  `origin/main`) is governed by the operator's STOP #1 decision (package 05
  step 2 item 5) — restate that decision and its status here; it is not
  silently resolved by the tip fix.
- **STOP — operator push ask** (close-m6 05 step 5): main carries this
  plan's tooling/plan/record commits, unpushed. List the commits; explicit
  approval required for `git push origin main`; after pushing, verify
  `git status` shows up to date with origin.

### 6. Hand off what remains

Say explicitly, in the resolution and the M6 bead comment, that execution
continues under `.agents/plans/phase4-m6-scoring-goal-integration/`
packages 05–07 (sidecar+budget, smoke, handoff surface) — those gates now
hold, with this plan's artifacts as their inputs.

## Acceptance criteria

- Gate checker 4/4 with full-corpus branch; GATE-RECORD updated.
- Both FULFILLMENT files exist with truthful statuses; one corpus id across
  every record (grep-verify, matrix row 3).
- `04-resolution.md` exists, matrix-backed, with open items (if any) named
  and owned; M6 bead state matches the resolution exactly.
- All public files pass redaction scan; push approved-and-verified or
  explicitly deferred by the operator.

## On failure

- Any matrix row fails: the corresponding freeze/evidence is wrong — fix at
  the source package and re-freeze under the versioning rules; never edit
  frozen artifacts or the resolution to match each other.
- Redaction scan hit: pull the file, fix, re-scan; if it was already
  committed, the commit must be rewritten BEFORE the push ask (rewriting
  unpushed local history is fine; never after push).
