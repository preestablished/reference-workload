# Review resolutions (two independent reviews, 2026-07-15)

Both reviewers verified the core frame-alignment analysis (dump at frame `F` ⇒
log holds ≥ `F+1` lines; replay compares after index `F`; holds across resumed
segments; replay/integrity failures occur before the log is opened for append).
Findings and dispositions:

## Accepted — changes the plan

1. **[both, must-fix] Stale `session.yaml` after in-place padlog rotation.**
   Rotating only the padlog leaves old dumps in `session.yaml`; the next resume
   is then falsely refused (stage 2) or falsely flagged divergent (stage 3),
   and new dumps can overwrite `.bin` files the rotated log's dumps reference.
   **Resolution: drop in-place rotation entirely.** Fresh interactive mode now
   *refuses* to start in a session dir that already contains a padlog with pad
   lines or a `session.yaml` with dumps, directing the user to the wrapper
   (which rotates the whole dir) or a new `--session` dir. Never-truncate is
   preserved; the clobber-safety concerns of bak-N rotation disappear.
   `02-log-safety.md` rewritten accordingly; stage-2 error text updated to stop
   recommending the in-place fresh run.

2. **[both, should-fix] Checkpoint bytes looked up by label alias under
   duplicate labels / colliding sanitized filenames.** Resolution: build the
   checkpoint map deduped by `DumpMeta.file` (max-frame entry wins, warn about
   shadowed entries) and read bytes via the file path (`load_dump_bytes_for(&DumpMeta)`),
   not by label. `04-replay-verification.md` updated.

3. **[both, should-fix] Platform-captured dumps use `frame: 0` as an
   informational sentinel** (documented in `session.rs`) and must not become
   integrity constraints or replay checkpoints. Resolution: skip `frame == 0`
   dumps in both stage-2 and stage-3, with an eprintln note. The degenerate
   "genuine F5 dump at frame 0" is knowingly traded away.

4. **[both] Resume with dumps recorded but padlog missing** produced a
   misleading "likely truncated" error / contradicted stage 1's "create fresh".
   Resolution: detect the missing-file case explicitly and refuse with its own
   message; resume + missing log + **no dumps** still starts fresh (wrapper
   already prints "starting fresh" for this).

5. **[reviewer 2, must-fix] Deployment could be silently reverted** by any
   later `cargo build` in the stale `~/m6` source copy the wrapper points at.
   Resolution: sync `crates/`, `Cargo.toml`, `Cargo.lock` from the git tree
   into `~/m6/preestablished/reference-workload` (git tree is authoritative;
   the m6 copy is strictly older), then build the release binary **in the m6
   tree** so the deployed binary and its sources agree. Record the deployed
   git commit in the final report. `05-deploy-and-verify.md` updated.

6. **[reviewer 2, should-fix] Promote `log_frames` in `session.yaml` from
   future work.** Detects tail truncation past the last dump (the original
   symptom class stage 2 alone can't see). Resolution: add
   `log_frames: Option<u64>` (serde default) to `Session`, updated at every
   save in the interactive loop; stage-2 errors when the padlog holds fewer
   frames than the recorded count. Added to `03-resume-integrity.md`.

7. **[reviewer 2, should-fix] `Session::save` is a non-atomic truncate+write**
   (called on every F5 dump; a crash mid-save bricks the dir for `search` too).
   Resolution: write `session.yaml.tmp`, then rename. Added to stage 2 scope.

8. **[reviewer 2, should-fix] Stage-4c does not need a display.** Replay and
   checkpoint verification run before `Window::new`; headless runs validate
   everything and then fail loudly at window-open. Resolution: run the
   end-to-end resume checks headless unconditionally; drop the display-bound
   rotation check (now covered by refusal unit tests).
   Also **[reviewer 1, nit]**: pre-check the bak-2 fixture's own consistency
   (log length vs max dump frame) before using it, since a stage-2 refusal
   would otherwise mask the checkpoint test.

9. **[reviewer 2, nit] Live-loop fault currently exits 0.** Resolution: after
   saving the session, return `Err` so a faulted session is visible to
   scripts. One line, in scope.

10. **[reviewer 1, nit] `--skip-replay-verify` guard placement**: the
    non-interactive guards sit after the interactive early-return, so the
    "same pattern" would silently ignore the flag. Resolution: explicit
    `skip_replay_verify && !resume → error` inside the interactive branch,
    plus the usual non-interactive rejection after it.

## Accepted in principle, conditional

11. **[reviewer 2, should-fix] Concurrent runs on one session dir.** Real
    hazard (two appenders scramble one padlog). Resolution: take an exclusive
    advisory lock on the padlog (`File::try_lock`, std, Rust ≥ 1.89 — released
    automatically on process death, no stale-lockfile problem) and fail with
    "another ramdiff is running on this session". **Conditional on the
    installed toolchain being ≥ 1.89**; if older, document as future work
    rather than adding a dependency.

## Rejected / already moot

- Rotation clobber-safety via hard-link claim (reviewer 1 nit) — moot; rotation
  removed (see 1).
- BTreeMap duplicate-frame parenthetical — moot; map now dedupes by file (2).
- Whole-dir rotation inside ramdiff — rejected in favor of refusal; the wrapper
  already owns rotation UX, and duplicating it in-tool doubles the surface for
  exactly the class of bug being fixed.
