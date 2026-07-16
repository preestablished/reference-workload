# Package 05 — Closeout & Gate Verification (Last)

## Steps

1. **Update the M6 gate checker's CANDIDATE PATHS** in
   `tools/m6-gate-check.sh` with the real (public-safe) artifact
   locations from package 04 — the corpus/session/fallback paths it
   should probe. Keep fail-closed semantics: probe for the specific
   manifest/marker files the freeze produced, not bare directories.
   The path must not leak private naming; use the opaque bundle root.

2. **Re-run `tools/m6-gate-check.sh`** — expected: 4/4 PASS (or 3/4 +
   branch=first-room-fallback with its reduction warning, if that branch
   was selected). Paste the output verbatim into
   `.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md`
   (M6 plan package 01 step 3 — that file's format governs), naming the
   branch, the scorer build SHA available, and resulting M6 scope.

3. **Bead hygiene:** czi/20v/5tk closed with evidence (or 5tk honestly
   annotated if only the raw session exists); comment `refwork-5be`
   that its gate is open, with the GATE-RECORD pointer.

4. **Fast-follow bookkeeping** belongs to the fast-follow (packages
   06–08: context fixture, fulfillments, resolution) — do not close its
   records from here; if executing them, do so under that plan, not this
   one.

5. **Operator ask 3 — push.** Main now carries the czi commit plus plan
   and tooling commits, all unpushed. Ask the operator to approve
   `git push origin main` (explicit approval required). List the commits
   being pushed.

6. **Hand off to the M6 plan.** With GATE-RECORD.md in place, execution
   continues under `.agents/plans/phase4-m6-scoring-goal-integration/`
   packages 02 (joint half) through 08 — that plan's gates now hold.
   Say so explicitly wherever the closeout is reported.

## Exit signal

Gate checker green (per selected branch); GATE-RECORD.md written; beads
consistent; push approved-and-done or explicitly deferred; M6 plan
unblocked and named as the next executable unit.
