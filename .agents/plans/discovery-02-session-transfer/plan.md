# Discovery-02 session transfer to infra-control

## Scope

Copy the four completed, final-linted and replay-verified hand-play sessions from the local private root used by `tools/record-ramdiff` to `infra-admin@100.82.43.93`. The canonical remote location is `~/.agents/private/reference-workload/discovery-02-sessions/`, outside any source checkout. Include each session's `session.yaml`, `interactive.padlog`, dumps, and `SESSION-NOTES.md`, plus its final lint and complete replay evidence: `replay-fidelity-*.txt`, `double-run-*.json`, `replay-verify-*.stderr`, and `double-run-*.stderr`. No ROM or full private root is transferred.

Sessions: `discovery-02-main`, `discovery-02-gameover-death`, `discovery-02-gameover-timer`, `discovery-02-score-currency`. The score/currency lint has a documented waiver for two `both-*` labels.

## Execution

1. Recheck the four exact local directories and the latest final lint and replay-fidelity verdicts; exclude `.bak-*`, `take2`, derived, and replay-verify directories. Check for active recorder/replay writers. The death session notes were edited after its reports; inspect their consistency and rerun final lint. Require batch-mode SSH access and enough remote disk space.
2. Create a sorted relative-path SHA-256 manifest of every selected source file before transfer. Create a mode-700 remote staging directory under `~/.agents/private/reference-workload/`. Copy only the four named directories and their twenty named evidence files with `rsync` over SSH. Preserve the original session names and a `ramdiff/` and `evidence/` layout. Ensure the staging root and canonical root stay mode 700; inspect ancestor and child permissions.
3. Recompute the source manifest after transfer and require byte-for-byte equality with the first manifest. Transfer that manifest as `SHA256SUMS` to staging. Verify every hash remotely, compare file counts and byte totals, and confirm the remote tree has no symlinks or extra files. On failure, keep staging for diagnosis and do not publish it as the canonical location.
4. Rename the verified staging directory to the canonical location if absent. If the canonical location already exists, verify its manifest against the source: identical is success; a difference stops publication and documentation. Recheck permissions and manifest at the canonical location.
5. Record the host, relative location, four names, included evidence, verdicts, manifest validation, date, and retrieval layout (`ramdiff/<name>/`, `evidence/`, `SHA256SUMS`) in the `reference-workload` project in Beans through `bn remember --key discovery-02-session-transfer` (which commits and pushes the shared hub). Keep ROM identity, hashes of ROM/WRAM, private absolute paths, and dump contents out of Beans. Verify `bn status --json` is synchronized and the memory can be read.

## Validation and recovery

The SHA-256 manifest is private transfer evidence stored with the remote copy. Source files stay in place. If transfer or validation fails, retry into the staging directory; do not replace a published destination without comparing manifests first. This is a copy and a Beans metadata write; no compatibility migration is needed.

## Execution record — 2026-09-16

- The four exact sessions and twenty evidence files were copied to the canonical host location above. The death session final lint was rerun after its notes edit and passed.
- The source manifests matched before and after transfer. The host verified all 101 files (9,684,960 bytes) against `SHA256SUMS`, checked that the file set had no extras or symlinks, and verified the manifest again after publication. The published directory is mode 700.
- The project memory `discovery-02-session-transfer` in Beans records retrieval and evidence details. Main and death operator notes retain unfilled counter/cause placeholders; those fields should be checked before interpretation.
