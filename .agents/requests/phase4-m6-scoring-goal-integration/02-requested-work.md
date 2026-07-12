# Requested Work

## Entry Conditions (Hard Gate — Verify Before Opening)

Do not start this request until **all** of:

1. **scorer M3 closed** (per the phase doc: M6 depends on scorer M2–M3)
   — the DSL compiles this repo's demo program and hash/goal evaluation
   exists. Track via the state-scorer packet's beads.
2. **`refwork-czi` closed** — the exporter committed with its final
   clean-checkout gate recorded.
3. **`refwork-20v` closed** — the real-offset private feature map +
   scoring program validated (the placeholder pair is disqualified for
   every real-capture item below).
4. **A hand-played session artifact exists** — either `refwork-5tk`'s
   corpus (full or the recorded first-room v1 fallback) or, minimally,
   the `ramdiff record` hand-play session the fast-follow's entry
   condition 6 schedules. Be precise about what the fallback branch can
   and cannot do: the fast-follow's first-room-only fallback contains
   **no hand-played trajectory at all** (decode goldens, dedup groups,
   volatile/stable pairs from the scripted first-room log). In that
   branch, item 2 runs against the scripted trajectory's first-room
   prefix only, **item 3 cannot run** (there is nothing to trace for
   later stages or the credits flag), and **Phase 4 exit gate 3 is NOT
   declared** — it stays open, named in the resolution as blocked on
   the hand-play session. Record which branch you took.

Items 4–5 below additionally need **scorer M4** (running service) and a
confirmed-up deployed stack. If you're reading this and condition 1
fails, the work item is the state-scorer packet; if 2–4 fail, it's the
fast-follow.

## Work Items

0. **Tracking.** Create an M6 bead (labels: integration, phase4) with
   dep edges to `refwork-czi`/`refwork-20v` and a note naming the scorer
   packet; children per item if you want finer grain.

1. **Program/map handoff + compile validation (startable at scorer M2,
   before the full gate).** Hand `feature-maps/demo-game.yaml` +
   `scoring/demo-game.yaml` (and, privately, the real pair once 20v
   lands) to state-scorer; their M2 acceptance compiles the demo program
   — jointly confirm compile + hand-computed stage scores match this
   repo's API.md §2.1 intent. Divergences between their compiler and
   this repo's spec are settled by this repo's API.md (we own the DSL
   spec) unless the spec itself is wrong — then fix the spec here. (The
   same rule is recorded in the scorer packet's item 2.) Once their M4
   service runs, this item extends to loading the **real** map/program
   pair into the live service — their exit-gate evidence run (scorer
   packet item 6) requires it; validate the load jointly.

2. **Scripted-trajectory stage validation.** Stage scores on the
   scripted trajectory: leave start area → first upgrade → first boss
   checkpoints score exactly per API.md §2.1, evaluated by the scorer
   (not a reimplementation).

3. **Hand-played labeled trajectory — the gate-3 fixture.** Artifact
   lineage rule first: if the fast-follow's item 3 already emitted the
   traced/labeled trajectory (`trace` + `phase4-score-plan` output),
   **consume that frozen artifact directly** — do not re-derive a second
   independently-hashed trajectory under a different id; only run
   `refwork-verify trace` yourself if the fast-follow handed off the raw
   `ramdiff record` session without the trace step. Either way the
   result is one labeled feature-trajectory JSONL (one record/frame,
   decoded features + stage annotations) under the corpus's single id
   convention. Staged program scores it **monotonically**
   through its stages; goal predicate fires **only** on the credits flag
   (credits save-state via scripted play or operator-provided late-game
   save RAM). This artifact is consumed by Phase 4 exit gate 3 and
   Phase 8 campaign step 1 — version and freeze it with the corpus
   conventions the fast-follow established.

4. **Scorer fixture corpus + budget check.** Supply the 1,000-state
   captured `(wram, fb)` fixture corpus with expected scores; scorer
   evaluates a captured region set in <1 ms/state for this map (their
   budget, your fixture).

5. **Exploration-readiness smoke.** Orchestrator dev loop against the
   image: 1,000 bursts, zero `Fault`s, zero determinism-hash mismatches
   on spot-replayed branches. A trivial random-input loop satisfies the
   plan; using input-synthesizer v1 if closed is better evidence —
   don't gate on it. Confirm stack up first; coordinate the window so it
   doesn't overlap live Play while hypervisor `l1w` verification is
   open.

6. **Handoff surface.** Register `WorkloadImage` with control-plane if
   their resource API exists by then; otherwise ship the manifest +
   `dist/` layout the hypervisor consumes directly, and note the
   registration as control-plane's follow-on.

## Suggested Sequencing (Yours To Overrule)

1 early (at scorer M2 — it de-risks both sides while the gate matures);
then 2 → 3 (same session artifacts) → 4 → 5 → 6. If the operator session
delivers everything at once, 2–4 collapse into one validation pass.

## Acceptance Criteria

Owner IMPLEMENTATION-PLAN §M6's three accept bullets, adopted by
reference, measured with the **real** map/program on real captures
(first-room-fallback reductions recorded explicitly), plus:

- The labeled-trajectory JSONL versioned/frozen with recorded hashes and
  its monotonicity + goal-only-on-credits result reproducible from it.
- Joint items signed both sides: scorer packet's resolution and this one
  cross-reference each other for items 1–2, 4, and 5's window
  coordination. On item 5: your exploration-readiness smoke and the
  scorer packet's M4 joint smoke are **different tests** (yours counts
  `Fault`s/hash mismatches; theirs proves the scoring loop) that may
  share an operator window but must be recorded separately.
- Phase 4 exit gate 3 wording satisfiable verbatim from your recorded
  evidence — hand-play branch only; in the first-room fallback branch
  gate 3 is explicitly not declared (entry condition 4) and the
  resolution names it blocked on the hand-play session.

## Out Of Scope

- Everything the fast-follow owns: exporter, real map *authoring*,
  corpus production/freeze/handoff, FULFILLMENT floor closures,
  operator approvals.
- scorer-side implementation (their packet), synthesizer v1 (theirs),
  orchestrator transport work (`cww`).
- The P2/P3 emulator-perf beads.
- Phase 5 loop work — the phase doc's "only the loop itself is missing"
  is the *next* phase's opening line, not yours.
