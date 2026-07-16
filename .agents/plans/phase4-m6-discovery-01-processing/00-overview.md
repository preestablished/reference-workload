# Phase 4 M6 — Discovery-01 Processing (From Captured Session To M6 Evidence)

## Outcome

Take the completed hand-play capture session (`discovery-01`, 2026-07-15:
16 labeled full-WRAM dumps + a 45,230-frame `interactive.padlog` +
`session.yaml`, all under the approved private root) and drive it to:

- **(a)** `refwork-20v` closed — the real private feature-map +
  scoring-program pair authored and validated;
- **(b)** `refwork-5tk` closed — the ≥1,000-state corpus captured, validated,
  and frozen;
- **(c)** the gate-3 labeled trajectory produced and scorer-evaluated, with an
  explicit record of what gate 3 CAN claim from this trajectory (it ends at
  world-2 stage-1; no credits state) vs what needs the credits-reaching
  capture;
- **(d)** the state-scorer corpus handoff delivered (their M1–M4 is
  code-complete and waits only on this — `state-scorer` bead `state-scorer-v8n`,
  runbook `~/git/preestablished/state-scorer/docs/joint-smoke-runbook.md`);
- **(e)** `.agents/requests/phase4-m6-scoring-goal-integration/04-resolution.md`
  written per that plan's package 08 skeleton.

## Relationship to existing plans (cite, don't duplicate)

This plan is the **execution layer for the post-capture half** of:

- `.agents/plans/close-m6-entry-gates/` packages 03–05 and
  `SESSION-DAY-RUNBOOK.md` §§2–6 (Run A happened as `discovery-01`; this plan
  is everything downstream);
- `.agents/plans/phase4-real-capture-corpus-fast-follow/` packages 03–07 (the
  mechanics runbooks — where this plan and a fast-follow package disagree on
  mechanics, the fast-follow wins);
- `.agents/plans/phase4-m6-scoring-goal-integration/` packages 01–04, 08 (the
  M6 request choreography; its packages 05–07 — fixture-budget joint run,
  exploration smoke, handoff surface — are **out of scope here** and continue
  under that plan once this plan's outputs exist).

Two realities have changed since those plans were written; every package here
carries the corrections:

1. **The captured labels are trajectory-style**, not the controlled
   state-change label set `tools/m6-discovery-analyze.sh` hard-requires
   (`REQUIRED_LABELS`, lines 46–49 — missing labels are a hard `exit 1`).
   Package 03 reconciles this with replay-derived dumps + a plan-driven
   analyzer, no new operator session needed for the core feature set.
2. **The beads DB is gone.** The teardown of the m6 host (commit `7b9a363`)
   preserved the working tree but the embedded Dolt DB is gitignored and was
   lost — `bd list` in this checkout and in `state-scorer` both report "no
   beads database found". Package 02 restores tracking (with original IDs via
   `bd create --id`) and repairs `tools/m6-gate-check.sh` (KNOWN GAP 2: its
   CANDIDATE PATHS point at locations that never got created).

Stale-fact corrections (re-verified 2026-07-15):

- The three API.md spec questions (threshold edge inclusivity, `not{}` over a
  failed `valid_when` guard, bit-range strictness) are **ratified**
  (2026-07-12) — see
  `~/git/preestablished/state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/05-refwork-spec-ratification.md`.
  **BUT the stamps were never landed in API.md**: verified absent from
  `~/.agents/projects/determinism/docs/reference-workload/API.md` (that docs
  tree is not a git repo, so nothing preserved them — the ratification record
  exists, the stamps were lost or never applied). Package 02 re-applies the
  three stamps exactly per the ratification doc BEFORE packages 05/07
  adjudicate against API.md as normative. close-m6 package 03's "settle them
  first" is decided but not yet stamped.
