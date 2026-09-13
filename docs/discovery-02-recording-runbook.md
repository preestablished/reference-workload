# discovery-02 recording runbook (operator)

One page to follow at the keyboard to produce the three discovery-02
hand-play sessions. Plan:
`~/.agents/projects/reference-workload/plans/discovery-02-recording-readiness/`
(package 03). The label vocabulary here and the machine copy
`tools/discovery-02-required-labels.yaml` must agree name for name; the
lint (`tools/lint-session`) enforces the machine copy.

`$PR` is the private root from
`~/.agents/projects/reference-workload/private-root.path`. Its value, the
ROM identity, and any WRAM offset or value never appear in a commit or on a
recorded terminal.

Stage names: `W1-S1`..`W1-S4`, `W2-S1`; "world 2" = the W2 hub plus `W2-S1`.
The user's "first boss" is the **W1-S4 boss** (labels `w1s4-boss-*`); the
W1-S2 midboss keeps its own labels (`w1s2-midboss-*`).

## 0. Pre-flight (agent + operator, 15 minutes)

0.1 Build and install:
```sh
cd "$(git rev-parse --show-toplevel)"
git status --short            # must be empty
cargo build --release --locked --features interactive -p ramdiff
cargo build --release --locked -p refwork-verify
ls -l ~/.local/bin/record-ramdiff    # symlink into a checkout's tools/; that checkout must be at the epoch commit and rebuilt
```
The installed `record-ramdiff` symlink resolves the `ramdiff` binary from
the checkout it lives in. If that is a different clone than the one you
built, pull it to the same commit and rebuild there, or run
`tools/record-ramdiff` from this checkout directly.

0.2 Epoch: `grep 'pub const EMU_VERSION' crates/refwork-emu/src/lib.rs`
must print the version recorded in `$PR/evidence/discovery-02-epoch.txt`
(plan package 01 §1). If it differs, stop and redo gate G0.

0.3 Gamepad: F310 back switch on **D**; plug in before launch (macOS
gilrs does not pick up pads connected mid-session). If L/R or any button
land wrong, the wrapper cannot pass `--pad-debug` (it accepts only
`--resume`/`--stats`); run the binary directly into a scratch session:
```sh
PR="$(head -n1 ~/.agents/projects/reference-workload/private-root.path)"
target/release/ramdiff record --interactive --pad-debug --no-audio \
  --rom "$(find "$PR/ROMs/SNES" -type f | head -n1)" --session "$PR/ramdiff/paddebug-01"
```
read the pad UUID and mapping source from stderr, export an exact-UUID
`SDL_GAMECONTROLLERCONFIG` line (the env var passes through the wrapper's
`exec`), then launch `record-ramdiff` normally.

0.4 Warm-up run (repeats the `refwork-ta9` check from plan package 01 step 3, now with the lint as the tool-chain proof):
```sh
record-ramdiff warmup-01 --stats     # play ~2 minutes on the title/first stage, press F5 once, label: warmup, then Esc
tools/lint-session warmup-01 --kind discovery-02-main --final   # expected: FAIL only on C8 (missing labels) — proves the tool chain; C11 must PASS
tools/replay-fidelity warmup-01      # dry run of gate G2 on the one dump: expect `fidelity: PASS (warmup-01)`
```
Read the `--stats` lines: ~60.1 fps and `reprimes 0` after the first
window. Save stderr to `$PR/evidence/warmup-01.stderr` and close
`refwork-ta9` per plan package 01 §1.3 (or reopen it with the stderr if
audio gaps or fps below 59.5 return — then do not record).

0.5 Terminal layout: the F5 label prompt appears on the terminal that
launched `record-ramdiff`, not in the game window. Keep that terminal
visible; a second terminal runs `tools/lint-session <name>` at any time.

0.6 Names are immutable: never run `record-ramdiff` without a session
name (the default `discovery-01` would rotate the current `discovery-01`
dir), and never reuse a `discovery-02-*` name. A retake is
`discovery-02-main-take2`, and so on (the lint resolves `-takeN` to the
base kind). Budget: up to three takes of the main session; if the third
fails, stop and record why in `$PR/evidence/discovery-02-takes.txt` before
trying again.

