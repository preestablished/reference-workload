# Session-Day Runbook — Full-Corpus Hand-Play Session

One page, mechanical. Covers close-m6-entry-gates packages 03/04 (the
combined operator session) executed through `tools/m6-session-pipeline.sh`,
plus the closeout pointers into package 05. Read
`.agents/plans/close-m6-entry-gates/GATE-RECORD-ASK1.md` first — branch and
session shape are already decided (FULL CORPUS, ONE COMBINED SESSION).

Everywhere below, `$PRIVATE_ROOT` is the approved private root (location
given privately at session time, never in this file) and `$BUNDLE` is
`$PRIVATE_ROOT/bundle`.

## 0. Pre-flight checks (agent, before the operator sits down)

1. **Stack up** (fast-follow 01 step 3; plan package 06 step 1):
   ```sh
   systemctl --user status <bridge-unit-name>          # bridge systemd unit alive
   ss -xl | grep -F /run/dh/grpc.sock                  # worker UDS listening
   ss -xl | grep -F snapstore                           # snapstore UDS listening
   ls -d ~/.rbo73/m4-regen-20260707/ 2>/dev/null         # durable snapstore copy reachable
   ```
   Worker + snapstore are user processes and die on reboot — verify now, not
   from memory. Record the actual dh-workerd build ref (expected `6e348e5` or
   successor). If dangling-intent 503s appear, recover via the bridge's
   audited `clear-dangling-intents` subcommand and note it in the evidence.

2. **Window coordination:** `rom-operator-bridge-l1w` (hypervisor RSS-leak
   live verification) was **open as of 2026-07-12** — re-check its status
   before scheduling; if still open, this session must not overlap live
   Play. Agree the window with the operator and record the agreement in the
   session's private evidence.

3. **READY snapshot ref:** confirm `BRIDGE_REAL_SNAPSHOT_REF` in
   `~/.rbo73/m4-regen-20260707/handoff/real.env` still points at the
   approved READY snapshot (`948b73e6` per GATE-RECORD-ASK1.md, or record
   the successor). This is the pipeline `capture` stage's default
   `--snapshot`.

4. **Private root:** confirm it exists, is outside every checkout, and has
   restrictive permissions (fast-follow 01 step 7 — `phase4-private-intake`
   should already have run). Do not create `$BUNDLE` contents before this.

## 1. Run A — offset-discovery hand-play (operator, short segments)

One line: the operator performs `cargo run -p ramdiff --features interactive
-- record --interactive --rom <private> --session <private-dir>` through the
already-briefed label list — `baseline start-a1 start-a2 start-b room2
back-room1 area1 health-full health-hit pre-upgrade post-upgrade dead`
(see `tools/m6-session-pipeline.sh` header comment and
`tools/m6-discovery-analyze.sh` for the exact set) — everything else in this
runbook is agent-only.

## 2. Discovery analysis (agent, existing tool)

```sh
tools/m6-discovery-analyze.sh --session <private-session-dir> \
  --out <private-session-dir>/analysis-report.txt
```

Produces per-feature candidate offsets and draft `ramdiff emit` lines. Never
prints offsets/decoded values to stdout — detail is in the report file only.

## 3. Map authoring loop (agent, iterative)

For each feature with exactly one surviving candidate, emit it into the real
map:

```sh
target/release/ramdiff emit --map "$BUNDLE/feature-map.yaml" \
  --name <feature> --offset <hex> --type <type> \
  --stability <stable|volatile> --semantics <semantics> \
  --discretize <identity|none|bits> --description "<description>"
```

Grid/threshold-style discretization (position, health) is not accepted by
`emit` directly — hand-edit the discretize block into the YAML afterward,
matching `feature-maps/demo-game.yaml`'s shape.

**Stability rule (fast-follow 03 step 2 — binding):** never mark a field
`stable` off one trace. Before marking any field stable, re-dump a restored
state (load the same save/checkpoint again) and confirm the field is
unchanged across the reload, not just unchanged within one continuous
session. A field that only *looked* stable because it wasn't touched in one
trace is exactly the volatile-misclassified-as-stable failure this guards
against.

Author `scoring-program.yaml` alongside it, to the same stage/goal shape as
`scoring/demo-game.yaml` (state-scorer's compiler and M4 service load this
exact pair — do not diverge gratuitously). Settle any open API.md spec
questions (threshold edge inclusivity; `not{}` over a failed `valid_when`
guard; bit-range vs schema strictness) before authoring, per plan package 03
"Additions since the fast-follow was planned".

## 4. Pipeline stages 1–8

Every stage validates its own inputs and is safely re-runnable (`status`
shows progress; a stage refuses to clobber a non-empty prior output unless
`--force` is passed). Full flag reference: `tools/m6-session-pipeline.sh
<stage> --help`.

