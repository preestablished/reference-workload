# 01 — Owner Document Amendment (WP 1)

Target: `$HOME/.agents/projects/determinism/docs/reference-workload/API.md`
§4 "WorkloadImage artifact manifest" (lines 530–616, the final section of the
file). The file is not under version control (see `00-overview.md`
Constraints), so the implementer edits it in place and later quotes the result
in the response file (WP 4).

## Goal and prerequisite state

Goal: §4 defines `variant` and states, per top-level field, the rule under
`game` and under `generic`, so that the validator in WP 2 and guest-sdk's
builder implement the same contract from the same table.

Prerequisite: WP 0 (tracker issue) and a pre-edit backup: copy API.md and
its `sha256sum` output to
`$HOME/.agents/projects/reference-workload/backups/<date>-API.md.pre-variant`
(create the directory) before the first edit, and record the post-edit
hash beside it after the edit. Rollback for this package is restoring that
copy, only after checking the live file still has the post-edit hash. Gate G1 in
`00-overview.md` requires this file's table to be internally consistent with
decisions D3–D6a before WP 2 starts.

## Repository evidence

- §4 today is one YAML example (lines 538–604) followed by one
  "Validation:" paragraph (lines 606–616). There is no field table.
- The example shows `determinism.last_green` inline with an integer
  `suite_version` (lines 603–604), and INTEGRATION.md line 193 says the stamp
  is written "into the manifest". The validator reads sidecar files only
  (`xtask/src/image.rs` lines 1492–1638) with a string `suite_version`
  (`dist/workload-image-0.1.0/determinism.last_green`). The amendment
  annotates the example line rather than removing it (D6a).
- The §4 example omits `meta.built_from.emu_version`, which this repository
  requires for the game bundle (`xtask/src/image.rs` lines 287–304). The
  amendment documents it as a game-variant field so the doc and the
  validator agree.
- API.md lines 11–14 already state the forward-compatibility rule: unknown
  optional fields are ignored, unknown required-context fields are rejected.
  `variant` is an optional field with a default, so its absence is valid.
- The sidecar file names and keys the validator enforces
  (`determinism.unstamped.yaml`, `determinism.last_green`, keys in
  `validate_green_stamp_sidecar`) are not in §4 today; they are described
  only in `image/README.md` and the validator. The amendment adds a short
  sidecar subsection because the generic stamp producer lives in another
  repository and must know the keys.

## Exact change surface

Edit API.md §4 only. Do not edit §1–§3, `MAP.md`, `INTEGRATION.md`, or
`IMPLEMENTATION-PLAN.md`.

1. In the YAML example (line 539 onward): insert one line after
   `kind: workload-image`:
   `variant: game                  # optional; game (default) | generic (§4.1)`;
   add `emu_version: "<string>"  # game variant only` under
   `meta.built_from`; append to the existing `last_green:` line (604) the
   comment `# carried as the sidecar file determinism.last_green (§4.3); suite_version is a string there`.
   Remove no line. Make no other change to the example.
2. Replace the single "Validation:" paragraph (lines 606–616) with the
   subsections below, keeping every existing rule for the game variant.
3. Append subsections §4.1 through §4.4 at the end of the file.

## Normative content to write

### §4.1 `variant`

- First line of the subsection: `Status: proposed 2026-10-14, pending
  ratification by the response file` (WP 4 rewrites it to `Status: ratified
  <date>, response <path>`).
- `variant` is an optional top-level string. `null`, the empty string, and
  any non-string value are invalid, like any unknown value. Allowed values: `game`,
  `generic`. Absent means `game`. Any other value is invalid and the
  validator names the offending value.
- `game` is the console-emulator workload shipped by reference-workload.
  Its rules are unchanged from the pre-amendment §4.
- `generic` is a workload built with the guest-sdk generic template. It has
  no game image and may have no framebuffer. Its input transport is still
  `pv-pad`.
- Exclusions stated in the doc: no new device kinds; `pv-net` is a later
  request; no change to the game variant.

### §4.2 Field table

Write this table. "Pinned" means the exact value in the §4 example.
"Declared" means the workload chooses the value within the stated type.

| Field | `game` | `generic` |
|---|---|---|
| `schema_version` | 1 | 1 |
| `kind` | `workload-image` | `workload-image` |
| `variant` | absent or `game` | `generic` |
| `meta.name` | pinned `refwork-demo` | declared; matches `^[a-z0-9][a-z0-9-]*$` |
| `meta.version` | pinned to the generating xtask version | declared, non-empty, no `@` and no whitespace (so `<name>@<version>` is a registry ref) |
| `meta.built_from.repo` | `reference-workload` (documented; presence only is validated) | declared, non-empty: the repository that ran the build |
| `meta.built_from.git_rev` | present (presence only is validated) | required 40 lowercase hex: the commit of `built_from.repo` |
| `meta.built_from.guest_sdk_rev` | present (presence only is validated) | required 40 lowercase hex: the guest-sdk commit the agent was built from (equal to `git_rev` when guest-sdk builds the image) |
| `meta.built_from.emu_version` | required non-empty | not required; ignored if present |
| `artifacts.kernel` | `file: bzImage` + BLAKE3 recomputed | same |
| `artifacts.initramfs` | `file: initramfs.cpio.zst` + BLAKE3 recomputed | same |
| `boot.protocol` | `linux-direct` | `linux-direct` |
| `boot.cmdline` | append-only whitelist (existing rule) | same |
| `machine.vcpus` | 1 | 1 |
| `machine.mem_mib` | pinned 128 | declared, integer ≥ 1 |
| `machine.devices` | at least: `virtio-blk` role `game-image` readonly required; `detguest-channel` required; `pv-pad` required (other kinds are not rejected; existing behavior) | exactly: `detguest-channel` required and `pv-pad` required, each once. Any other kind (including `virtio-blk` and `pv-net`) or a duplicate kind is rejected. |
| `regions` | `wram`, `framebuffer` (with pinned format), `meta` required with pinned minimum sizes; optional `vram`, `sram` | declared, non-empty, unique names; each `size` is an integer multiple of 4096 (§3.5), or `0` only together with `optional: true`; `optional` is a boolean if present; `note` is free text; `framebuffer` optional and, if present, must carry `format`; no `layout_version` keys |
| `fps` | pinned rational | declared `num` ≥ 1, `den` ≥ 1, integers only |
| `pad_layout` | byte-identical to §3.4, `layout_id console16-12btn-v1` | same (unchanged requirement) |
| `defaults` | pinned `demo-game@latest` for both refs | optional; if present, both refs are non-empty `name@version` strings |
| `determinism.suite` | `refwork-verify` (documented; presence only is validated) | declared, non-empty |
| `determinism.last_green` | required for scheduling (sidecar, §4.3) | required for scheduling (sidecar, §4.3) |