0.7 Practice first (untracked): play W1-S1 through the W1-S4 boss at least
once under `record-ramdiff practice-01` (no F5 labels, Esc at the end) to
re-learn the midboss and boss patterns; these are the segments where a
game over would cost the whole take. Practice sessions are never linted or
replayed.

## 1. Label discipline (read once)

- Never press F5 on the very first frame after launch or after a resume
  finishes: wait until the game has visibly advanced (a second is plenty).
  A frame-0 dump is treated as a platform capture by every tool and the
  lint fails it (check C13).
- Press **F5**, type the label exactly as listed (lowercase, digits, `-`),
  press Enter. The game is paused while the prompt waits; audio may
  re-prime after — expected. When playing with the gamepad the terminal has
  keyboard focus, so the F5 keypress also reaches the prompt as an escape
  sequence; the prompt strips a leaked leading function-key sequence
  automatically, so type the label normally.
- The dump is taken at the frame shown; "entry" dumps are taken on the
  **first frame the player can move** in the stage.
- Bracket pairs (`score-before-N` / `score-after-N`): dump `before`, cause
  exactly one score-increasing event, dump `after` as soon as the counter
  has changed on screen. Use three different event kinds if the game has
  them (e.g. enemy, pickup, block) so increments differ.
- If you mistype a label, dump again with the correct label; add a line to
  `SESSION-NOTES.md` naming the stray label. If the correct label was
  already used once, use `-2`. A label typed with a space or any other
  character becomes a different file name than the label; the lint (check
  C6) fails such a session, so re-dump under the exact name.
- Mid-session check from the second terminal: `tools/lint-session <name>`;
  it lists the required labels not yet taken.

## 2. Session `discovery-02-main` (no game over allowed)

Launch: `record-ramdiff discovery-02-main --stats`. Create
`$PR/ramdiff/discovery-02-main/SESSION-NOTES.md` with the G0 confirmation
sentence (the `freeze lifted` line from `$PR/evidence/discovery-02-epoch.txt`),
the date, and the EMU_VERSION string before playing.

| # | Where | Label | Note |
|---|-------|-------|------|
| 1 | Title screen showing "press start" | `title-press-start` | |
| 2 | Title menu with 1P selected | `title-1p-selected` | |
| 3 | World-1 hub / map, first frame movable | `w1-hub` | |
| 4 | W1-S1 first movable frame | `w1s1-entry` | full health here |
| 5 | Standing still, undamaged, ~2 s later | `health-full` | |
| 6 | Right after the first hit | `health-hit-1` | take a second hit later → `health-hit-2` (optional) |
| 7 | Before/after a score event | `score-before-1`, `score-after-1` | see §1 |
| 8 | Second score event, different kind | `score-before-2`, `score-after-2` | |
| 9 | Third (optional) | `score-before-3`, `score-after-3` | |
| 10 | If a heal pickup exists | `heal-1` | optional |
| 11 | W1-S2 first movable frame | `w1s2-entry` | |
| 12 | Midboss fight starts (first frame it can be hit) | `w1s2-midboss-begin` | |
| 13 | Midboss defeated (its death animation starts) | `w1s2-midboss-defeated` | |
| 14 | W1-S3 first movable frame | `w1s3-entry` | |
| 15 | W1-S4 first movable frame | `w1s4-entry` | |
| 16 | Boss fight starts | `w1s4-boss-begin` | |
| 17 | Boss defeated | `w1s4-boss-defeated` | the user's "first boss" |
| 18 | Stage-clear / results screen | `w1-clear` | |
| 19 | World-2 hub, first movable frame | `w2-hub` | |
| 20 | W2-S1 first movable frame | `w2s1-entry` | |
| 21 | ~30 s of ordinary W2-S1 play later | `w2s1-play` | |
| 22 | Stand still 2 s, dump | `session-end` | then Esc |

Rules: do not lose all lives. If health reaches zero and the game respawns
you (a life lost), continue — that is fine; note the approximate time in
`SESSION-NOTES.md`. If a game over happens, press Esc; the failed take
stays on disk. Then either start `discovery-02-main-take2` from power-on,
or rewind (§2a) to keep the work done so far.

### 2a. Rewind-by-truncation (recover a failed take without replaying by hand)

