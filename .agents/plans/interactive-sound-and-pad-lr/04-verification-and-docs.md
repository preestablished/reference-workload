# Package 04 — Verification Gates And Docs Closeout

Depends on packages 01–03.

## Automated gates (run in this order, individually checked)

1. `cargo test --locked --workspace` — everything, default features.
2. `cargo test --locked -p refwork-emu --features audio`
3. `cargo test --locked -p ramdiff --features interactive` (lab Mac)
4. `cargo test --locked -p refwork-harness --test mock_agent -- mock_agent_happy_path_1000_frames`
   — the determinism happy-path fixture must be untouched by all of this.
5. xtask determinism test (`xtask/tests/determinism.rs`) — same reason.
6. `cargo clippy --locked --workspace --all-targets` clean for the changed
   surface; repeat with `--features interactive` for ramdiff and
   `--features audio` for refwork-emu.

Any red gate stops the line — fix before proceeding (review-workflow exit
gate; "it builds" is not "it works").

## On-hardware checklist (lab Mac, F310 in D mode, real session via `tools/record-ramdiff`)

The whole point of this plan is interactive UX; none of it counts as done
until observed live:

- [ ] Audio: example game's music/SFX audible from session start (after any
      resume replay, with no stale-audio burst).
- [ ] `M` mutes; window title shows `[muted]`; `M` again unmutes; the padlog
      for the session contains no artifact of the toggles (pure host-side).
- [ ] `--no-audio` runs silent with no audio stderr noise.
- [ ] Unplugged/absent audio device (or forced failure) degrades to silent
      with a single stderr note; session still records normally.
- [ ] `--pad-debug`: name/UUID/mapping-source printed; each of the 12
      buttons produces events; each maps to the right SNES bit.
- [ ] In-game: L and R perform their game function.
- [ ] Regression: keyboard-only session (no pad) still works; F5 dump still
      works; resume replay still verifies checkpoints.
- [ ] Perf sanity: window still holds ~60 fps with audio on; no periodic
      audio skip (watermark trims should be rare — roughly once per 90 s
      worst case with the 16,667 µs pacing, package 02 — or absent), no
      audible crackle, no input lag.
- [ ] Pad-mapping completeness: with `--pad-debug`, confirm each of the 12
      buttons lands on its correct SNES bit — not merely that "L/R respond
      to something" (the mode-2 default map can misroute Back/Start; see
      package 03 problem statement).

## Docs to update (small, in-repo, no commercial ROM naming)

- `crates/ramdiff/src/main.rs` doc header (`:6-54`): `--no-audio`,
  `--pad-debug`, `M` in the keyboard table, audio note.
- `crates/ramdiff/src/record.rs` module doc (`:6-35`): same additions; new
  window-title string.
- `crates/ramdiff/src/gamepad.rs` / `gamepad_macos.rs` module docs: folded
  trigger mapping (and its SDL-mapping-source gate), pad-debug,
  `SDL_GAMECONTROLLERCONFIG` workflow.
- `tools/record-ramdiff`: gamepad preflight text mentions pad-debug; export
  `SDL_GAMECONTROLLERCONFIG` if acceptance 03.3 required one; mention `M`
  mute in the session banner if it prints controls.
- `README.md`: only if it documents interactive mode today (it does not —
  skip unless drift is found).

## Tracking and close

- Beads: implementation beads for packages 01–03 plus this one; close each
  with `bd close <id> -r "<gate evidence>"` as gates pass.
- Session close per CLAUDE.md: commit per logical package, `git pull
  --rebase`, `bd dolt push`, `git push`, verify up-to-date with origin.
- If the on-hardware checklist cannot run in the implementing session (no
  human at the lab Mac), the code lands with automated gates green, the
  hardware items become a P1 bead assigned to the operator, and the plan is
  NOT closed — the bead records exactly which checklist lines remain.