### §4.3 Determinism sidecars

State that the stamp is carried as a sidecar file beside the manifest, not
inline, and that exactly one of the two must exist:

- `determinism.unstamped.yaml`: `schema_version: 1`,
  `kind: determinism-unstamped`, `workload_image: <name>@<version>`,
  `git_rev`, `reason`. For `game` it also carries `emu_version`. The gate
  validates `kind`, `workload_image`, and `git_rev` for `generic`; for
  `game` it checks presence only (existing behavior).
- `determinism.last_green`: `schema_version: 1`,
  `kind: determinism-last-green`, `workload_image: <meta.name>@<meta.version>`,
  `image_manifest_hash` (BLAKE3 of `workload-image.yaml`), `suite_version`
  (a non-empty string), `timestamp`, `suite_report_blake3` (64 lowercase
  hex). The revision key is `reference_workload_git_rev` for `game` and
  `git_rev` for `generic`; both must equal `meta.built_from.git_rev`. `game`
  also requires `emu_version` equal to `meta.built_from.emu_version`.
- Who stamps: `game` is stamped by this repository's determinism suite
  (`refwork-verify`); `generic` is stamped by the workload's own green-stamp
  gate (the requester's plan criterion 2 run). control-plane refuses to
  schedule exploration experiments on an unstamped image of either variant.

### §4.4 Bundle files

- `game`: `workload-image.yaml`, `bzImage`, `initramfs.cpio.zst`,
  `boot.toml`, `harness.toml`, `expected-regions.toml`, `README.md`, one
  sidecar. Existing rules for `boot.toml` and `harness.toml` apply.
- `generic`: `workload-image.yaml`, `bzImage`, `initramfs.cpio.zst`,
  `boot.toml`, `expected-regions.toml`, `README.md`, one sidecar.
  `harness.toml` is not required and is ignored if present. The schema of
  `boot.toml` and `expected-regions.toml` is owned by guest-sdk (guest-sdk
  API.md §7 and its expected-regions handoff format); this section does not
  restate it. The gate reads only these facts from them and applies these
  cross-checks: `boot.toml` has `boot_toml_version = 1`, an `[autostart]`
  table, at least one `[[unit]]` with a non-empty `exec`, and at least one
  `[[expected_region]]`; it is not required to have `[unit.control]`. Every
  `[[expected_region]]` name in `boot.toml` and every `[[regions]]` name in
  `expected-regions.toml` must appear in the manifest `regions`;
  `expected-regions.toml` sizes must equal the manifest sizes; and for each
  name present in both TOML files the `layout_version` values must be equal.
- Informative sentence for the hypervisor: `machine.devices` names the
  workload-visible devices only; how the hypervisor derives its machine
  configuration from it is the hypervisor's own contract (requester's plan
  WP 1.4; request criterion 3).
- Both: no file whose name contains `game.` or whose extension is one of
  `rom`, `sfc`, `smc`, `srm`, `sav` may exist under the bundle directory.

### Closing sentence of §4

Keep the existing statement that control-plane validates only
checksum/size/`schema_version` at registration and that the deeper checks are
this repository's `xtask image validate` gate, and add that the gate selects
its rule set from `variant`.

## Intended behavior and invariants

- The amendment is additive. Every sentence that applied to the only
  pre-existing shape now applies to `game`.
- The table is the single source for WP 2. Where the table and the YAML
  example disagree, the table wins; fix the example.
- No console, vendor, or case-study name is introduced (clean-room
  boundary).

## Acceptance

1. The field table exists with exactly the two variant columns and every
   row above.
2. §4.1–§4.4 exist with the content above.
3. The YAML example parses as YAML after the edit (extract the fenced block
   under §4 and feed it to any YAML parser; if PyYAML is absent, paste the
   block into `serde_yaml` via a throwaway test). The example no longer
   contains an inline `last_green` key.
4. `grep -c "variant" API.md` is at least 8 (example line, §4.1 heading and
   body, table rows, closing sentence).
5. `docs/MAP.md` is byte-identical to its pre-amendment state
   (`sha256sum` before and after).
6. The pre-edit backup, its SHA-256, and the post-edit SHA-256 exist under
   `$HOME/.agents/projects/reference-workload/backups/`.

## Dependencies, risks, exclusions

- Depends on nothing. Blocks WP 2 and WP 3.
- Risk: wording drift between this file and the doc. Mitigation: copy the
  table verbatim.
- Exclusion: do not restate the §3.4 button table inside §4.1–§4.4; refer to
  §3.4.
