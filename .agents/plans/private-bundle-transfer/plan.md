# Private bundle transfer to infra-control

## Scope

Copy the private map v3 bundle and host capture indexes from the operator Mac to `infra-admin@100.82.43.93` so plan packages 03 and 04 can run there. The procedure reuses the manifest-and-verify steps of `../discovery-02-session-transfer/plan.md`. Both machines resolve the private root (`$PR`) from `~/.agents/projects/reference-workload/private-root.path`; the relative layout is identical on both.

Trees: `$PR/bundle/`, `$PR/host-index/` (`main/`, `death/`, `timer/`), `$PR/bundle-draft/` (v2 history, read-only reference), `$PR/discovery-02/`, and the single file `$PR/evidence/forbid-list.txt`. Excluded: ROMs, ramdiff sessions (already on the host), `derived-*`, `replay-verify-*`, and everything else under `$PR/evidence`.

## Execution record — 2026-09-17

- No destination tree existed beforehand, so nothing was replaced. Files were copied with `rsync -a --checksum --files-from` over batch-mode SSH into a mode-700 staging directory under the host's `$PR`, verified, and renamed into place; staging was removed.
- 2,578 files, 9,209,863 bytes: `bundle/` 46, `host-index/` 2,366, `bundle-draft/` 3, `discovery-02/` 162, `evidence/forbid-list.txt` 1. One Python bytecode cache file under `discovery-02/` was left out as generated.
- The sorted relative-path SHA-256 source manifest was identical before and after transfer and ships as `$PR/SHA256SUMS` on the host; its SHA-256 is `75f6b896eda9db5da996528d393b9132ba4cffe81ca07bf074a1b072169565fd`.
- The host passed `sha256sum -c` for every file in staging and again after publication, with no extra or missing files, no symlinks, and no `._*` sidecars. Group/other permission bits were cleared; the host `$PR` is mode 700.
- `$PR/evidence/private-bundle-transfer-02.txt` holds the same record on both machines (hashes match).
- Naming note for packages 03/04: the v3 `bundle/validation/` carries per-session `map-check.expect-{main,death,timer}.yaml`; the single `map-check.expect.yaml` named in older runbooks exists only in `bundle-draft/`.
