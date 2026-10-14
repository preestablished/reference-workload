# 03 — Generic Fixture and Tests (WP 3)

Targets: `image/fixtures/generic/` (`new` directory under the existing
`image/`), `xtask/src/image.rs` `mod tests`, `xtask/tests/image_inputs.rs`,
`image/README.md`.

## Goal and prerequisite state

Goal: the request's acceptance criterion 2 is proven by tests and by the CLI:
a `variant: generic` fixture validates, a `variant: game` manifest without
`game-image` fails, and no existing game behavior regressed.

Prerequisite: WP 2 code exists (it may be developed together with this
package in one branch; the tests are written against the §4.2 table, not
against the implementation).

## Repository evidence and patterns

- `valid_dist()` (lines 2177–2210) builds a game bundle in a temp dir from
  `include_bytes!("../../image/boot.toml")` and siblings, writes placeholder
  `bzImage`/`initramfs.cpio.zst` bytes, and calls `write_workload_manifest`
  with the real hashes.
- Negative tests mutate the manifest text with `str::replace` and assert on
  error substrings via `validation_errors()` (line 2229).
- `xtask/tests/image_inputs.rs` asserts the checked-in `image/` inputs exist
  and match schema expectations (lines 26–60).
- `validate_no_game_content` rejects file names containing `game.` and the
  listed extensions (`00-overview.md` F11), so fixture file names must avoid
  them.
- `.gitattributes`/CI do not special-case binary fixtures; the placeholder
  artifact files are a few bytes of text.

## Exact change surface

### Fixture bundle `image/fixtures/generic/` (`new`)

| File | Content |
|---|---|
| `workload-image.yaml` | the manifest below |
| `bzImage` | the ASCII bytes `fixture-kernel\n` (placeholder; not bootable) |
| `initramfs.cpio.zst` | the ASCII bytes `fixture-initramfs\n` (placeholder) |
| `boot.toml` | `boot_toml_version = 1`, `[autostart] unit = 0`, one `[[unit]]` with `id = 0`, `exec = "/usr/bin/generic-workload"`, no `[unit.control]`, one `[[expected_region]] name = "state" layout_version = 1` |
| `expected-regions.toml` | `schema_version = 1`, `schema_owner = "guest-sdk"`, `kind = "expected-regions"`, one `[[regions]]` `name = "state"`, `size = 4096`, `layout_version = 1`, `required = true`, `writable = false` |
| `README.md` | three lines stating this is a validator fixture, that the artifacts are placeholders, and that guest-sdk's builder is the real producer |
| `determinism.unstamped.yaml` | `schema_version: 1`, `kind: determinism-unstamped`, `workload_image: generic-fixture@0.1.0`, `git_rev: "<40 zeros>"`, `reason: "validator fixture; never stamped"` |

Fixture manifest (the `blake3` values are computed from the placeholder
bytes at implementation time and written literally; the test recomputes
them):

```yaml
schema_version: 1
kind: workload-image
variant: generic
meta:
  name: generic-fixture
  version: "0.1.0"
  built_from:
    repo: guest-sdk
    git_rev: "0000000000000000000000000000000000000000"
    guest_sdk_rev: "0000000000000000000000000000000000000000"
artifacts:
  kernel: { file: bzImage, blake3: "<computed>" }
  initramfs: { file: initramfs.cpio.zst, blake3: "<computed>" }
boot: { protocol: linux-direct, cmdline: "quiet" }
machine:
  vcpus: 1
  mem_mib: 64
  devices:
    - { kind: detguest-channel, required: true }
    - { kind: pv-pad, required: true }
regions:
  - { name: state, size: 4096 }   # §3.5: region lengths are multiples of 4096
fps: { num: 60, den: 1 }
pad_layout: <the §3.4 block, byte-identical to write_workload_manifest's>
determinism:
  suite: generic-fixture-gate
```

No `defaults` block (exercises the optional path). No `framebuffer` region.

### Unit tests in `xtask/src/image.rs` `mod tests` (`new` functions)

Add `fn generic_fixture_dir() -> PathBuf` (`proposed`) returning
`Path::new(env!("CARGO_MANIFEST_DIR")).join("../image/fixtures/generic")`
(cargo runs tests with cwd `xtask/`, so repository-relative paths do not
resolve), and `fn generic_fixture_copy() -> TempDir` (`proposed`) that
copies every regular file from that directory into a temp dir with
`std::fs::copy` at runtime, so mutation tests never touch the checked-in
fixture. Reuse `manifest_text`, `write_manifest_text`, and
`validation_errors`.

