# Current State (Evidence-Based)

Repo `main` at `746e2b6` (the round-1 request filing commit on top of
`2ea42ad`), clean tree, assessed 2026-07-07. Bead census: 8 issues,
6 open, 0 in progress; `bd ready` = exactly `refwork-gp9`.

## Round-1 Status (The Gate)

Unexecuted. No image rebuild, no READY regen, no stamp, no bead-state
changes since filing. (One correction to round-1's text: the sequencing
dep edges `.12→.11`, `.13→.12`, `.14→.13`, `.15→.14` **do** exist —
verify with `bd dep list`, not `bd dep tree`, which silently omits the
chain and even mislabels `.15`; that rendering quirk misled two
assessments.) The d7t chain: `.1` blocked (M2 floor evidence — blocks the epic), `.11`–`.15`
open, `gp9` open with substantial 2026-07-02 progress recorded (dist
rebuilt at `369770a`, D7 framebuffer contract test, real
`register_region` before Ready) but boot-under-local-worker, READY
regen, and the operator cutover still outstanding.

## Phase-4 Tooling: Built, Tested, Idle

- Subcommands in `crates/refwork-verify/src/main.rs`: `trace` (:97),
  `phase4-bundle-check` (:99), `phase4-checksum-manifest` (:100),
  `phase4-context-check` (:101), `phase4-layout` (:102),
  `phase4-private-intake` (:103), `phase4-score-plan` (:104); sources in
  `crates/refwork-verify/src/phase4_*.rs`; 20 phase4 tests locked over
  synthetic fixtures.
- Operator runbook: `docs/phase4-corpus-guide/` — private-intake →
  map-layout → capture-export → label-score-trace → validate-handoff.

## The Two Stuck Project Requests (What This Closes)

At `~/.agents/projects/reference-workload/requests/`:

1. **`phase-4-scorer-golden-artifacts/`** — FULFILLMENT.md: "Status: not
   fulfilled … no validated lab-private scorer golden artifact bundle is
   recorded here yet"; all tooling done; "remains blocked on the Real
   Capture Evidence Floor and operator approval." Downstream:
   state-scorer M1's smoke (golden tests against Phase 3 captures,
   `phase-4-scoring-and-inputs.md` scorer chain).
2. **`pad-alphabet-and-phase4-context-fixtures/`** — FULFILLMENT.md:
   "partially fulfilled": pad layout identity `console16-12btn-v1`
   implemented and documented; "the live Phase 4 context fixture is not
   fulfilled yet because no durable Real Capture Evidence Floor artifact
   has been attached." Downstream: input-synthesizer M2 macro packs and
   the orchestrator's input-synth client context.

Both floors share the real-capture prerequisite that round 1 clears —
but be precise about inventory: round 1 produces **first-room** captures
only. The scorer-golden artifact contract
(`phase-4-scorer-golden-artifacts/01-artifact-contract.md`) additionally
requires a labeled **hand-played** trajectory with first-boss coverage
and a credits/late-game goal-positive fixture, **≥1,000 captured
states**, and a **real-offset** private feature map ("no placeholder
offsets" — the checked-in `feature-maps/demo-game.yaml` is explicitly
disqualified). And the runbook's step 3
(`docs/phase4-corpus-guide/03-capture-export.html`) says verbatim that
the capture exporter is "the main missing producer step" — `vm-first-room`
/ `vm-suite` write JSON reports and hashes, not per-capture
`captures/index.jsonl` + `artifacts/` rows. So this request carries
three real work fronts beyond "run the tooling": the capture exporter,
the real feature map/scoring program, and an operator hand-play session
(entry conditions and scope in `02-`).

## Operator Inputs (Captured Once, Used Twice)

The round-1 lab session collects ROM BLAKE3, first-room padlog BLAKE3,
and run owner. `phase4-private-intake` and the bundle manifests consume
exactly those fields — which is why this request is filed alongside
round 1 rather than after it: the lab operator should know both
consumers exist before the session, so nothing is collected twice.
Public release of any private game/revision metadata additionally
requires operator approval (scorer-golden FULFILLMENT.md).

## Who Consumes The Corpus

- **state-scorer M1** (feature decoding golden tests) — first item of
  the Phase 4 critical chain (`phase-4-scoring-and-inputs.md`).
- **input-synthesizer M2** (demo-game macro packs, pad fixture).
- **Phase 4 exit gate 2** (canonical-hash dedup "verified on real
  states") — the corpus is where those real states come from.
- The scorer/synthesizer repos do not exist locally yet; the corpus
  bundle + FULFILLMENT records are the frozen interface they'll open
  against.
