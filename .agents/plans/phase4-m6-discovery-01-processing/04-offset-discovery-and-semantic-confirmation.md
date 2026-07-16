# Package 04 — Offset Discovery & Semantic Confirmation (Agent-Only)

## Goal

Run the package-03 analyzer to a unique, **semantically confirmed** offset
for every feature the real scoring program will reference, with honest
stability classifications. This executes the discovery half of
`close-m6-entry-gates/03-close-refwork-20v.md` ("Agent does" list) using the
discovery-01 material; the map-authoring half is package 05.

## Steps

### 1. Narrowing loop

```sh
tools/m6-discovery-analyze2.sh --plan "$PR/discovery/frame-plan.yaml" \
  --out "$PR/discovery/analysis-report.txt"
```

Iterate on the plan file until each feature's count is 1 (or a small set with
a clear winner from hexdump context):

- Ambiguity (>1 candidates): add `--unchanged` clauses from unrelated anchor
  pairs — with 16 anchors + derived dumps there are many cheap constraints
  (e.g. a stage-id candidate must be unchanged across 5230↔5375 AND across
  idle-a↔idle-b AND across 19276↔23130). Every candidate for a
  stable-expected feature that also appears in the idle exclusion set is
  suspect (the analyzer warns, same as the original tool).
- Emptiness (0 candidates): wrong width hypothesis (try the other width),
  wrong encoding (try `--delta`/`--value` clauses — e.g. a damage event with
  a known visual magnitude, or BCD-coded values where `--changed` still works
  but `--inc/--dec` mislead), or wrong window (the state may latch at a
  different frame than the anchor — widen with grid marks per package 03
  step 1).

### 2. Semantic confirmation with `ramdiff watch` (required per entry)

For every surviving candidate (IMPLEMENTATION-PLAN risk table: "`ramdiff
watch` semantic confirmation required per entry"):

```sh
target/release/ramdiff watch --addr wram:<offset> --width <u8|u16le> \
  --rom "$ROM" --script "$SESS/interactive.padlog" \
  > "$PR/discovery/watch-<feature>.log" 2>&1
```

`watch` prints decoded values — **always redirect to a private file, never
stdout.** Confirm against the trajectory's known event frames: stage id
changes at ≈14452/24358/29241/39966 and nowhere mid-stage; health drops at
≈5375; boss/progress flags latch once inside 19276→23130 and 30101→37080 and
never unlatch; upgrade state changes once inside 39966→41511; world index
changes between 3800 and 39085. A candidate whose change-frames don't match
its story is wrong no matter how clean its search count was.

### 3. Stability evidence (binding rule — do not shortcut)

SESSION-DAY-RUNBOOK §3: never mark a field `stable` off one trace; **a
restored state must be re-dumped and confirm it before ANY field is marked
stable.** Deterministic replay of the same padlog is NOT a restore — it
reproduces the identical trace and adds zero evidence. Discovery-01 contains
no restore, so nothing in this package can license a final `stable` mark.
Evidence classes 1 and 2 below yield **`stable (PROVISIONAL)` only**; the
upgrade to `stable` happens exclusively after STOP #1, when the Run C
reload/restored full-WRAM dumps confirm the field (package 05 step 2 item 3
covers EVERY proposed-stable field, not just deferred ones). What counts as
provisional evidence, in order of preference with what this session offers:

1. **Cross-context persistence within the trajectory:** the field holds its
   value across stage-exit → overworld → stage-entry boundaries and across
   menu/pause excursions (verifiable from the watch logs — e.g. upgrade
   flags persisting from 41511 to end-of-log through any context switches;
   max-health persisting from 12730 onward). Record the exact frame windows
   cited.
2. **In-trajectory reload events** if the padlog contains any (stage restart
   after the damage event, continue screens): check the watch log around
   them.
3. If neither yields evidence for a field the scoring program depends on,
   the field cannot even be PROVISIONAL — mark it `volatile` or defer the
   feature and say so in the report; its entire stability case waits for
   STOP #1.

Either way, every `stable (PROVISIONAL)` mark is an input to STOP #1's
item 3 (package 05): the mini-session's reload/game-over segments provide
the restored-state re-dumps that SESSION-DAY-RUNBOOK §3 requires, and only
that confirmation upgrades PROVISIONAL → `stable` in the final map.

### 4. Findings record

Extend `$PR/discovery/analysis-report.txt` (or a `findings.yaml` beside it)
per feature: offset (private), width/type, semantics, proposed
stability + the evidence class from step 3, discretize plan, the draft
`ramdiff emit` line (the analyzer emits these), and open doubts. Explicitly
list the two known-undiscoverable items — `credits_flag` and the dead-state
`game_mode` value — as inputs to STOP #1.

## Acceptance criteria

- Every feature needed for the package-05 scoring program (stage ids /
  world index / upgrade flags / boss-progress flags / health / mode; plus
  position x/y for novelty discretization) has exactly one confirmed offset
  with a watch log whose change-frames match the trajectory story.
- No field is marked plain `stable` by this package. Every stability
  proposal is at most `stable (PROVISIONAL)` citing step-3 evidence class 1
  or 2; anything else is `volatile` or deferred — no exceptions. The final
  upgrade to `stable` happens only after Run C restored-state confirmation
  (package 05 STOP #1 item 3).
- The undiscoverable list (credits, dead value, anything else that emerged)
  AND the full proposed-stable list (for Run C confirmation) are written
  down for STOP #1's briefing.
- Nothing under `discovery-01/` was modified; no offset or decoded value
  appeared on stdout or in any file under the checkout.

## On failure / STOP marker

- Position x vs y cannot be disambiguated from hexdump context: add one
  axis-isolated derived pair — the padlog likely contains vertical-only
  motion (climbing/jumping windows; find Up/Down-held frames, bits 6/7 =
  `0040`/`0080`) — before falling back to any operator ask.
- A scoring-critical feature stays ambiguous after exhausting derived-dump
  constraints: **STOP-AND-COORDINATE (fold into STOP #1, package 05)** — add
  a targeted isolation segment to the mini-session brief rather than
  scheduling a separate ask. Batch, don't trickle.