- scorer M1–M4 closed at the SHAs in their `04-resolution.md` (M3 =
  `b9a6437`, M4 = `09439cf`); their DB loss makes `bd` unable to show it —
  package 02's gate-check repair handles that with documented evidence, not a
  heuristic.
- `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md` exists with the
  hand-score table and per-milestone needs (scoring-goal package 02 pre-gate
  half: done).
- `GATE-RECORD.md` (scoring-goal package 01 step 3) does **not** exist yet.

## Privacy conventions (binding, every package)

- `$PR` = the operator-approved private root (GATE-RECORD-ASK1: location
  never appears in this plan or any public record). Package 01 step 1 defines
  how the executing agent resolves it (a pointer file outside every checkout).
  The session is `$PR/ramdiff/discovery-01`; ROM(s) under `$PR/ROMs/`;
  the bundle is `$PR/bundle` (the `tools/m6-session-pipeline.sh` default).
- Dumps are referred to **by frame number** in this plan (f1242 … f41511)
  with generic role names only. The operator's actual label strings are
  game-descriptive and stay in the private `session.yaml`; match by frame,
  treat label text as opaque (one overworld label contains an operator typo —
  harmless, do not "fix" the session file).
- No ROM names, offsets, decoded values, or private absolute paths in
  anything under this checkout, in beads, or in terminal stdout. Commands
  that would print such values must redirect to files under `$PR`
  (`ramdiff candidates` and `ramdiff watch` print decoded values — always
  redirect; `ramdiff search` prints only counts and is safe).
- Never modify `$PR/ramdiff/discovery-01/` — it is source evidence. All
  derived work goes in sibling directories.
