# Package 06 — Corpus Capture, Validation, Freeze, Close `refwork-5tk` (OPERATOR-GATED)

## Goal

Produce and freeze the ≥1,000-state real capture corpus from the discovery-01
trajectory (+ Run C credits segment) via the deployed worker, then close
`refwork-5tk`. This executes `close-m6-entry-gates/04` and fast-follow 04/05
through `tools/m6-session-pipeline.sh` stages 4–8; those runbooks govern
mechanics — this package adds only the discovery-01-specific parameters,
the alignment probe, and the two-bundle composition rule (the multi-padlog
question is resolved: a second padlog cannot append — see step 4).

## STOP-AND-COORDINATE #2 — stack + window (before any worker traffic)

> **STOP.** All four must hold, coordinated with the operator:
>
> 1. **Stack up.** The bridge unit → dh-workerd `6e348e5` → snapstore
>    `~/.rbo73/m4-regen-20260707/` stack **may be down after the recent
>    teardown** (working tree preserved on `backup/m6-host-20260715`).
>    Run SESSION-DAY-RUNBOOK §0 step 1's checks verbatim; if the stack needs
>    redeploying, that is operator/bridge work — record the actual worker
>    build ref and READY snapshot ref (`BRIDGE_REAL_SNAPSHOT_REF` from
>    `real.env`, expected `948b73e6` or recorded successor) in private
>    evidence.
> 2. **Window.** `rom-operator-bridge-l1w` was open as of 2026-07-12: the
>    capture run must not overlap live Play. Re-check the bead (its repo's
>    DB may also have been lost — then ask the operator directly) and record
>    the agreed window (SESSION-DAY-RUNBOOK §0 step 2).
> 3. **Worker provenance / caps.** Confirm provenance includes hypervisor
>    `c0337ab` or later, else keep bounded-run caps (fast-follow 01 step 3);
>    `--hard-icount-cap` is mandatory input either way (the pipeline refuses
>    to default it).
> 4. **`--hard-icount-cap` value (operator input; agent proposes a derived
>    value).** Derivation rule: cap = total requested frames × an observed
>    per-frame instruction count taken from a prior recorded
>    capture-export/replay run (look in the m4/m5 evidence records for an
>    observed icount-per-frame figure), with generous headroom (×4). Both
>    failure directions matter: too low aborts a production run mid-capture
>    and wastes the window; too high defeats the cap's runaway-safety
>    purpose. Record the chosen value and its derivation in private
>    evidence; the same rule sizes the step-2 probe's cap (scaled to its 8
>    captures).

## Steps

### 1. Preconditions

`refwork-20v` closed (package 05); `$PR/bundle/` holds the final
`feature-map.yaml`, `scoring-program.yaml`, `layout.json`. The map is now
frozen: any change after this point discards captured rows (fast-follow 03
stop condition).

### 2. Alignment probe (before production capture — new, discovery-01-specific)

The padlog was recorded from host-core power-on; the exporter replays it from
the worker's READY snapshot. If snapshot frame-0 ≠ host frame-0, every
captured row is misaligned with the trajectory. Probe with a direct
invocation (the binary has no 1,000 floor — only the pipeline stage does):

```sh
target/release/refwork-verify phase4-capture-export \
  --endpoint unix:///run/dh/grpc.sock --snapshot "$SNAP" \
  --padlog "$SESS/interactive.padlog" \
  --map "$PR/bundle/feature-map.yaml" --layout "$PR/bundle/layout.json" \
  --bundle "$PR/probe-bundle" --count 8 --cadence 600 \
  --hard-icount-cap <N> --source-ref probe-align-01
```

