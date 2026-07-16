# Package 02 — Tracking Restore & Gate-Check Repair (Agent-Only)

## Goal

Restore the beads tracking lost with the m6 host teardown (embedded Dolt DB
is gitignored; commit `7b9a363` preserved the tree but not the DB), and fix
`tools/m6-gate-check.sh` (KNOWN GAP 2) so it can report gate state truthfully
and fail-closed. Without this, packages 05/06/08 cannot execute their
`bd close` exit signals and the gate checker can never pass.

## Steps

### 1. Confirm the loss is real (fail-closed, don't assume)

```sh
cd "$(git rev-parse --show-toplevel)"
bd list --limit 1                      # expect: "Error: no beads database found"
git log --all --name-only --oneline -- .beads
ls -a .beads/                          # config.yaml, metadata.json, README.md, .gitignore only (plus . / ..)
```

For the `git log` check, verify the **intent**, not a specific commit list
(observed commit sets differ with/without `--all`): confirm that no commit
touching `.beads` ever added a DB file or an `issues.jsonl` — i.e. the
history holds only config/metadata/README/.gitignore additions. If any commit
DID add a DB or JSONL, that file is recoverable — stop and recover it instead
of recreating.

Also check for any stray JSONL backup before recreating anything:
`find ~ -maxdepth 3 -name 'issues.jsonl' -path '*beads*' 2>/dev/null` and the
`origin/backup/m6-host-20260715` branch (already checked 2026-07-15: no DB,
no JSONL anywhere). If a backup IS found, `bd import <file>` instead of
step 3 and skip to step 4.

### 2. Re-initialize the embedded DB

```sh
chmod 700 .beads
BD_NON_INTERACTIVE=1 bd init --prefix refwork
bd list --limit 1  # expect: empty list, no error
```

`--prefix refwork` is mandatory (flag verified in `--help`): bare `bd init`
auto-detects the prefix from the directory name — here that yields
`reference-workload`, not `refwork`, which would defeat the original-ID
restore in step 3. Set `BD_NON_INTERACTIVE=1` (or the equivalent documented
non-interactive switch) so the init wizard cannot stall a non-interactive
agent. If step-3 creates still come out with a mismatched prefix, second-line
recovery is `bd create --force` with the explicit `--id`.

After init, check the config for the JSONL auto-backup **git-push** option
(the config docs say it is "Auto-enabled when a git remote exists") and
verify it is disabled — `bd` must never be able to push without the operator
push ask (package 08). Record the setting checked/changed.

Grounding note 1 (00-overview): if `bd init` refuses over the existing
config/metadata, read `bd init --help` for the documented recovery path — do
not hand-edit Dolt state. Run all `bd` commands serially (standing rule).

### 3. Recreate the tracked beads with their original IDs

`bd create --id` exists (verified via `--help`). Recreate exactly these, each
description opening with the provenance line
`"Recreated 2026-07-15 with original id after m6-host teardown lost the embedded dolt DB."`:

```sh
bd create "Phase 4 capture exporter + artifact/context/fallback verification" \
  -d "Recreated ... Original work: commit 2827665; evidence .agents/plans/close-m6-entry-gates/EVIDENCE-czi.md; pushed per GATE-RECORD-ASK1.md addendum." \
  -p 1 -l impl -t task --id refwork-czi --silent
bd close refwork-czi -r "Committed at 2827665 with clean-checkout gate recorded (EVIDENCE-czi.md); push recorded in GATE-RECORD-ASK1.md addendum. Closure restored after DB loss."

bd create "Real private feature-map + scoring pair (validated under private root)" \
  -d "Recreated ... Runbooks: close-m6-entry-gates/03-close-refwork-20v.md + fast-follow 03. Discovery-01 session captured 2026-07-15; processing plan: .agents/plans/phase4-m6-discovery-01-processing/." \
  -p 0 -l impl -t task --id refwork-20v --silent

bd create "Frozen >=1000-state real capture corpus (full branch)" \
  -d "Recreated ... Runbooks: close-m6-entry-gates/04 + fast-follow 04/05. Blocked on refwork-20v (map/layout must be final before production capture)." \
  -p 0 -l impl -t task --id refwork-5tk --silent

bd dep add refwork-5tk refwork-20v
```

Then create the M6 tracking bead exactly per
`.agents/plans/phase4-m6-scoring-goal-integration/01-entry-gate-and-tracking.md`
step 1 (its `bd create` command verbatim, plus the two dep edges to
`refwork-czi`/`refwork-20v`; note its rationale for NOT adding a 5tk edge).
Append the recreation provenance line to its description.

Also recreate **`refwork-5be`** (it lived in the lost DB too and is cited by
the public handoff doc `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md`;
package 08 step 4 comments it and assumes it exists): `bd create ... --id
refwork-5be` with title/description reconstructed from the handoff doc's
citation, opening with the same provenance line, left **open**.

If `bd create --id` rejects these suffixes: create without `--id`, record the
old→new ID mapping in each description AND in a new
`.agents/plans/phase4-m6-discovery-01-processing/ID-MAPPING.md`, and use the
new IDs in step 5's gate-check edits. Do not silently drop the old names.

### 4. Re-apply the three ratified API.md stamps (lost edits)

