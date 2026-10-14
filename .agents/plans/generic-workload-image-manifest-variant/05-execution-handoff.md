# 05 — Execution Handoff

## Work-package order

```text
WP 0 tracker setup + branch
  └─ WP 1 owner doc amendment (with backup) ──┐
                                               ├─ G1 ─▶ WP 2 validator split ─▶ WP 3 fixture + tests ─▶ G2 ─▶ push branch + open PR ─▶ user merges (G3) ─▶ WP 4 response
```

WP 2 and WP 3 may be developed in one branch and one or two commits. WP 1
has no code dependency but must be finished before WP 2 so the code follows
the table. Nothing in this plan parallelizes across people; one implementer
can do it serially in a single session.

## Packages

| WP | Changes | Prerequisites | Verification | Acceptance |
|---|---|---|---|---|
| 0 | `bn create` + `--claim`; branch `generic-manifest-variant` from `main` at or after `9fdb091` | none | `bn show <id>` shows in_progress | issue id recorded |
| 1 | backup of API.md to `$HOME/.agents/projects/reference-workload/backups/`; then `$HOME/.agents/projects/determinism/docs/reference-workload/API.md` §4: example lines (incl. the `last_green` sidecar annotation), §4.1 status line `proposed`, §4.1–§4.4, field table (`01-owner-doc-amendment.md`) | WP 0 | YAML example parses; `grep -c variant API.md >= 8`; `docs/MAP.md` hash unchanged; backup exists | `01` acceptance 1–6 |
| 2 | `xtask/src/image.rs`: `ManifestVariant`, `resolve_variant`, `manifest_variant`, `validate_schema_common`, `validate_game_meta`, `validate_determinism_block`, `validate_generic_handoff_files`, shared stamp expectations, `array_blocks`, `is_git_rev_hex`; variant parameter on `validate_machine`, `validate_regions`, `validate_fps`, `validate_defaults`; optional CLI suffix in `xtask/src/main.rs` (`02-validator-variant-split.md`) | WP 1 (G1); the characterization test from WP 3 captured first | `cargo test --locked -p xtask`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets -- -D warnings` | all pre-existing tests pass unchanged; characterization vectors equal |
| 3 | `image/fixtures/generic/*` (7 files), tests in `xtask/src/image.rs` and `xtask/tests/image_inputs.rs`, two `image/README.md` edits (`03-fixture-and-tests.md`) | WP 2 | `cargo test --workspace --locked`; the fixture `image validate` CLI run | `03` acceptance 1–4 |
| PR | push `generic-manifest-variant`, open a PR with `gh pr create` citing the request path and the `bn` id; the user merges | G2 | `gh pr view` shows merged; `git fetch origin && git log origin/main -1` | merge commit on `origin/main` (G3) |
| 4 | response file, request status line, `bn close` (`04-response-and-ratification.md`) | G3 | `git branch -r --contains <sha>` includes `origin/main`; `bn status --json` synchronized | `04` acceptance 1–4 |

## Commands

```sh
# WP 0
cd "$(git rev-parse --show-toplevel)"
git switch -c generic-manifest-variant main
bn create "Ratify generic workload-image manifest variant (API §4 + xtask validate)" -p 1 -t task
bn update <id> --claim

# WP 1 backup and checks
api="$HOME/.agents/projects/determinism/docs/reference-workload/API.md"
bk="$HOME/.agents/projects/reference-workload/backups"; mkdir -p "$bk"
cp -f "$api" "$bk/$(date -u +%F)-API.md.pre-variant"
sha256sum "$api" > "$bk/$(date -u +%F)-API.md.pre-variant.sha256"
#   ... edit API.md per 01 ...
sha256sum "$api" > "$bk/$(date -u +%F)-API.md.post-variant.sha256"
# Rollback (only if WP 2/3 are abandoned): first confirm the live hash still
# equals the post-variant hash, then cp -f the .pre-variant copy over API.md.
sha256sum "$HOME/.agents/projects/determinism/docs/MAP.md"   # before and after; must match
grep -c variant "$HOME/.agents/projects/determinism/docs/reference-workload/API.md"

# WP 2 / WP 3 checks
cargo test --locked -p xtask
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run --locked -p xtask -- image validate image/fixtures/generic/workload-image.yaml

# Negative CLI proof for the response: the unit test
# game_manifest_without_game_image_rejected is the required evidence (dist/ is
# untracked). Optionally, on a box that has a local 0.2.0 build:
#   tmp=$(mktemp -d) && cp -rf dist/workload-image-0.2.0/. "$tmp"/ \
#     && sed -i '/role: game-image/d' "$tmp/workload-image.yaml" \
#     && cargo run --locked -p xtask -- image validate "$tmp/workload-image.yaml"; echo "exit=$?"

# PR
git push -u origin generic-manifest-variant
gh pr create --fill   # body cites the request path and the bn id; the user merges
bn note <id> "PR <url> open; WP 4 (response file, §4 status→ratified) pending the merge"

# WP 4 (resume step, after the merge is observed)
git fetch origin && git log origin/main -1 --format=%H%n%s
bn close <id> -r "<sha on origin/main>; response written"
git switch main && git pull --rebase && git status
bn status --json
```

Commit grouping: one commit for the characterization test (captured against
the pre-refactor validator), then one commit for WP 2+3 (validator, fixture,
tests, README) with a body that cites the request path and the `bn` id. WP 1
has no commit (unversioned file). WP 4 has no repository commit.

## Interim end state (after the PR is open, before the merge)

The implementing session ends here if the merge has not happened. Required
state at that point: feature branch pushed and PR open; `bn` issue still
`in_progress` with the note above; API.md §4.1 status line reads
`proposed`; the backup and both hash files exist; local `main` untouched;
`git status` on the feature branch shows no unintended tracked changes;
`bn status --json` synchronized. WP 4 is resumed by a later session that
first runs `gh pr view <url> --json state,mergeCommit`.

## Integration and regression gates

- G1: `01` table complete and consistent with D3–D6a, and the API.md
  backup exists, before any code.
- G2: WP 2+3 tests green (including `game_error_vectors_are_unchanged` and
  `game_manifest_without_game_image_rejected`), fmt and clippy clean, the
  fixture CLI validation OK, and `git status` clean except the intended
  files, before opening the PR.
- G3: the PR is merged by the user; the SHA cited in the response is on
  `origin/main`.
- Regression: every pre-existing test name in `xtask/src/image.rs` lines
  2120–2789 still exists and passes. Count them before and after
  (`grep -c '#\[test\]' xtask/src/image.rs`) and require the after-count to
  equal the before-count plus the number of new tests.

## Definition of done

1. API.md §4 amended per `01`; `docs/MAP.md` unchanged.
2. `image/fixtures/generic/workload-image.yaml` validates via the CLI;
   a `game` manifest without `game-image` fails (unit test); game error
   vectors are unchanged (characterization test).
3. CI green on the PR (`cargo fmt`, `cargo clippy`, `cargo test --workspace --locked`).
4. PR merged by the user; response file written with the `origin/main`
   SHA and the API.md SHA-256; §4.1 status line reads `ratified`; request
   status `answered`; `bn` issue closed; `bn status --json` synchronized.
5. Request criterion 3 is explicitly recorded as hypervisor-owned in the
   response, not claimed.

## Deferred work and follow-ups

- Hypervisor manifest-driven `device_set` derivation and its generic-list
  test: determinism-hypervisor, requester's plan WP 1.4.
- Running `image validate` on fixtures in CI: existing issue `refwork-425`.
- `pv-net` device kind: future request from the requester (stated
  exclusion).
- Reference-workload consuming the generic image builder: requester's
  deferred list; not planned here.
