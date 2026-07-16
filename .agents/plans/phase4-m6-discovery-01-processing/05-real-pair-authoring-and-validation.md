# Package 05 — Real Pair Authoring, Validation, Close `refwork-20v`

## Goal

Author and validate the real private `feature-map.yaml` +
`scoring-program.yaml` pair and `layout.json`, then close `refwork-20v`.
This executes `close-m6-entry-gates/03-close-refwork-20v.md` and fast-follow
03 steps 1–6 with discovery-01-specific facts. Spec note: the three API.md
questions are **ratified** (2026-07-12, state-scorer
`05-refwork-spec-ratification.md`), but the stamps never landed in API.md —
package 02 step 4 re-applies them. Confirm the three stamps are present
before authoring, then author directly against the re-stamped API.md; do not
reopen the decisions.

## Why there is a STOP in the middle

The scoring schema **requires** a `goal` block
(`refwork-featuremap/src/lib.rs:351`), and the goal feature (credits /
completion flag) plus the dead-state `game_mode` value are not discoverable
from a trajectory that ends at world-2 stage-1 (package 04's gap list). And
per fast-follow 03's stop condition, the map cannot change after production
capture begins. So: finalize the map only after the mini-session below, and
only then run package 06.

## Steps

### 1. Draft (agent, before the STOP)

Under `$PR/bundle-draft/` (NOT `$PR/bundle` — the pipeline's real bundle root
stays empty until finalization):

- `feature-map.yaml`: structure from `feature-maps/demo-game.yaml` (schema 1,
  `regions: [{name: wram, size: 131072}]`), offsets/types/stability from the
  package-04 findings, real `meta.game_revision` (private). Use
  `target/release/ramdiff emit --map ... --force` for each discovered feature
  (draft lines are in the analysis report), then hand-edit grid/threshold
  discretize blocks (emit accepts only `identity|none|bits` — verified).
  Stability literal for `stable (PROVISIONAL)` fields in the DRAFT: emit
  them as `volatile` (emit's `--stability` accepts only `stable|volatile`);
  upgrade to `stable` only at finalization after the Run-C confirmation in
  STOP #1 item 3. The draft never leaves `bundle-draft/`.
- `scoring-program.yaml`: same shape as `scoring/demo-game.yaml` (stages
  `monotone: true` list / shaping / penalties / goal — state-scorer's
  compiler loads this exact shape; don't diverge gratuitously). Stage design
  rule learned from this trajectory: **stage predicates must sit on latched,
  monotone state** (progress/boss flags, upgrade flags, max-resource count),
  NOT on fluctuating ids (stage id and world index go up and down as the
  player moves between overworld and stages) — otherwise gate-3 monotonicity
  fails by construction. Draft stages, mirroring demo names where sensible:
  left start area (first latched-progress evidence), first upgrade
  (capacity pickup or equipment flag bit), midboss, world-1 boss, world-2
  reached (only if a latched flag exists for it), credits (goal, feature
  TBD at step 2). Penalties: prune on the dead `game_mode` value (TBD at
  step 2).
- Dry-run structural validation of the draft with a temporary goal/penalty
  referencing a discovered latched flag, clearly headed
  `# DRAFT-NOT-EVIDENCE — temporary goal for schema dry-run only`:
  `cargo run --quiet --locked -p refwork-featuremap -- validate <map> --scoring <scoring>`.
  This draft never leaves `bundle-draft/` and is never cited as validation.
- Draft `map-check` expectations (`expectations.yaml` schema:
  `crates/refwork-verify/src/expectations.rs`): assertions at the 16 anchor
  frames using `at_frame`/`by_frame` + `equals`/`changes_to` on decoded
  values read from the watch logs (values go in the private file only), plus
  `never` clauses (e.g. dead value never observed... only after step 2
  defines it — leave `never` for finalization).

### 2. STOP-AND-COORDINATE #1 — operator mini-session ("Run C")

> **STOP. Do not proceed to step 3 without this.** One batched ask, per the
> close-m6 operator-involvement model. GATE-RECORD-ASK1 already approved the
> full-corpus branch including a credits/late-game fixture; this schedules
> the remaining segments. Brief the operator with the package-04 gap list:
>
> 1. **Credits / completion evidence** — per the ASK1 deferred checklist the
>    source is the operator's call: hand-play to credits, or
>    operator-provided late-game save RAM loaded on the host core. Needed
>    dumps: pre-credits and during/post-credits (F5-labeled), same ROM.
> 2. **Death / game-over** — a short segment reaching the dead state
>    (pre-death, dead-screen dumps).
> 3. **Reload/restored-state evidence** — reload the same save/continue
>    twice and dump the same location twice, for **EVERY field package 04
>    proposed as `stable (PROVISIONAL)`, plus every deferred field** — not
>    just the deferred ones. SESSION-DAY-RUNBOOK §3 requires a re-dumped
>    restored state before ANY field is marked stable; only this
>    confirmation upgrades PROVISIONAL → `stable` in the final map.
> 4. Any ambiguous-feature isolation segments package 04 flagged.
> 5. **Decision item — pre-existing pushed privacy leak:**
>    `tools/record-ramdiff` hardcoded the private-root location; the working
>    tree is fixed (package 02 step 6, normal commit), but the literal
>    remains in commit `5b35113`, already on `origin/main`. Whether to
>    rewrite pushed history (with its coordination cost) or accept-and-record
>    the historical occurrence is the operator's call — record the decision;
>    never rewrite pushed history unilaterally.
>
> **Save-RAM caveat (state it in the brief so the operator can weigh it):**
> if the credits source is operator-provided save RAM loaded on the host
> core, the `discovery-02` padlog will NOT replay from `Core::new` (the
> initial save RAM lives in neither ROM nor padlog). Run C evidence is then
> limited to **direct F5-dump search only** — no replay-derived
> confirmation: no `ramdiff watch` over the discovery-02 padlog and no
> second map-check pass (step 3 note) — unless an equivalent initial-state
> mechanism exists for replay. Hand-play from power-on keeps the full
> replay-derived toolset available.
>
> **Verify before the operator leaves (session-completeness checklist,
> mirrors package 01 step 2):**
>
> - every briefed dump present: pre-credits, during/post-credits, pre-death,
>   dead-screen, the reload/restored pair per item-3 field, and each item-4
>   isolation segment;
> - every dump exactly 131,072 B (`wc -c`, `sort -u`);
> - padlog sanity: `padlog v1` header + a positive frame count that matches
>   the session's `log_frames`;
> - `discovery-02/session.yaml` lists every briefed dump;
> - reconfirm with the operator that the discovery-01 root (and this
>   session's root) is the ASK1-approved private root (grounding note 7's
>   deferred check).
>
> **Resume signal:** proceed past this STOP only when the operator confirms
> the session is done AND the checklist above passes against
> `discovery-02/session.yaml`. A missing dump discovered after the operator
> leaves is a second ask — exactly what this checklist exists to prevent.
>
> Batching note (00-overview): this same ask may also INITIATE STOP #2's
> coordination — stack redeploy and Play-window agreement are operator/bridge
> work that can proceed in parallel while the agent processes Run C.
> Sequencing is unchanged; STOP #2's checks must still all hold before any
> worker traffic.
>
> Mechanics: `cargo run --release --features interactive -p ramdiff -- record
> --interactive --rom <private> --session "$PR/ramdiff/discovery-02"` — the
> operator plays and F5-labels; everything downstream is agent-only (same
> division as SESSION-DAY-RUNBOOK §1). Record the credits-fixture source
> decision in `$PR/evidence/` (never publicly).
>
> **Fallback branch (only if the operator explicitly defers Run C):** the
> pair cannot carry a truthful credits goal. Either hold 20v open (default),
> or — operator decision, recorded — ship a reduced pair whose goal uses the
> most-final discovered latched flag, with the reduction propagated
> everywhere: gate-3 "fires-on-credits" undeclarable, resolution names the
> blocker (mirrors the scoring-goal plan's fallback discipline). Never
> soften.

After Run C, **before any replay-derived use of the discovery-02 padlog**
(`ramdiff watch`, the second map-check pass), run a package-01-style
byte-exact replay-fidelity mini-gate over `discovery-02`: replay its padlog
with `--mark` at each of its labeled frames into a fresh sibling session and
`cmp` every replayed dump against the F5 originals (same mechanics as
package 01 step 5; evidence to `$PR/evidence/replay-fidelity-02.txt`). If
Run C used the save-RAM route, this gate cannot pass by construction — skip
replay-derived confirmation entirely and record the evidence reduction (the
save-RAM caveat above).

Then run package-04 discovery mechanics over `discovery-02` for
`credits_flag`, the dead `game_mode` value, the PROVISIONAL→stable
confirmations (every proposed-stable field, item 3), and any isolation
segments — dump-search always; watch only on a passed mini-gate.

### 3. Finalize and validate (agent)

Move the completed pair into `$PR/bundle/` and run the pipeline stages
(SESSION-DAY-RUNBOOK §4 stages 1–3; flags verified against
`tools/m6-session-pipeline.sh`):

```sh
tools/m6-session-pipeline.sh validate-map --private-root "$PR"
tools/m6-session-pipeline.sh map-check    --private-root "$PR" \
  --rom "$ROM" --script "$SESS/interactive.padlog" \
  --expect "$PR/bundle/validation/map-check.expect.yaml"
tools/m6-session-pipeline.sh layout       --private-root "$PR" \
  --capture-spec-hash <opaque-ref>        # --exporter-commit defaults to 2827665
```

Notes: map-check runs host-side against the real ROM + the session padlog —
fully agent-side; its report is the pipeline's wrapped JSON (no native
`--report`, as the stage `--help` documents). The layout stage's mechanized
independent review needs pyyaml (package 01 prereq).

A second map-check pass against the Run C padlog covers the credits/dead
assertions — **only on the power-on branch, with the discovery-02
replay-fidelity mini-gate passed** (on the save-RAM branch there is no valid
replay; the credits/dead evidence is the direct F5-dump search, and record
that reduction). Mechanism for the second pass — the pipeline guards
`validation/map-check.json` against overwrite, so do not improvise: check
`tools/m6-session-pipeline.sh map-check --help` first. Preferred: direct the
wrapped report to a distinct path (`validation/map-check-runc.json`) via the
stage's report-path option if it has one. If the stage hardcodes the report
path and offers only a `--force`/overwrite rerun, first preserve the pass-1
report as `validation/map-check-run1.json`, then rerun with the Run C
expectations. Either way both reports survive — package 08 cites both.

### 4. Close the bead

Fast-follow 03 exit criteria all hold (its list governs). Then:
`bd close refwork-20v -r "<evidence summary — opaque refs and report
statuses only, no offsets>"`, and re-run `tools/m6-gate-check.sh` — expect
4/4 PASS now (branch=raw-session). Write
`.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md` per that
plan's package 01 step 3 (verbatim checker output, branch, scorer build SHA,
resulting scope).

## Acceptance criteria

- `validate-map`, `map-check`, and `layout` stages all PASS (pipeline
  `status` shows it) — map-check over both padlogs on the power-on Run C
  branch (both reports preserved — in practice via the preserve-as-
  `map-check-run1.json`-then-rerun fallback, since the stage hardcodes its
  report path; step 3 has the mechanics), over discovery-01 only on
  the save-RAM branch with the evidence reduction recorded; layout review
  report clean (in-bounds, order, total_len, no demo-map hash/offset
  reuse — the review script checks all of this mechanically).
- Every stage predicate + goal + prune references only features with
  confirmed offsets and honest stability; every `stable` in the final map
  is Run-C-confirmed (PROVISIONAL upgrades recorded per package 04 step 3);
  goal is credits-truthful (or the recorded fallback branch applies,
  everywhere).
- `refwork-20v` closed; GATE-RECORD.md written; gate checker 4/4.

## On failure

- `validate-map` errors: fix the artifact (schema/pair rules per API.md §1–2);
  if the spec itself is wrong, the spec-ownership rule applies (fix API.md,
  record in both packets — scoring-goal package 02).
- `map-check` assertion failures: the expectation, the offset, or the frame
  anchor is wrong — reconcile against the watch logs; never nudge an
  expectation to force a pass.
- Layout review FAIL on demo-offset collision: a real offset may genuinely
  collide with a placeholder value by coincidence — the review flags it;
  document the coincidence in the private report and re-run review only if
  the check supports an explicit waiver; otherwise treat as suspect
  discovery and re-verify that feature.
