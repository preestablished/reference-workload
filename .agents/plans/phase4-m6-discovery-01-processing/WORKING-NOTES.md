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