Replay is deterministic, so a padlog cut at frame T followed by `--resume`
restores exactly the state at T. Choose T **before** the segment you want
undone (before the death that led to the game over, or before a mislabeled
sequence) and **after** the last dump you want to keep. Lives lost before
T stay lost. Steps (agent may run them; nothing private is printed):

```sh
PR="$(head -n1 ~/.agents/projects/reference-workload/private-root.path)"
OLD="$PR/ramdiff/discovery-02-main"; NEW="$PR/ramdiff/discovery-02-main-take2"; T=<frame>
python3 -c 'import yaml' || { echo "pyyaml missing: run with PATH=\"$HOME/.venvs/refwork/bin:$PATH\" (or install pyyaml per WORKING-NOTES pkg 01)"; exit 1; }
[ -e "$NEW" ] && { echo "take dir exists"; exit 1; }
WORK="$NEW.building"; rm -rf "$WORK"; mkdir -p "$WORK"     # build in a scratch dir; rename atomically at the end
head -n $((T + 1)) "$OLD/interactive.padlog" > "$WORK/interactive.padlog"   # header + frames 0..T-1
cp -f "$OLD/session.yaml" "$WORK/session.yaml"
# keep only dumps with frame < T, copy their .bin files, set log_frames: T
python3 - "$OLD" "$WORK" "$T" <<'EOF'
import sys, shutil, yaml
old, new, t = sys.argv[1], sys.argv[2], int(sys.argv[3])
s = yaml.safe_load(open(f"{new}/session.yaml"))
keep = [d for d in s["dumps"] if 0 < d["frame"] < t]
for d in keep: shutil.copy(f"{old}/{d['file']}", f"{new}/{d['file']}")
s["dumps"] = keep; s["log_frames"] = t; s["candidates"] = {"width": "u8", "offsets": []}
yaml.safe_dump(s, open(f"{new}/session.yaml", "w"), sort_keys=False)
print(f"kept {len(keep)} dumps, log_frames={t}")
EOF
mv "$WORK" "$NEW"                                   # only now does the take name exist; a failure above leaves only *.building to delete
tools/lint-session discovery-02-main-take2          # mid-session mode: expect INFO on missing labels, no FAIL
record-ramdiff discovery-02-main-take2 --resume --stats   # replays T frames, verifies kept dumps, then continues live
```

`pyyaml` lives in `~/.venvs/refwork` (activate it or prefix `PATH`). The
`emu_version` / `rom_blake3` stamps are copied with `session.yaml`, so the
resume check runs on the take as on the original. Record `T`, the source
take, and the reason in the new take's `SESSION-NOTES.md`. A take made
this way is a complete session from power-on for every downstream purpose
(its padlog is the full input log).

Score counter fallback: if no score is displayed, bracket the nearest
monotone counter (coins, gems, points) and write which one in
`SESSION-NOTES.md` under `score_counter:`. If none exists, write
`score_counter: NOT-APPLICABLE` and skip labels 7-9; then run the final
lint as `tools/lint-session discovery-02-main --final --waive 'score-*'`,
which must end `lint: PASS (WAIVED: score-before-1, …)`. Any other missing
label still fails. The processing plan reads `SESSION-NOTES.md` first.

After Esc: `tools/lint-session discovery-02-main --final` → must end
`lint: PASS`, or `lint: PASS (WAIVED: …)` when `--waive 'score-*'` was
used with the recorded `score_counter: NOT-APPLICABLE` note. Then §6 and
plan package 04 (`tools/replay-fidelity discovery-02-main`).

## 3. Session `discovery-02-gameover-death`

Launch: `record-ramdiff discovery-02-gameover-death --stats`.

| # | Where | Label |
|---|-------|-------|
| 1 | Title "press start" | `title-press-start` |
| 2 | W1-S1 first movable frame | `w1s1-entry` |
| 3 | Standing still, undamaged | `alive-baseline` |
| 4 | Same spot, 1 s later (lives counter untouched) | `lives-full` |
| 5 | First frame of the death animation of life 1 | `death-1` |
| 6 | First movable frame after respawn | `respawn-1` |
| 7 | Optional second cycle | `death-2`, `respawn-2` |
| 8 | Standing on the last life, before the final death | `last-life` |
| 9 | First frame of the final death animation | `death-final` |
| 10 | Game-over screen, first frame | `gameover-screen` |
| 11 | Game-over screen ≥2 s later (still showing) | `gameover-late` |
| 12 | Whatever follows (continue prompt or title) | `post-gameover` |

