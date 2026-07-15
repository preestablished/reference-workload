# Fix: `record-ramdiff --resume` restarts the game from the beginning

## Symptom

The user recorded an interactive session (session `discovery-01`) in which they beat
the first boss, taking 17 labeled WRAM dumps along the way (last dump:
`1-4 boss defeated` at frame **77,146**). Running `record-ramdiff --resume`
prints `interactive: replaying 8605 frames to restore session state`, then the
window opens **at the beginning of the game** (title screen), not at the
post-boss state.

## Root cause (confirmed by evidence, see 01-evidence.md)

Resume-by-replay itself works as designed. The problem is that **the padlog it
replays no longer contains the recorded run**:

1. **Data destruction (primary).** An earlier build of `ramdiff` opened
   `interactive.padlog` with `create+write+truncate` unconditionally — including
   when invoked with `--resume` (or when run fresh in an existing session dir).
   A resume attempt after the boss run therefore **truncated the ~77k-frame log
   in place** and re-recorded ~2 minutes of a fresh boot (all-zero pad words
   while the user sat at the title screen wondering why resume "didn't work").
   The boss-run input log is unrecoverable.

2. **Silent inconsistency (secondary).** The current resume path replays
   whatever is in `interactive.padlog` without any cross-check against
   `session.yaml`, which records that dumps were taken at frames far beyond the
   log's length (8,605 lines vs dump at frame 77,146). It cheerfully reports
   "replaying 8605 frames" and "resumed at frame 8605" — a state that is
   provably not the end of the recorded session. Nothing detects or reports the
   corruption, which is exactly the confusing behavior the user hit.

3. **No divergence detection (latent).** Even with an intact log, replay-based
   resume silently produces a wrong state if the emulator binary changed
   behavior between recording and replay (the deployed tree at
   `~/m6/preestablished/reference-workload` and the git tree differ across
   `bus.rs`, `core_impl.rs`, `ppu/*`, `timing.rs`). The session dir already
   contains ground truth — WRAM dumps at known frames — but replay never checks
   against them.

4. **Stale deployment (operational).** The wrapper
   `~/.local/bin/record-ramdiff` runs
   `~/m6/preestablished/reference-workload/target/release/ramdiff`, a binary
   built from a stale source copy (not a git checkout). Fixes landed in the git
   tree do not reach the user until that binary is rebuilt/replaced.

## Fix strategy

| Stage | File | What |
|-------|------|------|
| 1 | `02-log-safety.md` | Never truncate an existing non-empty padlog; fresh interactive runs refuse a session dir that already holds a recording (the wrapper owns rotation). |
| 2 | `03-resume-integrity.md` | Record the padlog frame count in `session.yaml`; before replaying, cross-check the padlog against recorded dump frames and the frame count; refuse resume with a precise diagnosis. |
| 3 | `04-replay-verification.md` | During replay, compare emulated WRAM against each stored dump at its recorded frame; report divergence loudly (with an escape hatch flag). |
| 4 | `05-deploy-and-verify.md` | Unit tests, sync sources into the deployed `~/m6` tree, rebuild there with `--features interactive`, and run headless end-to-end verification against copies of the real session dirs. |

Reviewed by two independent agents; dispositions in `06-review-resolutions.md`.

## Non-goals / explicitly out of scope

- **Full state snapshots** (serialize CPU/PPU/APU/VRAM/WRAM at exit, load on
  resume). This is the long-term robust design — it makes resume instant and
  immune to log loss — but it requires emulator-wide serialization support.
  Record it as future work; do not attempt in this pass.
- **Recovering the boss run.** The input log was overwritten in place; the
  `.bak-*` dirs hold either other sessions or already-zeroed logs. The 17 WRAM
  dumps in `discovery-01` remain fully usable for `ramdiff search`/`candidates`
  (that is their purpose), but continuing *play* from the post-boss state is
  impossible. A fresh playthrough will be needed, which the fixed tool will
  preserve correctly.
- **Wrapper rewrite.** `~/.local/bin/record-ramdiff`'s rotate-by-default logic
  is sound; it stays as is.

## Key code locations (git tree, branch `feat/ppu-feature-coverage`)

- `crates/ramdiff/src/record.rs` — `run_interactive`, `load_resume_log`,
  `open_interactive_log` (all changes land here + `main.rs` for flags).
- `crates/ramdiff/src/session.rs` — `Session { dumps: Vec<DumpMeta> }`,
  `DumpMeta { label, frame, file, region }`, `load_dump_bytes(label)`.
- Live-loop frame semantics (needed for the checks): the loop does
  `run_one_frame(pad)` → append log line → optional dump tagged with current
  `frame` → `frame += 1`. So a dump tagged frame `F` captures state **after**
  executing pad indices `0..=F`, and requires the log to hold at least `F + 1`
  frames.
