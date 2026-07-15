# Stage 4 — Build, deploy to the wrapper's tree, verify end-to-end

*(Revised per review — see 06-review-resolutions.md #5, #8.)*

## Why deployment is part of the fix

`~/.local/bin/record-ramdiff` hardcodes
`RAMDIFF="$HOME/m6/preestablished/reference-workload/target/release/ramdiff"`.
That tree is a **stale source copy, not a git checkout** — and any later
`cargo build` there would regenerate the truncating binary at the exact path
the wrapper runs. Dropping in a binary alone is not enough; the sources must
be brought forward too.

## Steps

1. **Tests** (git tree):
   ```
   cargo test -p ramdiff
   cargo test -p ramdiff --features interactive   # compile-checks the window path
   ```

2. **Commit** the fix in the git tree (authoritative source), so the deployed
   state has a citable commit hash.

3. **Sync sources into the deployed tree** (git tree is strictly newer; the m6
   copy predates the resume feature entirely):
   ```
   rsync -a --delete ~/git/preestablished/reference-workload/crates/ \
         ~/m6/preestablished/reference-workload/crates/
   cp ~/git/preestablished/reference-workload/Cargo.toml \
      ~/git/preestablished/reference-workload/Cargo.lock \
      ~/m6/preestablished/reference-workload/
   ```
   (Scope the rsync to `crates/` + manifests; leave `.beads`, `dist`, etc.
   alone. Keep the old binary aside first:
   `cp target/release/ramdiff target/release/ramdiff.pre-resume-fix` in the m6
   tree.)

4. **Build in the m6 tree** so the deployed binary and its sources agree:
   ```
   cd ~/m6/preestablished/reference-workload
   cargo build --release -p ramdiff --features interactive
   ```

5. **End-to-end verification — headless, real data, copies only** (replay,
   integrity, and checkpoint verification all run *before* `Window::new`, so
   no display is needed; a fully successful resume ends with a window-open
   error, which is the expected terminal state headless):

   a. **Truncated session is refused.** Copy `~/m6-private/ramdiff/discovery-01`
      to the scratchpad and run the deployed binary:
      ```
      ramdiff record --interactive --resume --rom <the ROM> --session <copy>
      ```
      Expected: exits nonzero **before replaying anything**, with the stage-2
      diagnosis naming `1-4 boss defeated` / frame 77146 vs 8605 log frames.
      The copy's padlog must be byte-identical afterwards.

   b. **Fresh run on a dirty dir is refused.** Same copy, without `--resume`:
      expected stage-1 refusal mentioning `--resume` and the wrapper; dir
      untouched.

   c. **Checkpoint verification fires.** Pre-check `discovery-01.bak-2`'s own
      consistency first (its max `session.yaml` dump frame must be `<` its
      25,229 log frames — otherwise pick another intact session; a stage-2
      refusal here would mask the checkpoint test). Copy it to the scratchpad
      and resume against it. Acceptable outcomes, all informative:
      - checkpoints pass → `replay checkpoint OK …` lines, then the expected
        headless window-open error (resume works end-to-end);
      - checkpoint divergence error naming a frame/label → the emulator has
        drifted since the session was recorded and resume now *detects* it.
      Report which outcome occurred.

6. **Report to the user**, explicitly including:
   - The boss-run padlog was destroyed by the old truncating build; that state
     cannot be resumed. The 17 WRAM dumps remain valid for
     `ramdiff search`/`candidates`.
   - `discovery-01` as it stands will be *refused* by `--resume` (by design,
     with an explanatory message). To continue working: run `record-ramdiff`
     fresh (the wrapper rotates the dir; dumps stay in the rotated copy).
   - The deployed commit hash, and that the `~/m6` tree's sources are now in
     sync with it.
   - Future sessions are protected: no code path truncates a non-empty log,
     truncated/missing logs are detected at resume, and replay is verified
     against dumps.

## Future work (record, don't implement)

- Full state snapshots (CPU/PPU/APU/VRAM/WRAM/cart) written at exit and on F5,
  making resume O(1) and immune to emulator drift. Requires emulator
  serialization support; belongs with the emu crate roadmap.
- Padlog file locking if the toolchain is < 1.89 (see stage 1).