Then Esc, `tools/lint-session discovery-02-gameover-death --final`.
Deaths should be by enemy contact or fall, both are fine; note which in
`SESSION-NOTES.md` (`death_causes:`). This session has no minimum length;
only the labels matter.

## 4. Session `discovery-02-gameover-timer`

First decide: does the demo game have a level timer that ends the level
or the game when it expires? Check the HUD in W1-S1 for a countdown.

- **No timer:** `mkdir -p "$PR/ramdiff/discovery-02-gameover-timer" && printf 'NOT-APPLICABLE: the demo game shows no level timer (checked %s).\n' "$(date -u +%F)" > "$PR/ramdiff/discovery-02-gameover-timer/NOT-APPLICABLE.md"`; run `tools/lint-session discovery-02-gameover-timer --final` → `lint: PASS (NOT-APPLICABLE)`; `tools/replay-fidelity discovery-02-gameover-timer` → `fidelity: NOT-APPLICABLE`. Done.
- **Timer exists:** `record-ramdiff discovery-02-gameover-timer --stats`:

| # | Where | Label |
|---|-------|-------|
| 1 | Title "press start" | `title-press-start` |
| 2 | W1-S1 first movable frame | `w1s1-entry` |
| 3 | Standing still, timer near its start value | `timer-start` |
| 4 | Roughly half way | `timer-mid` |
| 5 | Last ~10 s | `timer-low` |
| 6 | First frame after it reaches zero | `timer-expired` |
| 7 | Resulting game-over (or life-lost) screen, first frame | `gameover-timer-screen` |
| 8 | Same screen ≥2 s later | `gameover-timer-late` |

If expiry costs a life instead of the game, keep the same labels and write
`timer_expiry: life-lost` in `SESSION-NOTES.md`; the processing plan then
treats it as a death variant.

## 4a. Session `discovery-02-score-currency` (score vs currency isolation)

Purpose: `discovery-02-main` bracketed score events that also moved currency
and other counters at once, so the score offset can't be told from the
currency offset. This short session brackets **isolated** events. Plan:
`~/.agents/projects/reference-workload/plans/discovery-02-score-currency-session/`.
It stays in W1, contains no game over, and ends with Esc.

Launch: `record-ramdiff discovery-02-score-currency --stats`. If you already
ran the §0 warm-up in this sitting you need not repeat it. There is no boss
fight, so the take budget is relaxed; a crash or mislabel still means a retake
(`-take2`) or a rewind.

For this game (operator-confirmed): score and coins are separate counters,
coins give no score, and scoring gives no coins — so every bracket below is
recordable and **no waiver is needed**. Currency can only be spent in later
levels, so skip the `currency-spend` bracket here (note that in
`SESSION-NOTES.md`) and just show coins increasing. The score is 4 digits at
start and can reach 6 (past 65535), so it is a multi-byte counter: make
`score-only-2` a large gain and **write the exact displayed score before and
after** so the processing plan can find its high bytes; the top byte
(above 9999) likely will not move in W1, which is expected.

**For every bracket, write in `SESSION-NOTES.md` the on-screen before/after
value of each visible counter and the bracket's frame span** (the F5 dump
frames are shown on the launching terminal). This is what lets the processing
plan stitch a multi-byte counter and subtract incidental movers.

| # | Where | Label |
|---|-------|-------|
| 1 | Title "press start" | `title-press-start` |
| 2 | W1-S1 first movable frame | `w1s1-entry` |
| 3 | Standing still, undamaged, nothing collected | `baseline` |
| 4 | Stand/walk a few seconds gaining nothing; dump before and after | `idle-before-1`, `idle-after-1` |
| 5 | A **longer** stand/walk gaining nothing (match your longest event bracket below) | `idle-before-2`, `idle-after-2` |
| 6 | Before/after one event that adds **score only, no currency** | `score-only-before-1`, `score-only-after-1` |
| 7 | A **different** score-only event, **big enough to roll the score past a 100s/1000s digit** | `score-only-before-2`, `score-only-after-2` |
| 8 | Before/after one event that adds **currency only, no score** | `currency-only-before-1`, `currency-only-after-1` |
| 9 | A **different** currency pickup, at least one big enough to roll a digit | `currency-only-before-2`, `currency-only-after-2` |
| 10 | Before/after one event that changes **both** score and currency | `both-before-1`, `both-after-1` |
| 11 | Stand still 2 s, dump | `session-end` | then Esc |