```sh
PR="$PRIVATE_ROOT"

# 1. validate-map — featuremap validate of the map/scoring pair
tools/m6-session-pipeline.sh validate-map --private-root "$PR"

# 2. map-check — real map-check against the approved ROM/script
tools/m6-session-pipeline.sh map-check --private-root "$PR" \
  --rom <private-rom> --script <private-map-check-script> \
  --expect "$PR/bundle/validation/map-check.expect.yaml"

# 3. layout — generate + independently review layout.json
tools/m6-session-pipeline.sh layout --private-root "$PR" \
  --capture-spec-hash <blake3-or-opaque-ref>
  # --exporter-commit defaults to 2827665 (the refwork-czi commit)

# 4. capture — phase4-capture-export against the deployed worker
tools/m6-session-pipeline.sh capture --private-root "$PR" \
  --padlog <private-recorded-padlog> \
  --hard-icount-cap <N> --source-ref <opaque-provenance-ref> \
  --count 1000 --cadence <N>
  # --endpoint defaults to unix:///run/dh/grpc.sock
  # --snapshot defaults to BRIDGE_REAL_SNAPSHOT_REF from real.env
  # framebuffers are always captured (no flag) — CaptureExportOptions
  # hardcodes framebuffer: true (refwork-verify/src/phase4_capture_export.rs)

# 5. artifact-check — immediate durable validation of the export
tools/m6-session-pipeline.sh artifact-check --private-root "$PR"

# 6. score-plan — K=32 deterministic score plan
tools/m6-session-pipeline.sh score-plan --private-root "$PR" \
  --first-boss <capture-id> \
  --goal-positive <capture-id> --goal-negative <capture-id>
  # repeat --first-boss/--goal-positive/--goal-negative for multiple examples

# 7. trace — trajectory + trace report
tools/m6-session-pipeline.sh trace --private-root "$PR" \
  --labels <private-operator-labels.yaml>
  # writes $BUNDLE/trajectory/first-boss.jsonl + validation/trace-report.json

# 8. status — resumability / progress check, any time
tools/m6-session-pipeline.sh status --private-root "$PR"
```

Between stages 4 and 6, author the private artifacts the pipeline doesn't
generate itself (fast-follow 04 steps 5–6): `dedup-groups.jsonl` (both
`same_canonical_state` and `distinct_stable_state` examples) and the private
operator labels file consumed by stage 7's `--labels`.

## 5. Run B — the recorded trajectory (operator + agent, same session)

One coherent padlog covering, per plan package 04's operator briefing, all
**five required trajectory elements**:

1. start area + the leaving-start-area transition;
2. first upgrade;
3. first boss + post-boss evidence;
4. ordinary goal-negative states along the way;
5. a credits/late-game goal-positive state (hand-play, approved snapshot, or
   operator-provided late-game save RAM — record the source; identity of
   image/ROM/map/scoring/layout must be preserved across whichever source is
   used).

Element 5 is what the goal-only-on-credits proof rests on: without it, gate
3 of Phase 4 cannot be declared even on the full branch. If the late-game
fixture must come from a separately approved snapshot/segment, record that
source explicitly in the private evidence, not in any public record.

This padlog is the `--padlog` input to pipeline stage 4 (`capture`); the
resulting captures feed stages 5–7 the same as any other capture.

## 6. Closeout pointers (package 05 — executed after this session, not part of it)

1. Update `tools/m6-gate-check.sh`'s `CANDIDATE_PATHS` (top of file) with
   the real public-safe corpus/session path this session produced — probe
   for a specific manifest/marker file (e.g. the checksum-manifest freeze
   output), never a bare directory, to keep fail-closed semantics.
2. Re-run `tools/m6-gate-check.sh`; expect 4/4 PASS. Paste its output
   verbatim into
   `.agents/plans/phase4-m6-scoring-goal-integration/GATE-RECORD.md`, naming
   the branch (full-corpus), the scorer build SHA available, and resulting
   M6 scope.
3. Bead hygiene: close `refwork-czi`, `refwork-20v`, `refwork-5tk` with
   evidence; comment `refwork-5be` that its gate is open with the
   GATE-RECORD pointer.
4. Fast-follow bookkeeping (packages 06–08: context fixture, fulfillments,
   resolution) belongs to the fast-follow plan, not this one — do not close
   its records from here.
5. Operator ask 3: explicit approval to `git push origin main`, listing the
   commits being pushed.
6. Hand off execution to
   `.agents/plans/phase4-m6-scoring-goal-integration/` packages 02
   (joint half) through 08.

## Standing constraints (repeat — binding throughout)

Never track private game-derived payloads. No ROM names, offsets, decoded
values, or private paths in public records, beads, terminal transcripts, or
anything under this git checkout — everything private lives beneath
`$PRIVATE_ROOT`, outside every checkout. One corpus id everywhere once
frozen; no re-derivation under new ids.