Characterization test for R1 (write it **before** touching the validator,
run it, then keep it): `game_error_vectors_are_unchanged` builds
`valid_dist()` and applies, one at a time, the mutations already used by the
existing negative tests (vcpus 2, artifact filename drift, cmdline token,
device contract drift, float fps, pad drift, layout_version in regions,
removed `virtio-blk` line, removed `determinism.unstamped.yaml`) plus two
multi-defect mutations (wrong `meta.name` together with a removed
`virtio-blk` line; `vcpus: 2` together with a missing `pad_layout.layout_id`)
and asserts the full ordered `Vec<String>` from `validation_errors` equals
a literal expected vector captured from the pre-refactor validator, so a
change in error order is detected too. Capture the
vectors by running the test once with `assert_eq!(errors, Vec::<String>::new())`
and pasting the printed actuals.

| Test | Setup | Assertion |
|---|---|---|
| `generic_fixture_validates_in_place` | none | `validate_manifest(&generic_fixture_dir().join("workload-image.yaml"))` is `Ok` |
| `generic_fixture_copy_validates` | `generic_fixture_copy()` | `Ok` |
| `game_manifest_without_game_image_rejected` | `valid_dist()`; remove the `virtio-blk` line | errors contain `machine.devices missing virtio-blk` |
| `explicit_variant_game_without_game_image_rejected` | `valid_dist()`; insert `variant: game` after `kind:`; remove the `virtio-blk` line | same substring |
| `generic_with_virtio_blk_rejected` | generic copy; add the game `virtio-blk` line | errors contain `machine.devices unsupported kind virtio-blk` |
| `generic_with_pv_net_rejected` | generic copy; add `- { kind: pv-net, required: true }` | errors contain `machine.devices unsupported kind pv-net` |
| `generic_duplicate_pv_pad_rejected` | generic copy; duplicate the `pv-pad` line | errors contain `machine.devices duplicate kind pv-pad` |
| `generic_missing_pv_pad_rejected` | generic copy; remove the `pv-pad` line | errors contain `machine.devices missing pv-pad` |
| `generic_missing_detguest_channel_rejected` | generic copy; remove that line | errors contain `machine.devices missing detguest-channel` |
| `unknown_variant_rejected` | generic copy; `variant: generic` → `variant: sandbox` | errors equals exactly `["variant must be game or generic, got sandbox"]` |
| `generic_name_charset_enforced` | generic copy; `name: generic-fixture` → `name: Generic_Fixture` | errors contain `meta.name` |
| `generic_version_rejects_at_sign` | generic copy; `version: "0.1.0"` → `version: "0.1.0@x"` | errors contain `meta.version` |
| `generic_git_rev_must_be_40_hex` | generic copy; shorten `git_rev` to 39 zeros | errors contain `meta.built_from.git_rev` |
| `generic_requires_pad_layout_identity` | generic copy; `name: Up` → `name: UP` | errors contain `pad_layout.buttons[6].name` |
| `generic_fps_must_be_integer_rational` | generic copy; `num: 60` → `num: 60.0` | errors contain `fps.num` |
| `generic_fps_zero_den_rejected` | generic copy; `den: 1` → `den: 0` | errors contain `fps.den must be an integer >= 1` |
| `generic_framebuffer_requires_format` | generic copy; add `- { name: framebuffer, size: 1024 }` to regions | errors contain `regions.framebuffer.format` |
| `generic_framebuffer_with_format_accepted` | same with `format: xrgb8888-16x16-stride64` | `Ok` |
| `generic_duplicate_region_rejected` | generic copy; duplicate the `state` row | errors contain `regions duplicate name state` |
| `generic_region_size_must_be_page_multiple` | generic copy; `size: 4096` → `size: 256` in the manifest | errors contain `regions.state.size must be a multiple of 4096` |
| `generic_optional_zero_size_accepted` | generic copy; add `- { name: scratch, size: 0, optional: true }` | `Ok` |
| `generic_mem_mib_zero_rejected` | generic copy; `mem_mib: 64` → `mem_mib: 0` | errors contain `machine.mem_mib must be an integer >= 1` |
| `generic_requires_an_expected_region` | generic copy; delete the `[[expected_region]]` block from `boot.toml` | errors contain `boot.toml must declare at least one [[expected_region]]` |
| `generic_expected_region_must_be_declared` | generic copy; change `boot.toml` expected_region name to `other` | errors contain `boot.toml expected_region other is not declared in regions` |
| `generic_expected_regions_size_must_match` | generic copy; `size = 4096` → `size = 8192` in `expected-regions.toml` | errors contain `expected-regions.toml state size 8192 != manifest size 4096` |
| `generic_layout_version_must_agree` | generic copy; `layout_version = 1` → `layout_version = 2` in `expected-regions.toml` only | errors contain `expected-regions.toml state layout_version 2 != boot.toml 1` |
| `generic_unstamped_sidecar_keys_checked` | generic copy; `workload_image: generic-fixture@0.1.0` → `workload_image: other@0.1.0` in the unstamped sidecar | errors contain `determinism.unstamped.workload_image` |
| `generic_boot_toml_comment_does_not_count_as_table` | generic copy; replace `[autostart]` table with a comment line `# [autostart]` | errors contain `boot.toml missing [autostart]` |
| `generic_boot_toml_trailing_table_not_attributed_to_region` | generic copy; append `[extra]\nlayout_version = 9` after the `[[expected_region]]` block in `boot.toml` | `Ok` |
| `variant_null_rejected` | generic copy; `variant: generic` → `variant:` (null) | errors equals exactly one element starting with `variant must be game or generic` |
| `generic_does_not_require_harness_toml` | generic copy (no `harness.toml`) | `Ok` (covered by `generic_fixture_copy_validates`; keep as an explicit name for readability or fold it in) |
| `generic_does_not_require_emu_version` | generic copy | `Ok`; and a second step adds `emu_version: "x"` under `built_from` and still passes |
| `generic_green_stamp_accepted` | generic copy; delete the unstamped sidecar; write `determinism.last_green` with `workload_image: generic-fixture@0.1.0`, `git_rev` = 40 zeros, `image_manifest_hash` = BLAKE3 of the manifest, and the other required keys | `Ok` |
| `generic_green_stamp_wrong_workload_image_rejected` | as above with `workload_image: refwork-demo@0.2.0` | errors contain `determinism.last_green.workload_image` |
| `generic_green_stamp_requires_git_rev` | as above without `git_rev` | errors contain `determinism.last_green.git_rev` |
| `generic_unknown_top_level_key_ignored` | generic copy; add `x_builder_note: hello` at top level | `Ok` |
| `game_error_vectors_are_unchanged` | see above | exact vector equality for each mutation |

