# Resolution (M6) — interim branch, 2026-09-14

Filed by the discovery-02 processing plan
(`~/.agents/projects/reference-workload/plans/discovery-02-processing-and-interim-scoring/`, WP6).
Acceptance baseline: owner IMPLEMENTATION-PLAN §M6 Accept bullets (adopted by
reference), read together with the 2026-09-13 interim amendment under
phase-4 exit-gate item 3.

## Identifiers
- reference-workload SHA: 30fe9d1 + the review-fix commit on branch `discovery-02-processing` (recorded in git; the clean re-run below was built at 30fe9d1 — the fix commit changes report fields only, indexes and traces are byte-identical, re-verified)
- scorer build SHA: `0.1.0+b6662ad` (git `b6662ad1c90aaec04fc6ef9aa1edc5b8ad27d318`), engine grpc, `bytes_source: raw` — live evaluation 2026-09-14 (2026-09-13: trace-only); evidence: state-scorer `docs/evidence/exit-gate/2026-09-14-interim-goal.md`
- Loaded map hash: `blake3:b811fabc28f7daddce8dd44b011bec0e788adc5fa8a76de4e67fd266534081a4`   program hash: `blake3:b11b07420d75b87ce57c0e1901fa9f2e39cef5a03d5f3c7f6a58a8dbc564f0da`
- Corpus id: **none — corpus deferred** (see below)
- Labeled-trajectory hash: `blake3:220e05725c9f3305dbf24048e95e1cef911dbca039dea64438c56b9be0d8cb9b` (`$PR/bundle/trajectory/first-boss.jsonl`)
- Sidecar: `$PR/bundle/validation/handoff.b3` blake3 `blake3:39aefe5a4602b2c15f463f1f01f367bd463d6837d6eb3d6326cf60be9f88884a`

## Entry-gate branch
raw-session, per GATE-RECORD.md (2026-09-13, first pass and post-WP5 entry):
`discovery-02-main` (57,064 frames, 34 labeled dumps, emulator epoch
refwork-emu 0.2.3) via `~/.agents/projects/reference-workload/handplay-session`.

## Bead states
- refwork-otv (epic) with WP1–WP6 children: WP1–WP5 closed, WP6 (refwork-bes) closes with this file.
- refwork-20v closed 2026-09-13 (pair v3 validated). refwork-czi closed (earlier).
- refwork-1n8 closed (migration step superseded by discovery-02 re-confirmation; residue below).
- refwork-5tk open — corpus deferred (comment posted). refwork-5be commented (handoff delivered).
- 2026-09-14: refwork-5be closed (live scorer evaluation recorded, see Identifiers).
- refwork-ob3 open — items 4–5 gated (comment posted). refwork-8n5 commented (session consumed).

## Validation results (the four)
1. Compile + hand-score joint validation: this side validated the pair
   (featuremap validate PASS; map-check PASS on three padlogs: main 759
   assertions + 1 never-clause, death 250, timer 181 + 1 never); scorer-side
   compile + evaluation done 2026-09-14 by state-scorer `trajeval` (build
   `0.1.0+b6662ad`, engine grpc): main/death/timer PASS ×3, zero item
   errors, loaded map/program/layout hashes equal to the handoff's,
   `feature_bytes_len` 27.
2. Scripted-trajectory checkpoints: not re-run under this plan (the hand-played
   discovery-02 trajectory replaces the scripted one for gate 3; the July
   scripted checkpoints were pre-epoch).
3. Gate-3 fixture (interim form): monotonicity **PASS**, goal-iff-latch **PASS**
   (world-1-clear latch, both directions on 1,302 captures), prune on the
   death/timer negatives **PASS** — trace-only on 2026-09-13, confirmed by
   the live scorer run of 2026-09-14; see GATE3-CLAIMS.md.
4. Fixture corpus + budget: **not run** — no corpus (below).

