# Generic Workload Image Manifest Variant

Status: Ready

Planning is complete. Implementation has not occurred. No repository file
outside this plan directory has been changed by planning.

## Application context

```json
{
  "application_context": {
    "has_active_users": false,
    "backward_compatibility_required": false,
    "feature_flags": "not-applicable",
    "confirmation_digest": "7d21dfd66ee4781021afe0a93d0d6ce2016c19a43350bd887ee5cf120d0ea3eb",
    "confirmed_at": "2026-10-13T18:51:38Z"
  }
}
```

The user confirmed on 2026-10-13 that the manifest contract and the
`xtask image validate` gate have no active users and that backward
compatibility is not required. The user also answered that feature flags are
not appropriate. Because both booleans are false, the record stores
`not-applicable` as the plan format requires. No feature flag, environment
variable, or compatibility alias is introduced anywhere in this plan.

On 2026-10-14 the user, as maintainer and decision owner, approved
ratifying in API.md §4 the generic bundle rules that go beyond the request
text (D4, D5, D6: required `README.md` and one sidecar, 40-hex revisions,
`boot.toml` / `expected-regions.toml` cross-checks, the `meta.name` and
`meta.version` charset, and the `git_rev` stamp key).

One constraint looks like a compatibility promise but is a **request
requirement**, not a compatibility obligation: the request states that
`variant: game` (the default when the key is absent) keeps the current rules
unchanged, and that a `variant: game` manifest without `game-image` must still
fail. The plan honors that as acceptance criterion 2 of the request.

## Request

Source: `$HOME/.agents/projects/reference-workload/requests/2026-10-13-generic-workload-image-manifest.md`
(open, P1, requested by the determinism plan
`$HOME/.agents/projects/determinism/plans/generic-workload-and-cloud-gates/`
WP 1.2). Decision owner: the reference-workload maintainer.

Requested outcome: amend the owner document for the `workload-image.yaml`
manifest with a `variant` field (`game` default, `generic` new) and make this
repository's validator accept the generic device list, so that guest-sdk's
generic image builder can emit a manifest the platform accepts without the
`--unratified` escape hatch.

## Change type and affected areas

Change type (inferred): contract amendment plus validator change in developer
tooling. No runtime, emulator, harness, or guest code changes.

Affected areas:

| Area | Location | Nature |
|---|---|---|
| Owner document | `$HOME/.agents/projects/determinism/docs/reference-workload/API.md` §4 (lines 530–616, end of file) | normative amendment |
| Validator | `xtask/src/image.rs` (`validate_manifest` and the `validate_*` helpers, lines 264–1792) | variant-aware split |
| Validator tests | `xtask/src/image.rs` `mod tests` (lines 2085–2789), `xtask/tests/` | new fixture and negative tests |
| Fixture | `image/fixtures/generic/` (`new`) | checked-in generic bundle fixture |
| Input docs | `image/README.md` | one paragraph describing the generic fixture and variant |
| Request response | `$HOME/.agents/projects/reference-workload/responses/2026-10-13-generic-workload-image-manifest.md` (`new`) | ratification record |

Not affected: `crates/`, `scoring/`, `feature-maps/`, `schema/`, the image
build path (`build_image`, `write_workload_manifest`), `double_build`,
`register_image`, CI workflow files.

## Measurable success criteria

These map one-to-one onto the request's acceptance criteria, plus the
response contract.

1. API.md §4 contains a `variant` field definition and a field table that
   states, for each top-level manifest field, whether it is required under
   `game` and under `generic` and what value rule applies. `docs/MAP.md`
   is not edited. Evidence: the amended section quoted in full inside the
   response file.
2. `cargo run --locked -p xtask -- image validate image/fixtures/generic/workload-image.yaml`
   prints `image validate: OK - ...` and exits 0 from a clean checkout.
3. A `variant: game` manifest whose `machine.devices` lacks the
   `virtio-blk game-image` entry fails `validate_manifest` with an error that
   contains `machine.devices missing virtio-blk`. A manifest with no
   `variant` key behaves identically. Both are unit tests.
4. A `variant: generic` manifest that lists `virtio-blk`, any device kind
   other than `detguest-channel` and `pv-pad` (for example `pv-net`), or a
   duplicated kind fails. A `variant: generic` manifest missing `pv-pad` or
   `detguest-channel` fails. A manifest with an unknown `variant` value fails
   with exactly one error naming the value. All are unit tests.
