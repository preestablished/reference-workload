# 07 — CI additions: SPC gate, 100k double-run, cross-arch hash compare

**Incremental** — land each gate with the package that makes it meaningful
(noted per item). All CI work uses the synthetic ROM only; demo-game runs
are lab-runner jobs outside this repo's CI.

## Gates to add to `.github/workflows/ci.yaml`

1. **SPC700 corpus gate** (with package 01): mirror the 65816 arrangement —
   corpus pinned in `xtask/test-roms.lock`, `cargo xtask fetch-test-roms`
   into a cached dir, `cargo xtask spc-tests` as a release-mode job step
   with the corpus cache keyed on the lock-file hash. If the 65816 corpus
   job is currently lab-only/nightly rather than per-PR, match whatever it
   does (consistency over ambition); if neither runs in CI, add both as a
   nightly job — the lock-pinned fetch makes them reproducible.
2. **Double-run depth** (with package 02, extended by 03): keep the 10k
   per-PR double-run as is; add a **nightly 100k-frame** double-run +
   zero-alloc release job on the extended synthetic ROM (APU + raster
   segments). 100k on every PR is wasted minutes; nightly catches drift.
3. **Cross-arch chained-hash compare** (with package 02; this is the new
   structural piece):
   - Add an aarch64 job. Preferred: a GitHub-hosted arm64 runner
     (`ubuntu-24.04-arm`) if available to this repo — note these have
     historically been free for **public** repos only; if this repo is
     private, expect this rung to fail immediately and budget the trial
     accordingly. Fallback: the lab's aarch64 box ("the Spark") as a
     self-hosted runner; last resort: `cross`/QEMU-user emulation (slow but
     deterministic — acceptable for nightly). Decide by trying them in that
     order; record the choice in this file when made.
   - **CI ≠ the lab evidence run.** This job covers the *synthetic ROM*
     only. The M2 acceptance run (demo game, 100k frames, real aarch64
     hardware) happens on the provisioned Spark per 06's preconditions —
     QEMU is not a valid substitute there, and provisioning the Spark is a
     pre-06 task regardless of which CI rung wins.
   - Mechanism already exists: `cargo xtask hash-chain` prints the chained
     frame hash (`blake3` chain over `blake3(wram ‖ fb)`; definition moves
     into the shared `refwork-hash` crate per 05). The job runs hash-chain
     (or `refwork-verify double-run --report`, once 05 lands) at fixed
     frame count on both arches and **diffs the two hash values** — a
     job-level compare step that fails on mismatch. Per-PR at 10k frames,
     nightly at 100k.
   - The aarch64 job also runs the plain test suite + clippy once per PR —
     cheap insurance that the workspace builds and tests cross-arch at all.
4. **Gate hygiene for new crates** (with packages 04/05):
   - `ramdiff` and `refwork-verify` join fmt/clippy/test/build jobs
     automatically via `--workspace` — verify no job uses an explicit crate
     list that would skip them.
   - Confirm the deny gate's scope statement still matches reality: it
     covers **three** crates today — `refwork-emu` (now much larger),
     `refwork-harness`, and `refwork-protocol` (`xtask/src/deny.rs`
     ~244–246) — and must **not** be widened to host CLIs (`ramdiff`,
     `refwork-verify`, `refwork-hash` may legitimately use floats for
     `.png` output etc.). Add `refwork-script` (05) to the deny scope
     **only if** `refwork-harness` ends up depending on it later — note
     this in the deny config comment.
   - Windowing deps (`ramdiff --interactive`) must not break headless CI:
     CI runs scripted tests only; if the window crate needs system libs at
     link time even unused, feature-gate the interactive module
     (`--features interactive` locally, default off).
5. **Negative determinism test** (with 05's `double-run`): nightly job
   exercises the test-only divergence hook and asserts `refwork-verify
   double-run` **fails** — the "tests the tester" row of the
   testing-strategy table. **The hook must live in `refwork-verify`, not
   the core**: `xtask/src/deny.rs` token-scans the *source text* of
   `refwork-emu`/`refwork-harness`/`refwork-protocol` for
   clock/RNG/float tokens regardless of cfg gates, so a committed
   `cfg(feature = "nondet-test")` wall-clock read in those crates fails
   the per-PR deny gate, and allow-listing it would weaken the gate.
   `refwork-verify` perturbing its own pad stream / hash input on run 2
   under a test flag achieves the same negative without touching deny
   scope (05 acceptance).

## Acceptance (package-local)

- All new jobs green on main; a deliberately broken case for each proves
  the gate bites (SPC corpus with a forced opcode bug fails; hash-compare
  with mismatched frame counts fails; nondet build fails double-run).
- Per-PR wall-clock budget: the added per-PR jobs (SPC if per-PR, 10k
  cross-arch, new-crate lint/test) keep total CI under a sane bound (~15
  min); everything heavier is nightly.
- CI config contains no reference to game content, lab paths, or operator
  artifacts.