Optional: `currency-spend-before-1`/`currency-spend-after-1` around **spending**
currency (shop/ammo) if the game allows it; a third of either kind
(`score-only-before-3`/`score-only-after-3`,
`currency-only-before-3`/`currency-only-after-3`).

Why the "big enough to roll a digit" rule: the processing plan keeps an offset
only if it rose in **every** score-only bracket. A multi-byte counter's high
byte moves only on a carry, so at least one score-only and one currency-only
event must be large enough to carry into the next digit, and you must record
the displayed values so the high byte isn't lost.

Fallbacks — pick the one that matches this game and record it in
`SESSION-NOTES.md`, then lint with the matching waiver:

| Situation | note line | final lint |
|-----------|-----------|------------|
| No currency counter at all | `currency: NOT-APPLICABLE` | `tools/lint-session discovery-02-score-currency --final --waive 'currency-only-*'` |
| Currency exists but every currency pickup also gives score | `currency_only: NOT-AVAILABLE` | same `--waive 'currency-only-*'` |
| Every scoring event also gives currency | `score_only: NOT-AVAILABLE` | `--waive 'score-only-*'` |
| No event changes score and coins together | `both_event: NOT-AVAILABLE` | `--waive 'both-*'` |

The bracket **order does not matter** (the lint kind is order-independent), so
record them in whatever order the play makes easy. If the game has **no** event
that changes score and coins together, that is fine and expected — record
`both_event: NOT-AVAILABLE` in `SESSION-NOTES.md` and lint with
`--waive 'both-*'`; otherwise the `both-before-1`/`both-after-1` bracket carries
the substitute data for a fallback. After Esc:
`tools/lint-session discovery-02-score-currency --final` → `lint: PASS` (or the
matching `PASS (WAIVED: …)`), then `tools/replay-fidelity
discovery-02-score-currency` → `fidelity: PASS`.

## 5. Crash, stall, or mistake

- Window closed or process died: relaunch with
  `record-ramdiff <same-name> --resume`. The tool checks the emulator
  version and ROM stamp before replaying, then replays every recorded frame
  and verifies each dump. Wait for `interactive: resumed at frame N`.
- `cannot resume: session was recorded under ...`: the build changed. Do
  not use `--skip-replay-verify` for a discovery-02 session; rebuild the
  same commit (see `$PR/evidence/discovery-02-epoch.txt`) or start a new
  take.
- `cannot resume: session was recorded against a different ROM (rom_blake3
  mismatch)`: the file under `$PR/ROMs/SNES` is not the one the session was
  recorded with. Never use `--skip-replay-verify` here: the restored state
  would be meaningless. Check `$PR/ROMs/SNES` holds exactly the approved
  ROM (compare its BLAKE3 with `$PR/evidence/rom-identity.txt` privately),
  restore it, and retry `--resume`.
- `replay diverged from the recorded state ...`: stop; this is a
  determinism defect or a build mismatch — escalate with the message text
  (it has no private values) before recording anything else.
- Audio gaps or slow-frame notes returning: finish the current segment,
  Esc, save stderr to `$PR/evidence/<name>.stderr`, and reopen
  `refwork-ta9` with it.

## 6. Closing checklist (per session)

1. `tools/lint-session <name> --final [--waive 'score-*']` → `lint: PASS` (or `lint: PASS (NOT-APPLICABLE)` / `lint: PASS (WAIVED: …)`).
2. `SESSION-NOTES.md` has: date, EMU_VERSION string, `score_counter:`,
   `death_causes:` / `timer_expiry:` where relevant, stray labels, take
   history.
3. Plan package 04 fidelity evidence exists for the session
   (`tools/replay-fidelity <name>` → `fidelity: PASS (<name>)`).
4. `chmod -R go-rwx "$PR/ramdiff/<name>"`.

## Exclusions

No gamepad remapping work, no emulator tuning, no derived dumps, no
searching. If the game cannot be brought to W2 in one sitting, the take
rule applies; there are no save states in `ramdiff` (use §2a).