5. Every existing test in `xtask/src/image.rs` and `xtask/tests/` still
   passes with its assertions unchanged, and a characterization test
   recorded before the refactor proves the game error lists are identical
   after it (`03-fixture-and-tests.md`). The `dist/workload-image-0.2.0/`
   bundle is untracked (`.gitignore` line 5), so validating it in place is
   an optional local check, not a gate.
6. The response file exists, names the commit on `origin/main` that landed
   items 2–5 (after the pull request merges), quotes the amended §4 text
   together with the SHA-256 of API.md at response time (item 1 cannot be
   committed), and is referenced from the request's status line. Request criterion 3 (hypervisor `device_set` test)
   is recorded there as owned by determinism-hypervisor WP 1.4, not
   claimed.

## Scope

- Define the `variant` field and the `generic` rule set in API.md §4.
- Split `validate_manifest` into a variant-aware schema layer and a
  game-variant bundle layer so a generic bundle built elsewhere validates.
- Add a checked-in generic fixture bundle and tests for both variants.
- Write the response file and update the request status.

## Non-goals

- No change to how the game variant is built or validated. `build_image`
  keeps emitting the same manifest bytes it emits today (no `variant` key),
  so `double_build` expectations and the existing `dist/` bundles are
  unaffected. Every new validator rule in this plan is scoped to
  `variant: generic`; the game error list is unchanged (R1).
- No change to `image register` semantics. Because `register_image` calls
  `validate_manifest` (`xtask/src/image.rs` lines 391–436), a generic
  bundle will pass `image register` once this plan lands. Registering
  generic bundles through this repository's handoff is not a goal and is
  not tested; control-plane registration of generic images is the
  requester's follow-up.
- No new device kinds. `pv-net` is explicitly a later request.
- No change to the hypervisor, orchestrator, control-plane, or guest-sdk.
  Request criterion 3 is hypervisor work.
- No builder for generic images in this repo. guest-sdk owns that (WP 1.2).
- No change to `docs/MAP.md`.
- No green stamp for any generic image. The stamp is produced by the
  workload's own gate; this repo only defines what the stamp sidecar must
  contain to validate.

## Constraints

- The owner document lives outside any git repository
  (`$HOME/.agents/projects/determinism/docs/` has no `.git`; verified
  2026-10-13). Its amendment cannot be referenced by commit. The response
  file must quote the amended §4 text so the ratified wording is durable.
- Clean-room source boundary (`$HOME/.agents/projects/determinism/docs/MAP.md`
  §Clean-room source boundary): no vendor, console, or case-study names in
  any new repository text. Use the existing neutral phrasing of API.md.
- `bn` is the tracker. Create one issue for this work before implementation
  and close it in the response step. Do not use markdown TODO lists.
- Dirty worktree at planning time: untracked `.beads/`, `data/`,
  `.agents/plans/validate-private-phase4-contracts/`, and this plan
  directory. None are implementation work. Leave them unstaged; every
  "clean tree" check in this plan means "no unintended tracked changes".
- The validator must stay dependency-free beyond what `xtask/Cargo.toml`
  already lists (`serde_yaml 0.9`, `blake3`).

## Repository findings that drive the design