The three spec ratifications of 2026-07-12 — the §1 discretize note, the §1
guard-semantics paragraph (`not{}` over a failed `valid_when`), and the §2.3
bit-range width-strictness rule — were recorded in state-scorer's
`.agents/requests/phase4-m1-m4-first-boss-scoring/05-refwork-spec-ratification.md`
but were **never stamped into**
`~/.agents/projects/determinism/docs/reference-workload/API.md` (verified
absent 2026-07-15; that docs tree is not a git repo, so nothing preserved
them — see the 00-overview stale-facts list). Packages 05 and 07 adjudicate
against API.md as normative, so this must land before they run:

- Re-apply the three stamps to API.md **exactly per the ratification doc's
  wording**, citing that doc (path + date) in each stamp.
- Agent-only; no git commit involved (the docs tree is not a repo).
- Verify by grep that all three stamps are present afterwards, and record
  the re-application (date + source citation) in this plan dir's working
  notes so a future loss is detectable.

### 5. Repair `tools/m6-gate-check.sh`

Edits (keep `set -u`, keep fail-closed semantics, no private literals):

1. **Condition 1 (scorer M3) — documented-evidence fallback.** The scorer
   repo's DB is also lost, so the current check yields permanent UNKNOWN.
   Add: when `$SCORER_REPO/.beads` has no database, PASS **only** if
   `$SCORER_REPO/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md`
   exists and its beads table marks M3 closed — a fixed-string check, e.g.
   `grep -F 'state-scorer-0gy' ... | grep -qi 'closed'` — and print the
   evidence path in the detail column. Anything less stays
   UNKNOWN/not-passed. This is positive documented evidence (their filed
   resolution), not a heuristic.
2. **Condition 4 (hand-play artifact) — make the documented candidate paths
   real instead of leaking private ones.** The script already probes
   `$HOME/.agents/projects/reference-workload/{corpus,handplay-session,first-room-fallback}`.
   Create the session pointer now (public-safe path, private target):

   ```sh
   ln -s "$PR/ramdiff/discovery-01" ~/.agents/projects/reference-workload/handplay-session
   ```

   (`corpus` gets its symlink in package 06 after the freeze; never before.)
   Tighten the probes per close-m6 package 05 step 1: for the session path
   require `session.yaml` AND `interactive.padlog` (non-empty); for the
   corpus path require `manifest.json` AND `captures/index.jsonl` — marker
   files, not bare non-empty directories.
3. Leave the informational block; update the `rom-operator-bridge-l1w` hint
   only if its bead state is re-verifiable.

Then run a redaction sanity pass over the diff before staging anything:
`git diff -- tools/m6-gate-check.sh` must contain no private path component
and no game-derived literal.

### 6. Parameterize `tools/record-ramdiff` (remove the hardcoded private root)

Package 01 step 2b's repo-wide scan confirms `tools/record-ramdiff`
(lines ~41–42) hardcodes the private-root location for `ROM_DIR`/
`SESSION_DIR` — a pre-existing GATE-RECORD-ASK1 violation (landed in commit
`5b35113`, already on `origin/main`). Fix the working tree now:

- Resolve the root from the package-01 pointer file
  (`~/.agents/projects/reference-workload/private-root.path`); fail with a
  clear public-safe message if the pointer is absent. Derive
  `ROM_DIR`/`SESSION_DIR` from it; delete the hardcoded literals.
- Commit as a **normal commit** — this fixes the tip, not history. The
  occurrence in already-pushed history is surfaced to the operator at
  STOP #1 (package 05 step 2) as a decision item; never rewrite pushed
  history unilaterally.
- Redaction-check the diff and the commit message (no private component
  anywhere, including comments), then re-run the package-01 step-2b scan:
  **zero tracked-file hits** is the new expected state.

### 7. Verify

```sh
tools/m6-gate-check.sh; echo "exit=$?"
```

Expected today: `scorer-M3 PASS` (evidence fallback), `refwork-czi PASS`,
`refwork-20v FAIL` (open — correct), `hand-play-artifact PASS
branch=raw-session` via the symlink; overall exit nonzero. Do NOT write
`GATE-RECORD.md` yet — that happens when the gate first fully passes
(scoring-goal package 01 step 3), i.e. after package 05 closes 20v.

## Acceptance criteria

- `bd show refwork-czi` closed; `refwork-20v`/`refwork-5tk`/`refwork-5be`/M6
  bead open with dep edges; every recreated bead carries the recreation
  provenance line; bead IDs carry the `refwork-` prefix; JSONL auto-backup
  git-push verified disabled.
- API.md carries all three ratified stamps (grep-verified) with the
  ratification-doc citation.
- `tools/record-ramdiff` resolves via the pointer file; package-01 step-2b
  scan now reports zero tracked-file hits.
- Gate checker output exactly matches the expectation above (3 PASS, 1 FAIL,
  nonzero exit), and `bash -n tools/m6-gate-check.sh` is clean.
- No private literal in the working-tree diff (manual read + `git diff --check`).

## On failure

- `bd` embedded-Dolt lock errors: retry serially, bounded (standing rule).
- `bd init` cannot proceed: stop and record; bead closures in later packages
  then fall back to durable plan-dir records (a `BEAD-STATE.md` in this plan
  dir) and the gate checker's bead conditions get an evidence-file fallback
  in the same style as condition 1 — but only do this after recording that
  `bd` restoration failed and why.
- Gate checker shows anything other than the expected pattern: the deviation
  is data, not noise — investigate before proceeding (e.g. czi evidence
  missing means the recreated closure was wrong; delete and redo, don't
  force).
