# 05 — Acceptance checklist (M0 exit, mapped to commands)

Run from the repo root. Every box must be checked before the branch merges.

## M0 acceptance clauses (IMPLEMENTATION-PLAN.md, verbatim mapping)

- [ ] **"`cargo test` green"**
      `cargo test --workspace` — all crates, including the new featuremap
      round-trip/rule tests, protocol round-trip + golden-bytes tests, fixture
      sweep, and deny self-test. (Plus the standing M1 suites still green.)
- [ ] **"`refwork-featuremap validate feature-maps/demo-game.yaml` passes"**
      `cargo run -p refwork-featuremap -- validate feature-maps/demo-game.yaml`
      → exit 0. And the cross-file form with `--scoring scoring/demo-game.yaml`
      → exit 0.
- [ ] **"rejects 10 checked-in negative fixtures"**
      `cargo test -p refwork-featuremap --test fixtures` → every
      `tests/fixtures/invalid/*.yaml` rejected with the expected rule id
      (manifest-asserted, manifest↔directory bijection checked); ≥10 fixtures
      present, including `bad offset` (#01) and `volatile-in-predicate` (#12)
      named by the acceptance text.
- [ ] **"CI deny-gates demonstrably fail a PR that adds `std::thread` to
      `refwork-emu`"**
      `cargo test -p xtask --test deny_selftest` → planted-token tree fails
      the scan, comment-only mention does not. `cargo xtask deny` still exits
      0 on the real tree (now including `refwork-protocol`).

## Standing repo gates (must not regress)

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo xtask deny`
- [ ] `cargo test --release -p xtask --test determinism -- --include-ignored`
      (10k-frame double-run — M1 gate, unaffected but re-verified)
- [ ] `cargo test --release -p xtask --test zero_alloc`
- [ ] Schema drift: `cargo run -p refwork-featuremap -- schema | diff -u schema/feature-map.schema.json -`

## Spec-fidelity spot checks (manual, five minutes)

- [ ] `feature-maps/demo-game.yaml` and `scoring/demo-game.yaml` byte-match
      the API.md §1.4/§2.1 examples modulo the placeholder-warning header
      comment (no invented offsets/values).
- [ ] `CtlMsg`/`FaultCode` variant names, field names, and **variant order**
      match API.md §3.1 exactly — enforced mechanically by the per-variant
      golden-bytes table (plan 02), not by eyeball; confirm that test covers
      all 11 `CtlMsg` variants + all 5 `FaultCode` values.
- [ ] `PROTO_VERSION` is declared `pub const PROTO_VERSION: u16 = 1;` and the
      old stub's `u32` is gone: `grep -rn "proto_version: u32" crates/` →
      empty.
- [ ] No commercial console/game names anywhere:
      grep the diff for the known excluded proper nouns (clean-room
      acceptance criterion) → no matches.
- [ ] The spec README's "Repository layout (target)"
      (`~/.agents/projects/determinism/docs/reference-workload/README.md`)
      now matches reality: `schema/`, `feature-maps/`, `scoring/` exist with
      content. (The in-repo README is a one-liner; updating it is optional,
      not an M0 clause.)

## Doc-reconciliation follow-through

- [ ] The three upstream doc-drift items in `00-overview.md` §doc-recon
      (`stage.requires` missing from refwork §2; volatile-in-shaping
      severity; feature-name pattern) are recorded as documentation issues —
      not silently absorbed into code comments only.

## Process

- [ ] Implemented on a branch; `/review` dual-reviewer pass run on the diff;
      findings reconciled (apply or reject-with-reason); fixes re-verified.
- [ ] Commit message records what M0 clause each change satisfies.
- [ ] Branch pushed; PR references this plan directory.