| Id | Finding | Evidence |
|---|---|---|
| F1 | `validate_manifest` is a whole-bundle validator for `refwork-demo`, not a schema validator. It pins `meta.name` to `refwork-demo`, `meta.version` to the xtask crate version, `fps` to the exact console rational, `regions` to `wram`/`framebuffer`/`meta` with fixed sizes, `defaults` to `demo-game@latest`, and requires `boot.toml`, `harness.toml`, `expected-regions.toml`, `README.md` beside the manifest. | `xtask/src/image.rs` lines 10–40 (constants), 264–326, 1352–1490, 1492–1535 |
| F2 | `validate_machine` requires exactly the three game devices and pins `mem_mib` to 128. | `xtask/src/image.rs` lines 1227–1266 |
| F3 | `validate_boot_toml` requires `[unit.control]` with `refwork-ctl`, `game_dev = /dev/vdb`, `game_source = pv-blk`, and `[[expected_region]]` entries for the three game regions. `validate_harness_toml` requires `game_image_device = /dev/vdb`. A generic bundle has neither a control leg nor a `harness.toml`. | `xtask/src/image.rs` lines 1661–1754 |
| F4 | The green-stamp sidecar validator pins `workload_image` to `refwork-demo@<xtask version>`, requires `reference_workload_git_rev` equal to `meta.built_from.git_rev`, and requires `emu_version` equal to `meta.built_from.emu_version`. `emu_version` is required unconditionally in `meta.built_from`. | `xtask/src/image.rs` lines 287–304, 1537–1638 |
| F5 | The manifest is written by `write_workload_manifest` as a literal template with no `variant` key. The checked-in `dist/workload-image-0.2.0/workload-image.yaml` has no `variant` key. | `xtask/src/image.rs` lines 1043–1113; `dist/workload-image-0.2.0/workload-image.yaml` |
| F6 | API.md §4 is the last section of the file (lines 530–616). Its header says parsers must ignore unknown optional fields and reject unknown required-context fields. | `$HOME/.agents/projects/determinism/docs/reference-workload/API.md` lines 11–14, 530–616 |
| F7 | `docs/MAP.md` contract-ownership table does not name the WorkloadImage manifest in its own row; ownership is established by API.md's header ("§4 is the WorkloadImage artifact manifest") and INTEGRATION.md line 13. No MAP.md edit is needed or requested. | `$HOME/.agents/projects/determinism/docs/MAP.md` lines 137–156 |
| F8 | No downstream repository parses `workload-image.yaml` with serde. The hypervisor hard-codes its device set in `m9_linux_machine_config`; the orchestrator's `fps`/`pad_layout` attrs come from `ExperimentConfig`, not the manifest file. Adding a top-level key cannot break a strict parser today. | `determinism-hypervisor/crates/dh-worker/src/m9_handoff.rs` lines 1318–1340; `exploration-orchestrator/crates/orch-server/src/bringup.rs` lines 27–35 |
| F9 | Unit tests build a valid bundle in a temp dir with `valid_dist()` and mutate the manifest text with `replace`. Negative tests assert on error substrings such as `machine.devices.virtio-blk.role`. | `xtask/src/image.rs` lines 2177–2235, 2638–2658 |
| F10 | CI runs `cargo test --workspace --locked`, which includes `xtask` unit and integration tests. No CI job runs `image validate` on a fixture. | `.github/workflows/ci.yaml` line 48; open issue `refwork-425` |
| F11 | Fixture naming must avoid `validate_no_game_content`: file names containing `game.` or with extensions `rom`, `sfc`, `smc`, `srm`, `sav` are rejected anywhere under the bundle directory. | `xtask/src/image.rs` lines 1756–1792 |
| F12 | The request's hypervisor criterion refers to a `device_set` derivation from the manifest that does not exist yet; the determinism plan schedules it as WP 1.4. Today `m9_linux_machine_config` hard-codes six device ids (detchannel, pv-clock, pv-pad, pv-entropy, pv-blk, debug-serial), four of which never appear in any manifest. | determinism plan `01-generic-workload-template.md` WP 1.4; `determinism-hypervisor/crates/dh-worker/src/m9_handoff.rs` lines 1318–1340 |
| F13 | `dist/` is ignored (`.gitignore` line 5; `git ls-files dist` is empty). The shipped 0.2.0 bundle exists only on boxes that built it. | `.gitignore`; `git ls-files dist` |
| F14 | §4's YAML example shows `determinism.last_green` inline with integer `suite_version`, and INTEGRATION.md line 193 says CI stamps it "into the manifest". The validator reads only the sidecar files `determinism.unstamped.yaml` and `determinism.last_green`, with a string `suite_version`. The doc and the implementation already disagree for the game variant. | API.md lines 603–604; `xtask/src/image.rs` lines 1492–1638; `dist/workload-image-0.1.0/determinism.last_green` |
| F15 | §3.5 requires `RegisterRegion.len` to be a multiple of 4096. The game regions all satisfy it. | API.md lines 497–498 |
| F16 | `validate_manifest` has seven callers (`build_image_with_git_rev`, `double_build` twice, `register_image`, the test helpers `validation_errors` and `validator_accepts_generated_manifest_shape`, and `cmd_image_validate`). Changing its return type is not a one-caller edit. | `xtask/src/image.rs` lines 260, 379, 380, 402, 2230, 2285; `xtask/src/main.rs` line 226 |

## Key decisions