## Self-verification matrix (package 08)
1. Clean-checkout re-run: a fresh `git worktree` at 30fe9d1, release build
   from an empty target dir, recorded `host-capture-index` + `trace` commands
   re-run for all three sessions → byte-identical to the frozen artifacts:
    index main: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    blobs main: IDENTICAL (1302 files)
    index death: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    blobs death: IDENTICAL (262 files)
    index timer: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    blobs timer: IDENTICAL (793 files)
    trajectory first-boss: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    trajectory negative-death: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    trajectory negative-timer: IDENTICAL (sha256 in $PR/evidence/rerun-30fe9d1-sha256.txt)
    mismatches: 0
2. Re-evaluation from frozen artifacts: the gate-3 property script re-run over
   the re-produced (identical) trace rows reproduces every boolean in
   `gate3-eval-summary.json` (monotone, goal-iff-latch, prune) — trace-only,
   no scorer build involved.
3. Corpus id / FULFILLMENT cross-check: no corpus id exists; FULFILLMENT is
   `partial` (`~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`).
4. Smoke evidence: **smoke not run** (needs the stack; items 4–5).
5. Gate 3 declarable verbatim: **only in the interim form** of the 2026-09-13
   amendment; the credits form is not declared.

## Smoke
Not run. Bursts 0, Faults n/a, spot-replays n/a, mismatches n/a; no window.

## Deferred corpus capture
The ≥1,000-state `(wram, fb)` capture corpus and its freeze (`refwork-5tk`,
prior plan package 06) did not run. Prerequisites still open: the
dh-workerd/snapstore stack does not exist on this Mac and must be redeployed
with builds re-baselined to emulator epoch 0.2.3 (hypervisor icount fixtures,
snapstore snapshot versions); a Play-window agreement; an operator-signed
`--hard-icount-cap`. When those hold, the prior plan's package 06 runs
unchanged against map v3, then `trace` re-runs over the real corpus index
and GATE3-CLAIMS is upgraded to `full-corpus`.

## Epoch-rollout residue carried from refwork-1n8 (operator items)
- Lab expect-file regeneration under 0.2.3 (epoch-cut bill item 3).
- Sibling programs' icount re-baseline (bill item 4) before any capture is trusted.
- `integration.rs` frame_ctr pin verification; package-06 re-stamp; doc
  updates for old corpus-id references.
- 2026-09-14: frame_ctr pin verified unchanged at `3afde31` / emulator 0.2.3
  (`map_check_positive`, `map_check_negative_wrong_value` PASS; recorded in
  `$PR/evidence/frame-ctr-pin-023.txt`). The synthetic ROM exercises no APU
  timing, so this closes the bookkeeping item only. A grep for old corpus-id
  references found none (no July corpus id was ever minted).

## Operator decisions (resolved 2026-09-14 unless noted)
- Commit `5b35113` (private-root literal in pushed history): **left as-is
  (operator, 2026-09-14)** — no history rewrite; a later cleanup is tracked by bead `refwork-1ls`.
- Wording of `.agents/decisions/2026-09-13-interim-goal-and-score-shaping.md`:
  **confirmed (operator, 2026-09-14)**.
- Stability asserted from single-trace evidence only (`level_timer_d100/d10/d1`;
  constant-in-all-sessions digits `score_d100k`, `score_d10`, `score_d1`,
  `currency_d100`): **accepted as-is (operator, 2026-09-14)**; none is used by
  the v3 program — see GATE3-CLAIMS.md and the private CHANGELOG-v3.md.

## Handoff surface
No WorkloadImage registration (unchanged). Scorer handoff:
`~/.agents/projects/reference-workload/requests/discovery-02-scorer-handoff/HANDOFF.md`
+ `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md` (2026-09-13 M4 slot).

## Gate assessment
Gate 3: **declared in the interim form only** (world-1-clear latch;
fires-on-credits UNDECLARABLE; evaluated live by state-scorer
`0.1.0+b6662ad` on 2026-09-14). Evidence contributed to gates 1–2: none new
(the live scorer run evaluates gate 3 only; no real-state dedup run).
Cross-references: scorer packet resolution
`~/git/preestablished/state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md`
(items 1–2, 4, 5-window remain two-sided).
