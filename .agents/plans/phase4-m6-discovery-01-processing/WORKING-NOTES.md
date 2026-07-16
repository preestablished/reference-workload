# Working Notes — phase4-m6-discovery-01-processing execution

## Package 01 (2026-07-16) — PASSED

- Private-root pointer file written to
  `~/.agents/projects/reference-workload/private-root.path` (mode 600,
  1 line). Value resolved from the migrated m6 host state (memory:
  m6-host-teardown migration); the discovery-01 session and ROMs dir were
  found under it, satisfying grounding note 7's "where the approved session
  landed" definition. Reconfirm at STOP #1.
- Session integrity: 16 labels / `log_frames: 45230` / 45231 padlog lines /
  16 dumps, all 131072 B / `candidates.offsets: []`. `chmod -R go-rwx`
  applied.
- Step 2b repo-wide tracked-file scan: exactly one hit —
  `tools/record-ramdiff` (the known pre-existing GATE-RECORD-ASK1 violation,
  commit `5b35113`). No other hits.
- Builds: `ramdiff` + `refwork-verify` release binaries built `--locked`.
- pyyaml: PEP 668 blocked `pip --user`; no brew formula; **venv route used**:
  `~/.venvs/refwork` with pyyaml installed. Every pipeline invocation that
  needs the layout stage must run with
  `PATH="$HOME/.venvs/refwork/bin:$PATH"`.
- ROM identity: exactly 1 file under `$PR/ROMs`; b3sum recorded in
  `$PR/evidence/rom-identity.txt`.
- Replay-fidelity gate: full 45,230-frame scripted replay of
  `interactive.padlog` with 16 marks → **16/16 IDENTICAL**
  (`$PR/evidence/replay-fidelity-01.txt`), zero faults
  (`$PR/evidence/replay-verify-01.stderr` empty of faults).
- Double-run: `deterministic: true`, `frames_run: 45230`,
  `first_divergent_frame: null` (`$PR/evidence/double-run-45230.json`).

## Package 02 (2026-07-16, in progress)

### Beads DB restore

- Loss confirmed real: `bd list` → "no beads database found"; git history
  for `.beads` holds only config/metadata/README/.gitignore (commit
  `8c21d5d`); the stray `~/git/beads/issues.jsonl` is the beads project's
  own tracker (`bd-main-` prefix, zero `refwork-` IDs); backup branch
  `origin/backup/m6-host-20260715` carries no DB/JSONL.
- `BD_NON_INTERACTIVE=1 bd init --prefix refwork` succeeded over the
  existing config/metadata.
- **Push-safety hardening** (deviation worth noting): first `bd create`
  attempted a **Dolt auto-push** to a `origin` Dolt remote auto-configured
  from the git remote (push failed: "no common ancestor"; nothing left the
  machine). Response: `backup.git-push: false` and `dolt.auto-push: false`
  set explicitly in `.beads/config.yaml`, and the Dolt remote removed
  entirely (`bd dolt remote remove origin`; `bd dolt remote list` → none).
  bd can no longer push anything without a deliberate re-add.
- Recreated with original IDs (all accepted by `bd create --id`):
  `refwork-czi` (closed with restored closure reason), `refwork-20v`,
  `refwork-5tk` (dep → 20v), `refwork-5be` (reconstructed from
  `.agents/handoffs/m6-scoring-handoff-for-state-scorer.md`, left open),
  and the M6 bead as `refwork-ob3` (fresh hash — no original ID existed;
  created verbatim per scoring-goal package 01 step 1, deps → czi, 20v; no
  5tk edge per that plan's rationale).
- Provenance-line date: plan text said "Recreated 2026-07-15"; the actual
  recreation happened 2026-07-16, so descriptions say 2026-07-16
  (truthfulness over template).

### API.md ratification stamps (re-applied 2026-07-16)

Source: state-scorer
`.agents/requests/phase4-m1-m4-first-boss-scoring/05-refwork-spec-ratification.md`
(ratified 2026-07-12). Stamps were verified absent beforehand (`grep -i
ratified` → none; that docs tree is not a git repo). Re-applied to
`~/.agents/projects/determinism/docs/reference-workload/API.md`:

1. §1.2 discretize `threshold` note — bin = count of edges ≤ value; an edge
   value belongs to the interval to its right.
2. §1.2 guard-semantics paragraph (after the feature-entry example) — leaf
   over a failed `valid_when` guard evaluates false; `not{leaf}` therefore
   TRUE; author warning about `not{}` over guarded features.
3. §2.3 bit-range strictness — compile-time rejection of
   `bit >= feature width` is normative (stricter than schema `0..=31`).

Each stamp carries "ratified 2026-07-12" + the ratification-doc path.
Verification: `grep -c 'ratified 2026-07-12' API.md` → 3. If a future
session finds them absent again, the docs tree was overwritten — re-apply
from the ratification doc, which remains authoritative.

### Commit-gate note (applies to every commit this plan makes)

The user-level pre-commit hook runs `cargo test --workspace`, which cannot
build on this Mac: the guest-sdk sibling's `detguest-sdk` uses
`libc::SOCK_CLOEXEC`, absent on apple targets (pre-existing; guest-sdk local
== origin/main, no upstream fix). Commits use the hook's documented
`--no-verify` bypass, with the change surface verified directly instead
(`bash -n`, live runs, redaction scans). Follow-up candidate: a macOS
`FD_CLOEXEC` fix in guest-sdk (own repo, own review).

## Package 03 (2026-07-16) — PASSED

- Idle/movement analysis: 44 all-zero-pad idle runs (≥60 frames); chosen
  window W1-S1. `idle-a`=6500, `idle-b`=6560 (both inside idle run
  6468–6737); `move-b`=7040 (end of sustained right-hold 6868–7045).
- Grid marks (47): capacity-pickup 12400–12900/50, midboss 19500–23000/250,
  boss 30250–37000/500, upgrade 40000–41500/250. Private files:
  `$PR/discovery/{idle-runs,move-frames,grid-marks}.txt`.
- `$PR/discovery/frame-plan.yaml`: 13 feature entries (dual-width rows for
  health/max_health/upgrade_flags/boss_flags), 2 recorded gaps
  (credits_flag, game_mode dead value → STOP #1).
- `derived-01` session: single replay pass, 50/50 dumps, zero faults.
- `tools/m6-discovery-analyze2.sh` added (plan-driven; original tool
  untouched). `bash -n` clean; `--self-test` (synthetic planted-byte vs the
  real binary) PASSES; dry run produced candidate counts for all 13
  features → `$PR/discovery/analysis2-report.txt`.
- `discovery-01/session.yaml` pristine (`candidates.offsets: []`;
  b3sum prefix 04c11d86bdc4f65c recorded for future comparison).
- **Privacy deviation (fixed):** the analyzer's first version echoed the
  report path (under the private root) to stderr → it appeared in terminal
  output once. Tool corrected to never echo the report path. The occurrence
  was in this session's transcript only — no file/commit/bead carries it.
- Dry-run counts are wide for game_mode/area_id/room_id/upgrade_flags
  (4.5k–9k) — expected: package 04 narrows with grid pairs and exclusion
  sets. health (57–91), player_x/y (358), boss_flags (108–176),
  max_health (175–252) are already tractable.
