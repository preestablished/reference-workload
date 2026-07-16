# refwork-czi Closure Evidence (2026-07-12)

## Commit

`2827665` — "Phase 4 capture exporter, artifact/context/fallback
verification (refwork-czi)": 16 files, +2,692/−189. Implementation was
carried uncommitted at base `4eb8a3a`; intervening main commits
(`3a53298`, `709b075`, `b16fa72`, `98f81f3`, `35a9d48`) verified
doc/tooling-only with no code overlap.

## Pre-commit inspection

- Diff matches the bead scope: exporter (`phase4_capture_export.rs`,
  698 lines), read-only artifact check (342), context export (452),
  separately typed fallback validator (367), checksum-manifest
  payload-root + `--verify` rework, bundle/context check touch-ups,
  CLI wiring in `main.rs`, integration tests, 3 corpus-guide doc pages.
- Private-payload grep (absolute home paths, long hex constants outside
  fixtures): clean. No absolute paths in the new modules.
- `data/` disposition: snapstore runtime scratch (`STORE_VERSION`,
  `meta/tree.db`, `store/pages/*.spk`, `snapstore.sock`), referenced by
  nothing in the diff; live sockets matching "snapstore" were listening
  at inspection time — left entirely untracked and untouched.

## Dirty-tree verification run (pre-commit)

`cargo fmt --all -- --check` && `cargo test --locked -p refwork-dh-client`
&& `cargo test --locked -p refwork-verify` && `git diff --check` →
ALL GREEN (full-crate run subsumes the runbook's focused `phase4_*`
filters).

## Final clean-checkout gate (the bead's hold-open condition)

Sibling worktree `../reference-workload-czi-clean-checkout` at `2827665`
(sibling placement required by the `../control-plane` etc. path deps;
`cargo metadata --locked` verified resolving first). Full suite there:

- refwork-dh-client: 3 + 1 passed
- refwork-verify: 14 (unit) + 31 (integration, 590.27 s — includes the
  10k double-run determinism test) + 6 + 4 passed; doc-tests 0
- fmt + `git diff --check`: clean
- Result line: `CLEAN-CHECKOUT GATE: ALL GREEN at 2827665…`

Worktree removed after the run.

## Bead

`refwork-czi` closed 2026-07-12 citing this file.
