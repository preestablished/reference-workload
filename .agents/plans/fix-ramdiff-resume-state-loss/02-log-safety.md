# Stage 1 — Padlog safety: never destroy an existing recording

**File:** `crates/ramdiff/src/record.rs`
*(Revised per review — see 06-review-resolutions.md #1, #4, #7, #11.)*

## Current behavior

`open_interactive_log(path, append)` truncates whenever `append == false`
(computed as `opts.resume && opts.output_log.exists()`), so every non-resume
interactive run against an existing session dir silently destroys the log. The
wrapper normally rotates the whole dir first, but `ramdiff` must not rely on
its caller for data safety — the destroyed boss run proves why.

## New behavior

**Fresh interactive mode refuses a dirty session dir.** No in-place rotation
(review finding: rotating only the padlog strands stale `session.yaml` dumps
that would falsely trip the stage-2/3 checks and lets new dumps overwrite
`.bin` files the old session references).

In `run_interactive`, after `Session::load`, before anything is opened:

```rust
if !opts.resume {
    ensure_fresh_session(&session, &opts.output_log)?;
}
```

```rust
/// A fresh (non-resume) interactive run must not start on top of an existing
/// recorded session. Errs if the padlog contains any pad lines or the session
/// has recorded dumps.
fn ensure_fresh_session(session: &Session, log_path: &Path) -> Result<(), String>
```

- "Padlog contains pad lines" = file exists and has any non-empty line beyond
  the `padlog v1` header. A missing or header-only file is clean.
- Error message:

```
record: session dir already contains a recorded session
  ({n} logged frames, {m} dumps).
  - to continue it:            re-run with --resume
  - to start over safely:      use record-ramdiff (rotates the whole session
                               dir aside) or pass a new --session directory
Nothing was modified.
```

`open_interactive_log(path, resume)` then becomes simple and safe:
- `resume` and the file exists → open append (header already present).
- otherwise → create/truncate with header. Truncation is safe here only
  because `ensure_fresh_session` has proven the file is absent or header-only.

## Concurrency guard (conditional)

If the installed toolchain is Rust ≥ 1.89, take `file.try_lock()` (std
exclusive advisory lock, auto-released on process death) on the padlog right
after opening it; on failure return
`"another ramdiff appears to be running on this session"`. If the toolchain is
older, skip and note as future work — do not add a dependency for this.

## Call-site change in `run_interactive`

```rust
let mut log_file = open_interactive_log(&opts.output_log, opts.resume)?;
```

(The `append = opts.resume && opts.output_log.exists()` computation
disappears.)

## Tests (in `record.rs` `mod tests`, no `interactive` feature needed)

1. `ensure_fresh_session` errs when the log has pad lines (message mentions
   `--resume`); the file is untouched.
2. `ensure_fresh_session` errs when the log is clean but `session.dumps` is
   non-empty.
3. `ensure_fresh_session` passes on: missing file + no dumps; header-only file
   + no dumps.
4. `open_interactive_log(path, resume=true)` appends without rewriting the
   header (existing test — update to the new signature).
5. `open_interactive_log(path, resume=false)` on a missing file creates it
   with the header.
6. (If lock implemented) second open of a locked log errs.
