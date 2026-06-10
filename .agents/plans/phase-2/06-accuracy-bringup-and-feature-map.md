# 06 — First-room bring-up against the operator ROM; verified feature map

**Depends on:** 02 (full APU), 03 core lane (PPU raster features), 04
(`ramdiff`), 05 (`refwork-verify`). **The 3-week build-vs-vendor gate clock
starts when this package starts** — record the start date in the gate log
(package 08) on day one (and see 00/08: this clock-start reading needs
operator ratification, with a 4-week calendar backstop on 01–05).

## Preconditions — done *before* the clock starts, none on gate time

- [ ] **aarch64 lab box ("the Spark") provisioned**: Rust toolchain
  installed, this repo builds, ROM accessible — proven by a green
  synthetic-ROM cross-arch double-run against the Intel box (hash
  equality). QEMU is acceptable for CI (07) but **not** for the M2
  evidence run; the lab box must actually work.
- [ ] **Interactive environment designated and smoke-tested**: name the
  machine (and operator) where `ramdiff record --interactive` sessions
  happen — it needs a display, a keyboard, *and* the operator ROM. If
  that machine is not a lab box (e.g. a dev workstation), get explicit
  operator sign-off on ROM handling and write the rule in the bring-up
  log. Run the windowing smoke test (04 acceptance) **on that machine**
  before day one — discovering a broken windowing crate on gate time is
  self-inflicted.
- [ ] 03 core lane fully landed (its build-order items 1–7); modes 3/7
  pre-built if hands were free (03 item 8).
- [ ] `refwork-verify play --continue-past-faults` recon mode working (05).

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

1. `refwork-verify play --rom <operator>.rom --script boot.padlog
   --continue-past-faults` — start with an empty/held-input script, in
   recon mode, so the **first run yields the complete fault inventory**
   (D9 halts authoritative runs at the first fault; without recon mode
   every lab cycle discovers exactly one gap). Triage the inventory into
   03's on-demand lane (`UnimplementedBgMode` / `UnimplementedPpuFeature`)
   and core fixes (unimplemented bus address / opcode edge), then batch the
   implementations. Re-run *without* the flag to confirm the route is
   fault-clean before extending it. **Never weaken a fault to a silent
   fallback to make the game boot (D9)** — recon mode changes the host
   tool's stop-policy, never the core's faulting.
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
- Final M2 evidence run (lab, both arches — the Spark provisioned per the
  preconditions above; 07's CI runners are not a substitute):
  - `refwork-verify play --script first-room.padlog --report` →
    `room_id` transition asserted, goldens match.
  - `refwork-verify map-check` green against the verified map.
  - `refwork-verify double-run --frames 100000` green on x86_64 **and**
    aarch64, chained hashes **equal across the two architectures** (the
    cross-arch compare catches latent float/UB issues — the named reason
    this gate exists).
- **Provenance block** recorded in the bring-up log — hashes are not game
  content and are admissible under the same rule as the verified offsets;
  without them the evidence run is a trust statement, not a checkable
  claim. Record: repo git rev + rustc version of the evidence build, cart
  BLAKE3, `first-room.padlog` BLAKE3, golden-framebuffer BLAKE3 list,
  `m2-run.json` digest, and the final chained hash from **both**
  architectures. This block is a row in 08's acceptance checklist.

## Acceptance (= M2 acceptance items 1–3)

- First-room transition achieved by the committed-format script (script
  itself stored lab-side; its `.padlog` hash recorded in the bring-up log).
- `feature-maps/demo-game.yaml` carries only ramdiff-verified offsets and
  passes `refwork-featuremap validate` + `refwork-verify map-check`.
- 100k-frame cross-arch double-run green (both the synthetic ROM — in CI —
  and the demo game — lab evidence run).
- Bring-up log closed out with the gate decision input for package 08
  (dates, what worked, accuracy-bug catalog).