| Id | Decision | Reason | Rejected |
|---|---|---|---|
| D1 | `variant` is a top-level manifest key, string enum `game` or `generic`, absent means `game`. `schema_version` stays 1. | Additive and default-preserving. Matches the request wording and API.md's forward-compatibility rule (F6). | Bumping `schema_version` to 2 (forces every consumer to change for an optional field). Encoding the variant in `kind` (breaks the `kind: workload-image` registry contract). |
| D2 | Split validation into a **schema layer** that applies to both variants and a **game bundle layer** that applies only to `game`. The generic variant gets its own small **generic bundle layer** for sidecar files. | F1–F4 show the current checks cannot accept any manifest that is not `refwork-demo`. The request's "accept the generic device list" is necessary but not sufficient for criterion 2. | Patching only `validate_machine` (the fixture would still fail on `meta.name`, `fps`, `regions`, `defaults`, `harness.toml`, and the stamp pin). A second CLI subcommand for generic (two validators drift). |
| D3 | For `generic`: `fps` must be a positive integer rational, not the pinned console value. `pad_layout` must still be byte-identical to §3.4. `regions` is a non-empty list of uniquely named regions whose sizes are multiples of 4096 (F15), or `size: 0` only with `optional: true`; `framebuffer` is optional and, when present, must carry `format`. `mem_mib` is a positive integer. `vcpus` must be 1. `machine.devices` lists exactly `detguest-channel` and `pv-pad`, each once; any other kind (including `virtio-blk` and `pv-net`) or a duplicate is rejected. | Request text: fps is the workload's tick rate; pad_layout byte-identical; regions as declared; framebuffer optional; no new device kinds; D1 of the platform (one vCPU) is ecosystem-wide. | Requiring a `state` region (that is the guest-sdk template's choice, not the manifest contract). Rejecting unknown kinds for `game` too (changes game behavior; `expect_device` ignores extras today). |
| D4 | For `generic`: `meta.built_from.emu_version` is not required and, if present, is ignored. `meta.built_from.repo` is a required non-empty string naming the building repository; `git_rev` is that repository's 40-hex commit; `guest_sdk_rev` is the 40-hex guest-sdk commit the agent was built from (equal to `git_rev` when guest-sdk itself builds the image). `meta.name` matches `^[a-z0-9][a-z0-9-]*$`; `meta.version` is non-empty with no `@` or whitespace, so `<name>@<version>` is an unambiguous registry ref. No value is pinned. | A generic workload has no emulator epoch (F4). The builder repo is guest-sdk, so the xtask crate version is irrelevant. The registry ref must parse. | Requiring `emu_version` with a sentinel value (meaningless data in a contract field). |
| D5 | For `generic`: the bundle must contain `boot.toml`, `expected-regions.toml`, `README.md`, and exactly one of `determinism.unstamped.yaml` or `determinism.last_green`. `harness.toml` is not required. `boot.toml` is checked for `boot_toml_version = 1`, an `[autostart]` table, at least one `[[unit]]` with a non-empty `exec`, and at least one `[[expected_region]]` whose name appears in `regions`. `expected-regions.toml` is checked for `schema_version = 1` and that every `[[regions]]` name appears in `regions` with an equal `size`, and that each name's `layout_version` equals the `boot.toml` entry of the same name. **These bundle-file rules go beyond the request text**; the user approved ratifying them on 2026-10-14. §4.4 cites guest-sdk's `boot.toml` owner doc (guest-sdk API.md §7) for the file's schema and states only the cross-checks normatively, because MAP.md gives guest-sdk the boot manifest and clients must not restate it. | The generic template is a control-free autostart unit (determinism plan `01` template contract). The harness config is reference-workload specific. The cross-check between the three region lists is the one READY-gating invariant a consumer needs. | Reusing `validate_boot_toml` (requires the control leg). Skipping sidecar checks entirely (loses the only READY-gating cross-check). |
| D6 | For `generic`, the green-stamp sidecar uses the same key set as today except: `workload_image` must equal `<meta.name>@<meta.version>` from the manifest; the revision key is `git_rev` (the §4 inline `last_green` name) and must equal `meta.built_from.git_rev`; `emu_version` is not required. The generic unstamped sidecar is validated for `kind: determinism-unstamped`, `workload_image: <name>@<version>`, and `git_rev` equal to `meta.built_from.git_rev` (the game unstamped sidecar stays presence-only, as today). The game green stamp keeps `reference_workload_git_rev` and `emu_version` unchanged. **This key choice goes beyond the request text** and is stated in the response so guest-sdk WP 1.2 can cite it. | §4 already shows `last_green: { git_rev, suite_version }`. Reusing that name avoids inventing a new key. Validating the generic unstamped file keeps §4.3's documented keys honest. | Reusing `reference_workload_git_rev` for a guest-sdk build (misleading). Documenting unstamped keys without checking them (false contract). |
| D6a | §4's YAML example keeps its inline `determinism.last_green` line and gains a trailing comment stating that the stamp is carried as a sidecar file beside the manifest (§4.3) with a string `suite_version`, which is what the validator already enforces for the game variant (F14). No example line is removed. | §4.3 must not contradict the example, the request forbids changing the game variant, and INTEGRATION.md line 193 (not edited by this plan) still describes the stamp loosely as going "into the manifest". An annotation reconciles all three without a game-section rewrite. | Deleting the inline line (changes the game section and strands INTEGRATION.md). Leaving it unannotated (self-contradictory doc). |
| D7 | The generic fixture is checked in under `image/fixtures/generic/` with tiny placeholder `bzImage` and `initramfs.cpio.zst` bytes whose BLAKE3 hashes are written into the fixture manifest. A unit test validates it in place; a second test asserts that the `game` fixture path (`valid_dist()`) still passes. | The request asks for a fixture that validates in the CLI, and guest-sdk needs a reference shape to copy. Placeholder bytes keep the repository small. | Generating the fixture only inside tests (guest-sdk cannot read it). Shipping a real kernel in the fixture (megabytes, and a second kernel pin to drift). |
| D8 | `build_image` and `write_workload_manifest` are not changed. The game manifest keeps omitting `variant`. | Non-goal; keeps `double_build` and the shipped 0.2.0 bundle byte-stable. | Emitting `variant: game` explicitly (changes manifest bytes and the 0.2.0 handoff hash for no consumer benefit). |
| D9 | Request criterion 3 (hypervisor test) is recorded in the response as a determinism-hypervisor deliverable under its WP 1.4 and is not a gate for this plan. §4.4 gains one informative sentence: the manifest `devices` list names workload-visible devices only, and the hypervisor owns how it derives its machine configuration from it (requester's plan WP 1.4). No implicit-device list and no code location are asserted. | The request itself says the hypervisor may land it after ratification; no such code path exists today (F8, F12). The requester's WP 1.4 already names its own change sites; restating them here risks being wrong. | Blocking ratification on another repository's test. Listing the hypervisor's platform devices or a derivation site in this owner doc. |
| D10 | The pull request is the landing mechanism. The implementer pushes the feature branch and opens a PR; merging to `main` is the user's action. The response cites the commit on `origin/main` after the merge. | The user's git conventions permit pushing feature branches and opening PRs but require approval to push to `main`; recent history lands by PR merge (`ba78106`). | Pushing to `main` directly. Writing the response with a feature-branch SHA. |
| D11 | `validate_manifest` keeps its `Result<(), ImageError>` signature. The optional CLI variant suffix uses a separate `pub fn manifest_variant(&Path)`. | Seven callers depend on the current type (F16). | Changing the return type. |

## Change model: validator layering

Before:

```text
validate_manifest(path)
  └─ one flat sequence of checks, all pinned to refwork-demo
```

After (conceptual names; final names chosen in `02`):

```text
validate_manifest(path)
  ├─ read YAML, hash manifest, resolve `variant`
  │    absent → game; unknown → exactly one error, stop
  ├─ shared layer (both variants, all pre-existing checks, unchanged text)
  │    schema_version == 1, kind == workload-image
  │    artifacts: file names + BLAKE3 recomputed
  │    boot: protocol linux-direct, cmdline whitelist
  │    machine.vcpus == 1; pad_layout byte-identical §3.4
  │    no game-like payload files
  ├─ game layer (variant == game)   ← every other check exactly as today
  │    meta.name == refwork-demo, meta.version == xtask version, emu_version non-empty
  │    devices: the three game entries; mem_mib == 128
  │    fps == pinned rational; REQUIRED_REGIONS; defaults demo-game@latest
  │    boot.toml / harness.toml / expected-regions.toml game rules
  │    sidecar: unstamped presence-only, or green stamp with refwork-demo pin,
  │             reference_workload_git_rev, emu_version
  └─ generic layer (variant == generic)   ← all new rules live here
       meta.name charset, meta.version charset, built_from repo/git_rev/guest_sdk_rev
       devices: exactly detguest-channel + pv-pad, no extras, no duplicates; mem_mib ≥ 1
       regions: non-empty, unique, 4096-multiples (0 only if optional), framebuffer needs format
       fps: integers ≥ 1; defaults optional; determinism.suite non-empty
       boot.toml / expected-regions.toml generic rules (D5)
       sidecar: unstamped keys checked, or green stamp with <name>@<version>, git_rev (D6)
```

## Risks, assumptions, and gates

Risks:

- R1: Splitting the validator can silently relax or tighten a game-variant
  check. Gate: every existing negative test must still fail for the same
  error substring, and a characterization test captured before the refactor
  (`03-fixture-and-tests.md`) must show identical error vectors after it.
- R2: The owner document is unversioned, so the §4 edit has no commit and no
  revert, and it is edited before the code is merged. Gates: copy the
  pre-edit file and its SHA-256 to
  `$HOME/.agents/projects/reference-workload/backups/<date>-API.md.pre-variant`
  before editing; §4.1 carries the line `Status: proposed, pending
  ratification by the response file` until WP 4 changes it to `ratified
  <date>`; rollback is restoring the backup only after confirming the
  current file hash equals the post-edit hash recorded in the backup
  directory (so a concurrent edit is never overwritten); the response file
  quotes the amended §4 text verbatim with the file's SHA-256.
- R5: WP 4 waits on an external event (the user merging the PR). The
  session that opens the PR ends in a defined interim state
  (`05-execution-handoff.md`, "Interim end state"), and WP 4 is a resume
  step.
- R3: The guest-sdk builder may emit fields this plan did not anticipate.
  Mitigation: §4 states that unknown optional fields are ignored (F6), and
  the validator must not reject unknown top-level keys. Known-required keys
  are enumerated in the field table in `01`.
- R4: Stamp key naming (D6) is a contract choice guest-sdk will depend on.
  The response file states it explicitly so WP 1.2 can cite it.

Assumptions (not verified in the repository):

- A1: guest-sdk's builder will emit `boot.toml`, `expected-regions.toml`,
  and `README.md` beside the manifest, as the determinism plan's WP 1.2
  output list says. That list does not name a sidecar, but the same plan
  says the manifest is "unstamped sidecar until then", so an unstamped
  sidecar is expected.

Unresolved decisions (all non-blocking; this repository owns the contract
and the fixture defines the accepted shape):

- Q1: Will guest-sdk WP 1.2 emit `determinism.unstamped.yaml` and
  `README.md`, and use the `git_rev` stamp key? Owner: guest-sdk, answered
  by conforming to the response. If it cannot, it files a follow-up request.
- Q2: Whether `image validate` prints the resolved variant on success.
  Owner: implementer; the plan recommends printing it (D11).

Stop/go gates:

- G1 (before WP 2): WP 1's field table is written and internally consistent
  with D3–D6a, and the pre-edit backup of API.md exists. The implementer
  codes from the table, not from prose.
- G2 (before the PR): WP 2 and WP 3 tests pass with
  `cargo test --workspace --locked`, `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` pass, and the CLI
  command in success criterion 2 succeeds.
- G3 (before WP 4): the PR is merged by the user and the merge commit is on
  `origin/main`. Until then the session is in the interim end state and the
  §4 status line reads `proposed`.

## Document map

| File | Purpose |
|---|---|
| `00-overview.md` | This file: context, decisions, success criteria, risks. |
| `01-owner-doc-amendment.md` | WP 1: the exact API.md §4 amendment, field table, and sidecar rules. |
| `02-validator-variant-split.md` | WP 2: the `xtask/src/image.rs` restructuring into schema, game, and generic layers. |
| `03-fixture-and-tests.md` | WP 3: the generic fixture bundle, positive and negative tests, in-place regression test. |
| `04-response-and-ratification.md` | WP 4: response file, request status, tracker close-out, hypervisor hand-off note. |
| `05-execution-handoff.md` | Ordered work packages, commands, gates, definition of done, deferred work. |

## External requests

| Request | Owner | Consumers | Affected WPs | Blocking status | Unblock evidence |
|---|---|---|---|---|---|
| `$HOME/.agents/projects/reference-workload/requests/2026-10-13-generic-workload-image-manifest.md` (inbound; this plan answers it) | reference-workload | guest-sdk WP 1.2 (blocking consumer, demonstrated by the determinism plan); hypervisor WP 1.4, control-plane registry, orchestrator bootstrap (prospective schema consumers) | all | this plan **is** the unblock; nothing blocks this plan | the response file in WP 4 naming the landed commit |

No new outbound request is created by this plan. The hypervisor test
(request criterion 3) is already scheduled by the requester's own plan WP 1.4
and is noted in the response rather than requested again.