- **Known pre-existing violation:** `tools/record-ramdiff` (lines ~41–42)
  hardcodes the private-root location, and that landed in commit `5b35113`,
  already on `origin/main` — a GATE-RECORD-ASK1 breach ("never in any public
  record") that predates this plan. Package 01 detects it with a repo-wide
  tracked-file scan; package 02 parameterizes the tool (working-tree fix,
  normal commit); the already-pushed history occurrence is an **operator
  decision item at STOP #1** (package 05 step 2) — history rewrite of pushed
  commits is never unilateral; package 08 re-scans repo-wide before the push
  ask.

## Packages

| File | Package | Operator-gated? | Blocked on |
|---|---|---|---|
| `01-preflight-and-replay-fidelity.md` | Session integrity, builds, prereqs, byte-exact replay gate | no — **start here** | nothing |
| `02-tracking-restore-and-gate-check-repair.md` | Beads DB restore (original IDs), `m6-gate-check.sh` path/`bd` fixes | no | 01 (private-root pointer) |
| `03-label-reconciliation-and-derived-dumps.md` | KNOWN GAP 1: frame plan, replay-derived dumps, plan-driven analyzer | no | 01 |
| `04-offset-discovery-and-semantic-confirmation.md` | Candidate narrowing, `ramdiff watch` confirmation, stability evidence | no (STOP marker only if evidence insufficient) | 03 |
| `05-real-pair-authoring-and-validation.md` | **STOP #1** (Run C: credits/dead/reload mini-session) → author + validate pair, layout, close 20v | partially | 04 |
| `06-corpus-capture-and-freeze.md` | **STOP #2** (stack + window) → capture export, artifact-check, score-plan, bundle freeze, close 5tk | yes | 05 |
| `07-gate3-trajectory-and-scorer-handoff.md` | Trace → labeled trajectory; scorer evaluation; gate-3 claim record; handoff | joint (scorer service) | 06 |
| `08-closeout-and-resolution.md` | Gate-check 4/4, GATE-RECORD.md, fulfillments, 04-resolution.md, push ask | push ask only | 07 |

Packages 01–04 are executable immediately with zero operator input (offset
discovery runs entirely on the existing local dumps + deterministic replay).
**Launch contract (read this before claiming zero-input):** that claim holds
only if the kickoff prompt supplies the private root out-of-band OR the
pointer file (`~/.agents/projects/reference-workload/private-root.path`)
already exists. Otherwise package 01 stops at step 1 by design — that is the
one permitted question outside the marked STOPs, and it happens immediately,
not mid-run. Operator involvement is otherwise confined to the explicitly
marked **STOP-AND-COORDINATE** blocks in 05 and 06 (plus the push ask in 08),
and is batched per the close-m6 operator-involvement model.

Batching opportunity (optional, sequencing unchanged): STOP #1's ask may also
**initiate** STOP #2's coordination — the stack redeploy and Play-window
agreement are operator/bridge work that can proceed in parallel while the
agent processes Run C. STOP #2's checks must still all hold before any worker
traffic; only the kickoff of that coordination is batched forward.

## Sequencing constraint that shapes everything

`fast-follow 03` stop condition: *map/layout changes after production capture
has begun ⇒ discard and restart under a new id.* The scoring program's `goal`
block is **required** by the schema (`refwork-featuremap/src/lib.rs:351`,
`pub goal: Goal`), and the goal feature (credits/completion flag) is **not
discoverable from discovery-01** (the trajectory ends at world-2 stage-1).
Therefore the credits-discovery mini-session (STOP #1) must precede map
finalization, which must precede the corpus export (STOP #2). Do not
reorder; do not start the production capture against a draft map.

## Grounding notes

Verified against source on 2026-07-15 (file references for the executing
agent):

- `ramdiff` subcommands and flags: `crates/ramdiff/src/main.rs` — `record`
  (`--rom --script --session --mark <frame>=<label> --dump-every --frames`,
  interactive-only: `--interactive --resume --skip-replay-verify
  --output-log --gamepad`), `search` (`--session --width u8|u16le
  --changed A B --unchanged A B --inc A B --dec A B --value N --in L
  --delta D A B`), `candidates` (`--session --context --limit`), `watch`
  (`--addr region:offset --rom --script --width`), `emit` (`--map --name
  --offset --type --stability --region --semantics --description
  --discretize identity|none|bits --force`).
- `ramdiff search` seeds the full candidate set when `offsets` is empty or
  width changes, and intersects/persists across invocations against the same
  `session.yaml` (`crates/ramdiff/src/filter.rs`, `run_search`).
- Scripted replay dumps at `--mark` frames via the same dump path as
  interactive F5 (`crates/ramdiff/src/record.rs`, `run_record` /
  `dump_if_marked`); resume replay-verification machinery exists (commits
  `1b3c263`, `db51a50`).
- `refwork-verify` subcommands (crates/refwork-verify/src/main.rs usage
  block): `trace --captures --map --scoring --labels --out --report`;
  `phase4-score-plan --captures --out --client-batch-prefix
  --first-boss* --goal-positive* --goal-negative* --checkpoint-after-batch
  --restore-control-batch*` (K=32 hardcoded, `phase4_score_plan.rs:13`);
  `phase4-capture-export --endpoint --snapshot --padlog --map --layout
  --bundle --count --cadence --hard-icount-cap --source-ref [--production]`
  (framebuffer always on; count/cadence/cap must be positive, no 1,000 floor
  in the binary itself — the floor is in the pipeline script);
  `phase4-layout`, `phase4-artifact-check`, `phase4-bundle-check`,
  `phase4-checksum-manifest` (exactly one of `--out | --verify |
  --set-payload-root`), `phase4-context-export/-check`, `map-check --rom
  --map --script --expect` (no native `--report`; the pipeline wraps it),
  `redaction-scan --input [--forbid --forbid-file]`, `play`, `double-run`.
- `trace` label file schema: YAML `kind: phase4-trace-labels`,
  `schema_version: 1`, `labels: [{capture_id, expected_highest_stage?,
  prune?, goal?, first_boss_coverage?, active_stages?}]`; **every capture row
  in the index must have a label** (`phase4_trace.rs` emit_rows: "no label
  for capture_id" is an error). Capture rows need `capture_id`,
  `frame_index` (or `frame_counter`), `decoded_order`, `decoded_values` in
  feature-map order.
- `refwork-featuremap validate <map.yaml> [--scoring <scoring.yaml>]`
  (`crates/refwork-featuremap/src/main.rs`).
- `tools/m6-session-pipeline.sh` stages/flags as documented in its usage()
  and SESSION-DAY-RUNBOOK §4; `map-check`/`score-plan` wrapped-report notes
  are in the stage `--help` text.
- `tools/m6-discovery-analyze.sh` hard-fails on missing labels
  (lines 199–208); its scratch-session, exclusion-set, and report patterns
  are reused by package 03.
- `expectations.yaml` schema for map-check:
  `crates/refwork-verify/src/expectations.rs` (assertions: exactly one of
  `at_frame|by_frame` + one of `equals|changes_to|delta`; `never` clauses).
- Padlog format: `padlog v1` header + one 4-hex-digit pad word per line
  (`crates/refwork-script/src/lib.rs`); `interactive.padlog` = 45,231 lines
  = header + 45,230 frames, **frame i is line i+2**; no `rom=` header line
  present in this session's log.
- Session state: 16 dumps, each 131,072 B; `session.yaml`
  `candidates.offsets: []` (analysis not yet run); `log_frames: 45230`.
- Environment: `target/release/{ramdiff,refwork-verify}` built;
  `refwork-featuremap` release binary NOT built (pipeline falls back to
  `cargo run`); `b3sum`, `jq` present; **python3 lacks `yaml`** (pipeline
  `layout` stage review needs it — package 01 prereq); `bd` present at
  `/usr/local/bin/bd`, `bd create --id` flag exists per `--help`.
- Scorer side: `state-scorer` resolution + spec ratifications + joint-smoke
  runbook read; their beads DB also lost.

**Unconfirmed — verify at execution time** (do not treat as fact until
checked; items marked RESOLVED/CORRECTED below are now settled and are kept
in place only to preserve the numbering other packages cite):

1. `bd init` behavior over an existing `.beads/` (config/metadata present,
   DB absent), and `bd create --id` acceptance of the exact historical
   suffixes (`refwork-20v` etc.). Flag exists; semantics unverified.
2. **RESOLVED (2026-07-16, source-verified — no longer unconfirmed):**
   `phase4-capture-export` resume is same-padlog / same-source-ref /
   same-cadence only. `phase4_capture_export.rs` `validate_resume` (~line
   489) rejects any index row whose `node_ref != opts.source_ref`, and
   `validate_resume_frames` (~lines 501–514) enforces single-cadence
   continuity. A second padlog can NOT append into the same bundle —
   package 06 step 4 commits to the sibling-bundle branch accordingly.
3. Exact dump-filename sanitization for replay `--mark` labels (observed
   pattern from originals: spaces/commas → underscores; read the derived
   `session.yaml` instead of assuming filenames).
4. **CORRECTED (2026-07-16):** the ≥1,000 floor does NOT live in
   `phase4_artifact_check.rs` (no minimum-count threshold there) — it lives
   in the pipeline capture stage (`tools/m6-session-pipeline.sh`,
   `--count must be >= 1000`). Direct binary invocations (e.g. the
   package-06 alignment probe) have no floor; pipeline-driven production
   capture does.
5. READY-snapshot ↔ padlog frame-0 alignment (the padlog was recorded on the
   host-side core from power-on; the exporter replays from the deployed
   worker's READY snapshot). Package 06 step 2 probes this before any
   production capture.
6. `refwork-verify play` `--watch/--snap` flags are taken from the usage
   block only (module not read); available as an optional aid during offset
   confirmation if the executor wants a live view (no package requires it).
7. That the discovery-01 root is the ASK1-approved private root (assumed —
   it is where the approved session landed); reconfirm at STOP #1.