The game anchor is `valid_dist()` through the existing
`validator_accepts_generated_manifest_shape` (line 2283). Do not add a test
that reads `dist/`; it is untracked (F13) and would pass vacuously in CI.

Every pre-existing test keeps its name and assertions.

### Integration test additions in `xtask/tests/image_inputs.rs`

- Extend `required_image_inputs_exist` (line 26) with the seven fixture
  paths under `image/fixtures/generic/`.
- Add `generic_fixture_declares_no_game_device` asserting the fixture
  manifest text contains `variant: generic` and does not contain
  `game-image`.

### `image/README.md`

Two edits:

1. Add one paragraph after the paragraph that starts "No game ROM, SRAM,
   framebuffer golden" (line 19): the `fixtures/generic/` directory is a
   validator fixture for the `generic` manifest variant defined in API.md
   §4.1, its artifacts are placeholders, guest-sdk's generic builder is the
   producer, and the fixture is exercised by `cargo test -p xtask`.
2. In the paragraph that begins "The generated manifest records the
   emulator epoch" (line 28 onward), qualify the `emu_version` requirement
   and the stamp rule as game-variant only, with a pointer to API.md §4.2.

### CLI proof (recorded in WP 4)

```sh
cargo run --locked -p xtask -- image validate image/fixtures/generic/workload-image.yaml
```

Must print `image validate: OK - ...`. Validating a local
`dist/workload-image-0.2.0/` bundle in place is an optional extra check on a
box that has it (untracked, F13); it is not a gate.

## Acceptance

1. All tests in the table exist and pass under `cargo test --locked -p xtask`.
2. `cargo test --workspace --locked` passes (CI job at
   `.github/workflows/ci.yaml` line 48).
3. The fixture CLI command succeeds from a clean checkout.
4. `git status` shows no unintended tracked changes; the new tracked files
   are the fixture files, the test edits, and `image/README.md`. The
   pre-existing untracked paths listed in `00-overview.md` Constraints stay
   unstaged.

## Dependencies, risks, exclusions

- Depends on WP 2 for green tests; may be authored first (red) as a guide.
- Risk: `include_bytes!` style used by `valid_dist()` embeds files at
  compile time; for the generic copy use runtime `std::fs::copy` so the
  fixture directory can gain files without recompiling the test list.
- Exclusion: no CI workflow edit. Issue `refwork-425` tracks running image
  commands in CI and is not part of this plan.
