# Stage 3 — Replay verification: prove the restored state is the recorded one

**Files:** `crates/ramdiff/src/record.rs`, `crates/ramdiff/src/main.rs`,
`crates/ramdiff/src/session.rs`
*(Revised per review — see 06-review-resolutions.md #2, #3, #10.)*

## Rationale

Resume-by-replay assumes the emulator reproduces the original trajectory
bit-for-bit. That breaks silently when the binary's emulation behavior changed
between recording and resuming (the PPU/bus/timing code is under active
change). The session dir already contains ground truth — WRAM dumps at known
frames — so replay can *verify itself* at every dump it passes.

## Implementation

In `run_interactive`'s replay loop (resume path only):

1. Before the loop, build the checkpoint map. Two review-driven rules:
   - **Skip `frame == 0` dumps** (platform-captured sentinel; not replay
     checkpoints).
   - **Dedupe by `DumpMeta.file`, keeping the max-frame entry per file.**
     Labels are not unique and distinct labels can sanitize to the same
     filename; the `.bin` on disk always holds the *latest* dump written to
     that path, so only the max-frame entry can match. `eprintln!` a note for
     each shadowed entry (its frame cannot be verified).

```rust
// file -> (frame, &DumpMeta), max frame wins; then invert to frame -> meta
let mut by_file: BTreeMap<&str, &DumpMeta> = BTreeMap::new();
for d in session.dumps.iter().filter(|d| d.frame > 0) {
    match by_file.entry(d.file.as_str()) {
        // keep the entry with the larger frame; eprintln the shadowed one
        ...
    }
}
let checkpoints: BTreeMap<u64, &DumpMeta> = by_file.values().map(|d| (d.frame, *d)).collect();
```

2. Inside the loop, after `core.run_one_frame(pad)` for index `i`
   (`i as u64` is the frame number, matching live-loop semantics — dump at
   frame `F` is taken after executing index `F`). Bytes are read **by file
   path**, not label: add `Session::load_dump_bytes_for(&self, meta: &DumpMeta)`
   (same length validation as `load_dump_bytes`, reads `dir.join(&meta.file)`):

```rust
if let Some(dump) = checkpoints.get(&(index as u64)) {
    let expected = session.load_dump_bytes_for(dump)?;
    if expected.as_slice() != core.wram().as_slice() {
        // count differing bytes for the message; first divergent offset helps debugging
        return Err(format!(
            "cannot resume: replay diverged from recorded state at frame {} \
             (dump {:?}): {} of {} WRAM bytes differ (first at 0x{:05x}). \
             The emulator's behavior has changed since this session was \
             recorded; the restored state would not match what you played. \
             Re-record the session with the current build, or pass \
             --skip-replay-verify to resume anyway (state may be wrong).",
            index, dump.label, diff_count, WRAM_SIZE, first_diff
        ));
    }
    eprintln!("interactive: replay checkpoint OK at frame {} ({:?})", index, dump.label);
}
```

3. Failure timing: divergence aborts **before** the log is opened for append,
   so a failed resume leaves the session untouched (same property as Stage 2).

4. Escape hatch: new flag `--skip-replay-verify` (interactive+resume only),
   plumbed `main.rs → InteractiveOpts { skip_replay_verify: bool }`. When set,
   log each mismatch as a warning instead of erroring. Guard placement
   (review): the existing non-interactive guards sit *after* the
   `if interactive { return … }` early-return, so add an explicit
   `skip_replay_verify && !resume → Err` **inside** the interactive branch
   before constructing `InteractiveOpts`, plus the usual rejection when
   `--interactive` is absent. Add to the `usage()` text and the doc comment
   table in `main.rs`/`record.rs`.

5. Progress: the existing "replayed N frames" tick every 10,000 frames is fine;
   checkpoint lines already give richer progress.

Note on comparison cost: `load_dump_bytes` validates length (existing API);
compare with plain slice `==` first, and only on inequality walk the buffers
once to produce `diff_count`/`first_diff` (no cost on the happy path).

## Tests

Unit-testing the full replay needs a ROM, which tests don't have. Factor the
comparison + message into a pure helper and test that:

```rust
fn verify_checkpoint(frame: u64, label: &str, expected: &[u8], actual: &[u8]) -> Result<(), String>
```

1. Identical buffers → `Ok`.
2. One-byte difference → `Err` containing frame, label, `1 of 131072`, and the
   correct first-diff offset.
3. Length mismatch → `Err` (defensive; `load_dump_bytes` should already guard).

End-to-end verification of the real replay path happens in Stage 4 against the
actual session dirs.
