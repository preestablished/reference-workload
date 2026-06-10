# 06 — First-room bring-up against the operator ROM; verified feature map

**Depends on:** 02 (full APU), 03 (PPU raster features), 04 (`ramdiff`),
05 (`refwork-verify`). **The 3-week build-vs-vendor gate clock starts when
this package starts** — record the start date in the gate log (package 08)
on day one.

This is the unpredictable part ("accuracy debugging", 2–4 weeks in the
ARCHITECTURE §2 estimate) and the phase's schedule risk. Everything here
runs on **lab runners** with the operator-supplied game image; nothing
game-derived (ROM, dumps, goldens, the real input script's semantics) is
committed to the repo. The repo gets: code fixes, the verified
`feature-maps/demo-game.yaml` offsets (plain numbers — allowed; they are
operator-validated artifacts per the clean-room follow-up note), and
`map-check` expectation files.

## The bring-up loop

Iterate, keeping a dated log (`.agents/plans/phase-2/bringup-log.md`, no
game names — "the demo game"):

1. `refwork-verify play --rom <operator>.rom --script boot.padlog` — start
   with an empty/held-input script. Every `Fault{...}` is the next work
   item: `UnimplementedBgMode` / `UnimplementedPpuFeature` → implement in
   package 03's on-demand lane; unimplemented bus address / opcode edge →
   fix in the relevant core module. **Never weaken a fault to a silent
   fallback to make the game boot (D9).**
2. When a segment renders, extend the input script via
   `ramdiff record --interactive` (play forward, save the recorded
   `.padlog`), then re-verify the extended script replays deterministically
   via `refwork-verify double-run`.
3. Repeat until the script goes power-on → title → file/intro sequence →
   first room → **first room transition**.
4. For visual wrongness without faults (wrong colors, broken layers,
   garbled sprites): bisect with `--snap` framebuffer dumps and the per-line
   PPU unit-test style from 03 — turn each diagnosed bug into a committed
   regression test against the synthetic ROM where expressible.

Expected fault hot-list going in (from the M1 fault inventory): HDMA enable,
color-math enables, window enables, a BG mode for title/intro screens,
possibly SETINI bits. 03 should land its build-order items 1–5 *before* this
package starts to shorten the loop.

## Feature-map verification (parallel with later bring-up iterations)

Once the first room is reachable interactively:

1. Transcribe community RAM-map candidates for the operator's ROM revision
   into a scratch map (lab-side notes, not the repo — the repo map gains
   only verified entries).
2. `ramdiff record --interactive` labeled sessions per ARCHITECTURE §5
   ("standing-room-1", "health-after-damage", …); `ramdiff search` narrowing;
   `ramdiff watch` semantic confirmation for **every** entry — wrong-revision
   community maps are a named risk; no unverified address ships.
3. Decide `stability` (does it flicker during transitions/cutscenes? →
   `volatile`) and `discretize` per entry; `ramdiff emit` into
   `feature-maps/demo-game.yaml`, replacing the placeholder offsets; delete
   the "PLACEHOLDER FILE" preamble; keep `game_revision` accurate.
4. Author `map-check` expectations for the first-room script: at minimum
   `room_id changes_to <expected>` at the door crossing, `game_state` in
   gameplay mode during play, `health` delta on a scripted damage event if
   the route has one.

## Goldens and the acceptance run

- Pick scripted checkpoint frames (post-title, room entry, post-transition);
  `refwork-verify play --snap` produces framebuffer dumps; the operator
  approves them once; they live in the lab golden store. The lab runner's
  acceptance job re-runs the script and byte-compares.
- Final M2 evidence run (lab, both arches — see 07 for the aarch64 runner):
  - `refwork-verify play --script first-room.padlog --report` →
    `room_id` transition asserted, goldens match.
  - `refwork-verify map-check` green against the verified map.
  - `refwork-verify double-run --frames 100000` green on x86_64 **and**
    aarch64, chained hashes **equal across the two architectures** (the
    cross-arch compare catches latent float/UB issues — the named reason
    this gate exists).

## Acceptance (= M2 acceptance items 1–3)

- First-room transition achieved by the committed-format script (script
  itself stored lab-side; its `.padlog` hash recorded in the bring-up log).
- `feature-maps/demo-game.yaml` carries only ramdiff-verified offsets and
  passes `refwork-featuremap validate` + `refwork-verify map-check`.
- 100k-frame cross-arch double-run green (both the synthetic ROM — in CI —
  and the demo game — lab evidence run).
- Bring-up log closed out with the gate decision input for package 08
  (dates, what worked, accuracy-bug catalog).