Compare each probe row's `decoded_values` against the same features decoded
from host-side replay dumps at the same frames (this is exactly fast-follow
03 step 7's consumer-side cross-check). The comparison tool is specified,
not improvised:

- **Inputs:** (i) the probe bundle's `captures/index.jsonl` rows —
  `capture_id`, `frame_index`/`frame_counter`, `decoded_order`,
  `decoded_values`; (ii) host-side full-WRAM dumps from `ramdiff record
  --mark` at exactly the 8 probe frames, decoded through
  `$PR/bundle/feature-map.yaml` (same offsets/widths/discretize — reuse the
  package-03 analyzer's decode helpers rather than re-implementing decode).
- **Comparison:** for each probe row, every feature in `decoded_order`,
  value-for-value against the host decode at the same frame.
- **Output:** a per-frame, per-feature PASS/MISMATCH table written to
  `$PR/evidence/align-probe-01.txt` (decoded values are private — never
  stdout), plus a one-line overall PASS/FAIL summary.
- **Acceptance bar:** 8/8 frames with every feature identical; any
  mismatch fails the probe.

Mismatch ⇒
**stop**: snapshot/trajectory identity problem; coordinate (options: a
snapshot whose frame-0 matches power-on, or re-recording the trajectory
against the snapshot — operator decision). Delete `$PR/probe-bundle` after a
passing probe (it is not part of the corpus).

### 3. Production capture

45,230 frames / cadence 45 ⇒ 1,005 captures spanning the whole trajectory
(covers transitions rather than 1,000 near-identical frames):

```sh
tools/m6-session-pipeline.sh capture --private-root "$PR" \
  --padlog "$SESS/interactive.padlog" \
  --count 1005 --cadence 45 \
  --hard-icount-cap <N> --source-ref <opaque-provenance-ref> --production
```

### 4. Credits/goal-positive captures (Run C segment — sibling bundle)

The main padlog contains no goal-positive state, but
`phase4-score-plan` **requires** `--goal-positive` (it dies without one) and
the trace must contain both goal truth values. Grounding note 2 is
**resolved — source-verified, not open** (00-overview): resume in
`phase4_capture_export.rs` is same-padlog / same-source-ref / same-cadence
only (`validate_resume`, ~line 489, rejects any index row whose
`node_ref != opts.source_ref`; `validate_resume_frames`, ~lines 501–514,
enforces single-cadence continuity). A second padlog can NOT append into the
main bundle. Therefore, single branch:

- Export the credits segment to a **sibling bundle** (e.g.
  `$PR/bundle-credits/`) with the Run C padlog, a small count, a cadence
  covering the credits window, and a distinct `--source-ref` — following
  fast-follow 04 step 2's "separately approved snapshot or segment"
  provision.
- **Identity preservation is the invariant:** same worker image, ROM,
  `feature-map.yaml`, `scoring-program.yaml`, `layout.json` as the main
  bundle — record the identity-preservation evidence (map/layout hashes
  identical across both bundles) in the freeze records.
- Each bundle passes its own `phase4-artifact-check`/`phase4-bundle-check`;
  do not invent a merged **bundle** format — the two bundles are frozen as
  cross-referenced checked artifacts, and score-plan/trace consume the
  composed **index** defined in step 5.

Note the Run C padlog only replays from the READY snapshot if Run C started
from power-on on the same ROM — if it used save RAM, the capture needs its
own approved snapshot; surface that at STOP #2, not after.

### 5. Validate, label, plan, freeze (existing runbooks verbatim)

Per SESSION-DAY-RUNBOOK §4 stages 5–8 and fast-follow 04 steps 4–7 / 05:

**Composed index for score-plan and trace (two-bundle composition rule):**
`phase4-score-plan` requires every `--goal-positive`/`--goal-negative`/
`--first-boss` id to be a member of the SAME `--captures` index
(check_labels membership), and `trace` consumes ONE fully-labeled index —
the two bundles' indexes do not compose implicitly. Derive the composed
index explicitly:

- Concatenate the main and credits bundles' `captures/index.jsonl` into
  `$PR/bundle/derived/index-composed.jsonl`, after asserting (scripted,
  result recorded): identical map hash and layout hash across both bundles,
  and fully disjoint capture-id sets. Either assertion failing stops the
  package.
- Record it as a **derived artifact under the corpus lineage rule**
  (scoring-goal package 04: this is the single derivation, its hash is
  recorded in the freeze records against both source indexes; no second
  independently-hashed copy ever).
- If index rows carry bundle-relative payload references, record beside the
  composed index which bundle root each row resolves against — do not
  rewrite rows.
- Feed THIS composed index to `phase4-score-plan` here and to package 07's
  `trace`. Goal-positive ids are credits-bundle rows, now legal members of
  the composed index.

```sh
tools/m6-session-pipeline.sh artifact-check --private-root "$PR"
# author dedup-groups.jsonl (both relation types) + operator labels file
# score-plan runs against the COMPOSED index: use the pipeline stage if it
# accepts an explicit --captures override, else invoke
# target/release/refwork-verify phase4-score-plan directly with the
# pipeline's report-path conventions:
tools/m6-session-pipeline.sh score-plan --private-root "$PR" \
  --captures "$PR/bundle/derived/index-composed.jsonl" \
  --first-boss <capture-id> --goal-positive <capture-id> --goal-negative <capture-id>
```

Discovery-01 specifics for the label/dedup authoring (fast-follow 04
steps 5–6):

- dedup `same_canonical_state`: pairs of captures inside idle runs (package
  03's `idle-runs.txt` gives frames; capture ids follow from cadence);
  `distinct_stable_state`: pairs straddling stage entries / boss defeats.
- "first boss" labeling: decide and record which event the scoring program's
  `first_boss` stage means (the W1-S2 midboss at ≈f19276–23130 is the first
  boss encountered; the W1-S4 boss is the world boss) — one decision, used
  identically in labels, score-plan ids, and the package-07 trace labels.
- Operator reviews the mandatory examples (async, part of the STOP #2
  coordination — not a new session).

Then the freeze: fast-follow 05 verbatim (bundle assembly, `manifest.json`
fields, `phase4-bundle-check` with report OUTSIDE the bundle,
`phase4-checksum-manifest --out` external seal, retrieval re-verify with
`--verify`, redaction scans, one opaque corpus id). Create the gate-check
corpus pointer only now:
`ln -s "$PR/bundle" ~/.agents/projects/reference-workload/corpus`.

### 6. Close the bead

`bd close refwork-5tk -r "<opaque id, capture count, checker statuses,
freeze-manifest hash>"`. M6's entry condition 4 upgrades from
branch=raw-session to branch=full-corpus on the next `m6-gate-check.sh` run
(the corpus probe now hits its marker files).

## Acceptance criteria

- Alignment probe: 8/8 rows match host-side decoding (per the step-2
  comparison spec, table in `$PR/evidence/align-probe-01.txt`) before
  production ran.
- `capture-export-report.json` pass, completed == requested ≥ 1,005;
  `artifact-check.json` pass for BOTH bundles; bundle-check +
  checksum-manifest verify pass **from the retrieved copy** for both
  (fast-follow 05 freeze protocol).
- Credits sibling bundle frozen with the identity-preservation record
  (identical map/layout hashes); composed index derived with the disjoint-id
  and hash assertions recorded, hashed under the corpus lineage rule; the
  score-plan report passes with all three label classes **against the
  composed index**.
- If Run C was deferred: no credits bundle and no composed index — the
  corpus is frozen WITHOUT goal-positive rows and that reduction is recorded
  in the freeze record, the bead, and forwarded to package 07's claim
  split. `phase4-score-plan` cannot run without a goal-positive id — in
  that reduced branch, score-plan is deferred with the same record, not
  faked.
- One corpus id everywhere (the credits bundle and composed index
  cross-referenced under it); `refwork-5tk` closed (or honestly annotated
  per close-m6 05 step 3 if only partially frozen).

## On failure

- Worker 503s / dangling intents: bridge's audited `clear-dangling-intents`,
  note in evidence (SESSION-DAY-RUNBOOK §0).
- Probe mismatch: stop, per step 2 — never "correct" rows post-hoc.
- artifact-check failures: fix the cause and re-export the affected rows per
  the exporter's resume semantics; if the map must change, that is the
  fast-follow stop condition — new id, full restart of this package.
