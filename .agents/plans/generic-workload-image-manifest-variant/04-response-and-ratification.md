# 04 — Response File and Ratification (WP 4)

Targets: `$HOME/.agents/projects/reference-workload/responses/2026-10-13-generic-workload-image-manifest.md`
(`new`; the `responses/` directory does not exist yet and is created by this
package), the request file's `Status` line, and the `bn` issue for this work.

## Goal and prerequisite state

Goal: the requester (determinism plan WP 1.2) can drop `--unratified`
because a response names the landed commit and quotes the ratified §4 text.

Prerequisite: gate G3. WP 1–3 are complete, the feature branch is pushed,
the pull request is open, and the user has merged it so the resulting
commit is on `origin/main`. Merging to `main` is the user's action (D10);
the implementer does not push to `main`.

## Repository evidence

- The request's response contract names the exact path above and asks for
  the commit that amends §4 and `validate_machine`.
- No `responses/` directory exists under any `$HOME/.agents/projects/*`
  (checked 2026-10-13). The nearest precedent is
  `$HOME/.agents/projects/reference-workload/requests/pad-alphabet-and-phase4-context-fixtures/FULFILLMENT.md`,
  which opens with a `**Status:**` line and sections for the decision,
  source evidence, and what is not fulfilled. Follow that shape.
- The owner document is not version-controlled (`00-overview.md`
  Constraints), so the response must carry the amended text.

## Exact change surface

### Response file content

1. Header: Request path, date answered, responder (reference-workload
   maintainer), `**Status:** accepted and implemented` (or `accepted,
   implementation pending` if written before merge; do not leave it in that
   state at session end).
2. `## Landed commit`: the full 40-hex SHA on `origin/main` that contains
   the change (the merge commit, or the squash commit if the PR was
   squashed), its subject line, the PR number, and the list of files it
   touched. Obtain with `git fetch origin && git log origin/main -1
   --format=%H%n%s` after the merge and `git show --stat` on that SHA;
   never type a SHA from memory and never cite a feature-branch SHA.
3. `## Amended §4 text`: the verbatim §4.1–§4.4 content and the field table,
   copied from API.md after the edit, preceded by `sha256sum` of API.md at
   response time so later drift is detectable. In the same step rewrite the
   §4.1 status line from `proposed` to `Status: ratified <date>, response
   <path>` and recompute the hash after that rewrite (quote the final
   hash).
4. `## Validator behavior`: the fixture CLI invocation and its output line,
   plus the test names from `03-fixture-and-tests.md` that prove each
   request criterion (`game_manifest_without_game_image_rejected`,
   `generic_fixture_validates_in_place`, `generic_with_virtio_blk_rejected`,
   `game_error_vectors_are_unchanged`). Include the local 0.2.0 CLI run and
   the CLI negative run only if they were actually performed, each labelled
   "performed on <host>".
5. `## Contract notes for guest-sdk WP 1.2`: the rules that go beyond the
   request text and are now ratified: the stamp key decision (`git_rev`,
   `workload_image: <name>@<version>`, no `emu_version`), the validated
   unstamped-sidecar keys, the generic bundle file list (`README.md` and one
   sidecar required, `harness.toml` not), the `boot.toml` /
   `expected-regions.toml` cross-checks, the exact-two-devices rule, the
   4096-multiple region sizes, the `meta.name`/`meta.version` charset, and
   the fixture path to copy. State that if WP 1.2 cannot conform it should
   file a follow-up request rather than emit behind `--unratified`.
6. `## Not covered by this response`: request criterion 3 (hypervisor
   `device_set` derivation test) is owned by determinism-hypervisor under the
   requester's plan WP 1.4, which names its own change sites; `pv-net` is a
   later request; no generic builder lives in this repository;
   `image register` of generic bundles is untested and not a handoff path;
   INTEGRATION.md line 193's phrasing of the stamp is unchanged and may be
   tidied by a later doc pass.

### Request file

Change `- **Status:** open` to `- **Status:** answered (see responses/...)`.
Make no other edit to the request.

### Tracker

- Before WP 1: `bn create "Ratify generic workload-image manifest variant (API §4 + xtask validate)" -p 1 -t task`
  and `bn update <id> --claim`. Record the id in the commit body.
- After WP 4: `bn close <id> -r "<commit sha>; response written"`.
- `bn remember` one line: the owner doc path for §4 and the fact that it is
  unversioned, if no existing memory says so (`bn memories API.md` first).

### Session close

Follow `AGENTS.md` Session Completion: `git pull --rebase`, `git push`,
`git status` up to date, `bn status --json` synchronized.

## Acceptance

1. The response file exists at the exact path and contains the six sections.
2. The SHA in the response resolves on `origin/main`
   (`git branch -r --contains <sha>` lists `origin/main`).
3. The request file's status line reads `answered`.
4. The `bn` issue is closed with the SHA in its reason.

## Dependencies, risks, exclusions

- Depends on WP 1–3 merged via PR (G3).
- Risk: writing the response before the merge leaves a dangling SHA if the
  PR is squashed or rebased. Write it only after the SHA is on
  `origin/main`.
- Exclusion: do not edit the determinism plan files; the requester updates
  its own plan when it reads the response.
