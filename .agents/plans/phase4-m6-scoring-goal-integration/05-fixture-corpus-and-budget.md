# Package 05 — Scorer Fixture Corpus + Budget Check (Item 4)

**Gate:** GATE-RECORD.md exists AND scorer M4 (running service with its
timing harness). In the first-room-fallback branch the corpus is the
fallback bundle's captures — record the reduced size/coverage; the <1 ms
check still runs on whatever states exist.

## Supply side (this repo)

- The 1,000-state captured `(wram, fb)` fixture corpus **is** the
  fast-follow's frozen corpus (its normal outcome is ≥1,000 frame-coherent
  captures). Do not build a separate corpus — reference the frozen id.
- Produce the **expected-scores sidecar**: for each state, the expected total
  under the real map/program pair. **No tool in this repo computes this** —
  `refwork-verify phase4-score-plan` emits K=32 batch membership + label
  capture ids only (its doc comment says so; it takes no `--map`/`--scoring`);
  `trace` needs a full capture index + labels and reports stage annotations,
  not a per-state score sidecar. Generate the sidecar with a standalone
  script that decodes each state's features per the real map and evaluates
  API.md §2.2 semantics directly (the `refwork_featuremap` crate's parsers
  may be reused via a scratch cargo project with a path dependency — do not
  edit `refwork-verify`). Then hand-verify ≥10 states independently of that
  script before publishing, because this sidecar defines "correct" for the
  scorer's own acceptance. If the live scorer generates the sidecar instead,
  the hand-spot-check is mandatory and must be independent of the scorer.
- Freeze the sidecar alongside the corpus under the same id conventions;
  record its hash.

## Scorer side (joint)

The scorer evaluates a captured region set in <1 ms/state for this map —
their budget, their harness, your fixture. Jointly record: scorer build SHA,
map/program hashes, corpus id, states evaluated, timing distribution
(at minimum p50/p99 and max, not just mean), and score-match count
(expected-scores sidecar vs scorer output — must be 1,000/1,000; any
mismatch is settled by the spec-ownership rule).

## Exit Signal

One corpus id consistent across the fast-follow's FULFILLMENT records
(`~/.agents/projects/reference-workload/requests/phase-4-scorer-golden-artifacts/FULFILLMENT.md`
and `~/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`),
the sidecar, and the scorer packet's record; budget evidence recorded both
sides.
