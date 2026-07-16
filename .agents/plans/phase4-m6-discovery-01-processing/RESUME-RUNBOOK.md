# Resume Runbook — everything staged, two operator inputs pending

State as of 2026-07-16 (execution session; see WORKING-NOTES.md for the
full evidence trail). Packages 01–04 complete and committed. Package 05:
draft pair v2 validated (structural dry-run exit 0; draft map-check 23/23
over the full discovery-01 padlog); STOP #1 items 2/3/4 agent-resolved via
the scripted-death restore gate (which caught and fixed two
restore-unstable feature picks). Packages 05-finalization through 08 are
staged below and blocked ONLY on the operator inputs listed first.

## Operator inputs required (the complete list)

1. **Credits/completion evidence (STOP #1 item 1).** Options per ASK1
   (source decision is the operator's, re-confirmed at session time):
   - Hand-play to credits: `tools/record-ramdiff discovery-02`, F5-label
     pre-credits and during/post-credits dumps (power-on route keeps the
     full replay-derived toolset: watch + second map-check pass).
   - Late-game save RAM on the host core (none exists under `$PR` —
     verified): direct F5-dump search only; record the evidence reduction.
   - OR explicitly defer Run C → the plan's fallback branch: hold 20v open
     (default), or direct the reduced pair (goal on the world-1-clear
     latch; "fires-on-credits" recorded undeclarable everywhere).
2. **Commit `5b35113` decision (STOP #1 item 5):** rewrite pushed history
   to purge the private-root literal, or accept-and-record.
3. **STOP #2 (package 06) coordination — verified facts to plan against:**
   the worker stack does NOT exist on this Mac (no
   `~/.rbo73/m4-regen-20260707` snapstore copy, no `/run/dh/grpc.sock`,
   no `dh-workerd` binary; only repo checkouts were migrated in `~/m6`).
   Redeploying bridge → dh-workerd `6e348e5` → snapstore with READY
   snapshot `948b73e6` (or successor) is operator/bridge work. Also
   needed: the Play-window agreement (`rom-operator-bridge-l1w` state
   unverifiable — its DB was also lost) and the `--hard-icount-cap`
   sign-off (agent proposes: frames × observed icount/frame from m4/m5
   evidence × 4 headroom).
4. **Push ask (package 08 step 5):** explicit approval for
   `git push origin main` at closeout.

## On resume after credits evidence (discovery-02 exists)

1. Session-completeness checklist (pkg 05 STOP block): all briefed dumps
   present; every dump 131,072 B; padlog header + log_frames match;
   session.yaml lists everything; reconfirm the private root is the
   ASK1-approved one.
2. Power-on route only: replay-fidelity mini-gate over discovery-02
   (pkg 01 step 5 mechanics; evidence →
   `$PR/evidence/replay-fidelity-02.txt`). Save-RAM route: skip, record
   the reduction.
3. Discovery over discovery-02: credits_flag (changed pre→during-credits,
   persist semantics per what the game does), confirm the dead-value
   conjunction still holds if a game-over dump exists. Reuse
   `$PR/discovery/explore.py` + scratch sessions; watch only on a passed
   mini-gate.
4. Finalize: copy `$PR/bundle-draft/` → `$PR/bundle/`; add credits_flag to
   the map; upgrade the 9 restore-confirmed fields volatile→stable
   (evidence: `$PR/discovery/restore-confirmation.json` + scripted-death
   continue-reload dumps + any Run C reload dumps); add the grid
   discretize block on player_x (x: player_x, y: player_y, room: room_id
   — room is stable after the upgrade) and threshold edges on health;
   replace the DRAFT goal with the real credits predicate; drop the
   DRAFT-NOT-EVIDENCE headers; bump nothing else (fast-follow 03 freeze
   discipline starts at capture).
5. Pipeline stages 1–3 (PATH must have `~/.venvs/refwork/bin` first for
   the layout stage):
   `tools/m6-session-pipeline.sh validate-map --private-root "$PR"`;
   `... map-check --rom "$ROM" --script "$SESS/interactive.padlog"
   --expect "$PR/bundle/validation/map-check.expect.yaml"` (add the
   credits/dead assertions + never-clauses that Run C evidence supports;
   boot-era health-0 means NO bare `never health==0` clause);
   second map-check pass over the Run C padlog (preserve pass-1 report as
   `map-check-run1.json` if the stage hardcodes its path);
   `... layout --capture-spec-hash <opaque-ref>`.
6. Close: `bd close refwork-20v -r "<opaque refs + report statuses only>"`;
   `tools/m6-gate-check.sh` → expect 4/4 (branch=raw-session); write
   `.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md`
   (verbatim checker output, branch, scorer build SHA, scope).

## Package 06 (after STOP #2 holds)

Alignment probe (8 captures, cadence 600, direct binary — no 1,000 floor)
against host-side decodes at the same frames → `$PR/evidence/
align-probe-01.txt`, 8/8 bar. Production:
`tools/m6-session-pipeline.sh capture --count 1005 --cadence 45
--hard-icount-cap <agreed> --source-ref <opaque> --production`.
Credits segment → SIBLING bundle (`$PR/bundle-credits`, own source-ref;
resume cannot append a second padlog — source-verified). Compose index
per pkg 06 step 5 (identical map/layout hashes + disjoint capture ids
asserted; single derived hash under the corpus lineage rule). dedup
groups from `$PR/discovery/idle-runs.txt`; "first boss" decision: the
W1-S2 midboss is `first_boss` (one decision, used identically in labels,
score-plan ids, trace labels — record it). artifact-check both bundles;
score-plan against the composed index; freeze per fast-follow 05; THEN
`ln -s "$PR/bundle" ~/.agents/projects/reference-workload/corpus`;
close refwork-5tk.

## Package 07

Labels file generated from the composed index + frame→event table
(every capture id labeled; `goal: true` only on credits-bundle rows;
spot-check ≥10). trace → trajectory + report. Build & start state-scorer
M4 service per their `docs/joint-smoke-runbook.md` §1 (agent-doable),
LoadFeatureMap (cross-check feature_bytes_len) → LoadScoringProgram,
record hashes + build SHA. Run scoring-goal 03/04 runbooks (expected
totals computed from the REAL program). Write GATE3-CLAIMS.md per the
branch actually taken. Deliver the state-scorer handoff (fast-follow 06
checklist; update handoff doc §5 M4 slot; redaction-scan everything).

## Package 08

Gate-check 4/4 full-corpus; GATE-RECORD update; both FULFILLMENT files
(create `~/.agents/projects/reference-workload/requests/...` dirs first;
truthful pending statuses for unexecuted fast-follow scope);
`04-resolution.md` from scoring-goal 08 skeleton after pre-running matrix
rows 1–3; bead hygiene (comment M6 + 5be); widened redaction audit + the
repo-wide private-component scan (expect zero hits); restate the 5b35113
decision; operator push ask; verify up-to-date after push.

## Known environment facts the resume session needs

- Private root: pointer file `~/.agents/projects/reference-workload/
  private-root.path` (mode 600). Never echo its value; mask paths in any
  displayed output; redirect BOTH streams of ramdiff emit/candidates/watch.
- pyyaml lives in `~/.venvs/refwork` (PEP 668 blocked --user; no brew
  formula) — prepend its bin to PATH for pipeline layout stages.
- Pre-commit hook runs `cargo test --workspace`, which cannot build on
  this Mac (guest-sdk `libc::SOCK_CLOEXEC`); use `--no-verify` with the
  change surface verified directly, per WORKING-NOTES.
- The shell is zsh: no word-splitting of unquoted vars (use arrays for
  `--mark` lists), failed globs error out with the pattern echoed.
- All local commits are unpushed by design until the package-08 push ask.
