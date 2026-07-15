# Stage 2 — Resume integrity: refuse to "resume" a log that can't contain the session

**Files:** `crates/ramdiff/src/record.rs`, `crates/ramdiff/src/session.rs`
*(Revised per review — see 06-review-resolutions.md #3, #4, #6, #7, #9.)*

## Rationale

`session.yaml` records every dump's frame number, and (new) the padlog frame
count at last save. The live loop tags a dump with the current `frame` after
running it, so a dump at frame `F` implies the padlog held ≥ `F + 1` pad words
when the dump was taken. If the log at resume time is shorter than either
bound, it has been truncated or swapped — replaying it cannot reproduce the
session. Today this replays silently; it must be a hard, diagnostic error.

## `session.rs` changes

1. New field, backward compatible with existing YAML:

```rust
/// Number of pad frames in interactive.padlog at the last save, if this
/// session was recorded interactively. Detects tail truncation.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub log_frames: Option<u64>,
```

The interactive loop keeps it current: set `session.log_frames = Some(frame + 1)`
before the F5-dump save (at that point `frame+1` lines are logged) and
`Some(frame)` before the final save (counter has moved past the last line).
`run_record` (scripted mode) leaves the field untouched.

2. Make `save()` atomic: write to `session.yaml.tmp` in the session dir, then
`std::fs::rename` over `session.yaml` (it is called on every F5 dump; a crash
mid-write must not brick the dir).

## Integrity check (`record.rs`)

Pure function, unit-testable without a core or ROM:

```rust
/// Validate that a resume log can contain the recorded session.
/// `log_exists` distinguishes a missing padlog from an empty one.
fn check_resume_integrity(
    log_frames: usize,
    log_exists: bool,
    session_log_frames: Option<u64>,
    dumps: &[DumpMeta],
) -> Result<(), String>
```

Rules, in order:
1. Ignore dumps with `frame == 0` (documented sentinel for platform-captured
   dumps registered by hand; they are not interactive checkpoints).
2. If there is anything to resume *from* (dumps after filtering, or
   `session_log_frames > 0`) and `!log_exists` → error:
   `interactive.padlog is missing but session.yaml records N dumps / M frames…`
3. If `session_log_frames = Some(m)` and `log_frames < m` → error: the log
   held `m` frames at last save but now holds `log_frames` (tail truncated).
4. If any dump frame `F` (post-filter) has `F >= log_frames` → error naming
   the worst offender:

```
cannot resume: interactive.padlog holds 8605 frames but dump "1-4 boss defeated"
was recorded at frame 77146. The log no longer contains the recorded session
(it was likely truncated by an earlier run). Resuming would silently restart
from the wrong state.
  - The WRAM dumps and session.yaml are still valid for `ramdiff search`.
  - To start over in a fresh directory, use record-ramdiff (which rotates
    this session dir aside) or pass a new --session directory.
```

5. Otherwise `Ok(())`. Empty dumps + no recorded frame count → `Ok` regardless.

Call it in `run_interactive` right after `load_resume_log`, **before** any
frame is replayed and before the log is opened for append (a refused resume
leaves every file untouched):

```rust
let prior_log = load_resume_log(&opts.output_log, opts.resume)?;
if opts.resume {
    check_resume_integrity(
        prior_log.len(),
        opts.output_log.exists(),
        session.log_frames,
        &session.dumps,
    )?;
}
```

## Also in scope (review #9)

The live loop currently `break`s on a core fault and returns `Ok(())`. After
`session.save()`, return `Err` describing the fault so scripted callers see a
nonzero exit.

## Tests

1. `check_resume_integrity(8605, true, None, [dump @77146])` → `Err`
   containing the label and both frame numbers.
2. `(77147, true, None, [dump @77146])` → `Ok` (boundary `F + 1`).
3. `(77146, true, None, [dump @77146])` → `Err` (off-by-one guard).
4. Empty dumps, no recorded count → `Ok` for any length/existence.
5. Multiple dumps → error names the maximum-frame dump.
6. `frame == 0` dumps are ignored (alone → `Ok`; alongside a real dump the
   real dump governs).
7. Missing log (`log_exists = false`) with dumps → the "missing" error, not
   the "truncated" one.
8. `session_log_frames = Some(1000)`, log has 900 → `Err` (tail truncation)
   even with no dump past 900; log has 1000 → `Ok`.
9. Round-trip: `session.yaml` without `log_frames` still loads (serde
   default); with it, saves and reloads.
