# Implementation results (2026-07-15)

Implemented as planned (stages 1–4, with all review resolutions from
`06-review-resolutions.md`, including the conditional file lock — toolchain is
Rust 1.97, so `File::try_lock` was used).

## Code

- `crates/ramdiff/src/record.rs`: `ensure_fresh_session`, `check_resume_integrity`,
  `verify_checkpoint`, `count_pad_lines`, rewritten `open_interactive_log`
  (append-or-header-only + exclusive lock), checkpoint verification in the
  replay loop (deduped by dump file, `frame == 0` sentinel skipped),
  `log_frames` bookkeeping at every save, live-loop fault now returns `Err`.
- `crates/ramdiff/src/session.rs`: `log_frames: Option<u64>` (serde default),
  atomic `save()` via tmp+rename, `load_dump_bytes_for(&DumpMeta)`.
- `crates/ramdiff/src/main.rs`: `--skip-replay-verify` flag + guards + usage.

Tests: 32 pass (default), 41 pass with `--features interactive`. Commit
`1b3c263` on `feat/ppu-feature-coverage`, pushed.

## Deployment

- Old deployed binary kept at
  `~/m6/preestablished/reference-workload/target/release/ramdiff.pre-resume-fix`.
- `crates/`, `Cargo.toml`, `Cargo.lock` rsynced git → `~/m6` tree (verified
  identical), release binary rebuilt **in the m6 tree** so a future
  `cargo build` there reproduces the fix instead of reverting it.

## End-to-end verification (copies of real session dirs, headless)

a. **Truncated boss session refused.** `--resume` on a copy of `discovery-01`
   exits 1 before replaying:
   `interactive.padlog holds 8605 frames but dump "1-4 boss defeated" was
   recorded at frame 77146 …`. Padlog md5 unchanged.
b. **Fresh run on dirty dir refused.** Exit 1,
   `session dir already contains a recorded session (8605 logged pad lines,
   17 dumps)`, padlog md5 unchanged.
c. **Intact session resumes and verifies.** `--resume` on a copy of
   `discovery-01.bak-2` (25,228 frames, 11 checkpoints): **all 11 replay
   checkpoints OK** (byte-exact WRAM match at frames 1182 → 15983),
   `resumed at frame 25228`, then the expected headless
   `cannot open window` error. Notably the checkpoints pass even though the
   PPU/bus/timing code changed since that session was recorded — the WRAM
   trajectory is unaffected, so resume genuinely restores state end-to-end.

## User-facing outcome

- The boss-run input log was destroyed on Jul 14 (old truncating build); that
  state cannot be resumed by anything. Its 17 WRAM dumps remain fully valid
  for `ramdiff search`/`candidates`. A fresh playthrough is needed to regain a
  resumable trajectory; the fixed tool preserves it.
- `record-ramdiff --resume` now either restores a verified state or explains
  exactly why it can't.
