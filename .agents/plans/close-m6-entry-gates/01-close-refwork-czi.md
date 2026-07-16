# Package 01 — Close `refwork-czi` (Agent-Only, Start Immediately)

## State

The exporter implementation is **done and uncommitted** in the working
tree: modified `Cargo.lock`, `crates/refwork-dh-client/src/mock.rs`,
`crates/refwork-verify/{Cargo.toml,src/lib.rs,src/main.rs,src/phase4_bundle_check.rs,src/phase4_checksum_manifest.rs,src/phase4_context_check.rs,tests/integration.rs}`,
three `docs/phase4-corpus-guide/*.html`; untracked
`crates/refwork-verify/src/phase4_{artifact_check,capture_export,context_export,fallback_check}.rs`
and `data/`. The czi bead's 2026-07-11 comment records all locked gates
green at base `4eb8a3a`; main has since advanced by doc/tooling-only
commits (`3a53298`, `709b075`, `b16fa72`, `98f81f3` — no code overlap),
so a re-run is expected green but is **not optional**.

## Steps

1. **Inspect before committing.** `git diff --stat` and read enough of the
   diff to confirm it matches the czi bead's description (exporter,
   artifact-check, context-export, checksum --verify, fallback validator,
   docs, synthetic tests) and contains **no private payloads**: grep the
   diff for absolute home paths, hex offsets outside test fixtures, and
   anything under `data/`. On `data/` specifically: it is snapstore
   runtime scratch (`STORE_VERSION`, `meta/tree.db`, `store/pages/*.spk`,
   and a `snapstore.sock` socket), referenced by nothing in the czi diff —
   **leave it entirely untracked** and record that disposition in the
   evidence note. Do not delete it without first confirming no live
   process is bound to the socket (`ss -xl | grep snapstore` or `fuser`);
   if in doubt, leave it alone.
   Do not rubber-stamp: this is the quality gate for another agent's work.

2. **Run the package-02 verification suite** (fast-follow
   `02-exporter-implementation.md` "Verification") on the current tree:

   ```sh
   cargo fmt --all -- --check
   cargo test --locked -p refwork-dh-client
   cargo test --locked -p refwork-verify phase4_capture_export -- --nocapture
   cargo test --locked -p refwork-verify phase4_artifact -- --nocapture
   cargo test --locked -p refwork-verify phase4_context_export -- --nocapture
   cargo test --locked -p refwork-verify phase4_checksum -- --nocapture
   cargo test --locked -p refwork-verify phase4_fallback -- --nocapture
   cargo test --locked -p refwork-verify phase4 -- --nocapture
   cargo test --locked -p refwork-verify
   git diff --check
   ```

   (This list is a transcription of the runbook's Verification block — if
   the runbook has changed, the runbook wins.) `--locked` is unaffected by
   the tree being git-dirty; it only checks Cargo.lock vs Cargo.toml.

   Plus the synthetic end-to-end: one exporter run into a temp directory
   whose rows `phase4-bundle-check` accepts (there is a test for this in
   the suite; confirm it actually executes rather than assuming).
   Any failure stops this package — diagnose, don't massage.

3. **Commit** the czi files (explicit paths, never `git add -A`), message
   explaining what the exporter wave delivers and citing the bead. Keep
   the M6/plan files out of this commit — one logical unit.

4. **Final clean-checkout gate**: fresh worktree at the new commit —
   **placement is load-bearing**: the workspace has path dependencies on
   sibling checkouts (`../control-plane`, `../determinism-hypervisor`,
   `../guest-sdk`), so the worktree MUST be a sibling of
   `reference-workload` itself, e.g.
   `git worktree add ../reference-workload-czi-clean-checkout <SHA>`;
   a worktree in /tmp or the scratchpad fails `cargo metadata --locked`
   with a missing-path error that looks like (but is not) a code problem.
   Verify `cargo metadata --locked` resolves there first, then run the
   full suite from step 2 (`--locked`), record
   command output summary (pass counts, SHA) — this is the recorded
   evidence the bead's hold-open condition names. Remove the worktree
   after; beware the cwd-drift gotcha (cd back to repo root).

5. **Close the bead** with evidence:

   ```sh
   bd close refwork-czi -r "Committed at <SHA>; clean-checkout gate green (<N> tests, fmt, diff-check) recorded <date>; synthetic e2e export accepted by phase4-bundle-check. Evidence: <path/summary>."
   ```

   Record the same summary in a dated note under
   `.agents/plans/close-m6-entry-gates/EVIDENCE-czi.md` (public-safe:
   counts, SHAs, no private content).

## Exit signal

`bd show refwork-czi` = CLOSED; `tools/m6-gate-check.sh` shows gate 2
PASS; working tree contains no czi remnants (only intentionally untracked
private/scratch paths, each named in the evidence note).
