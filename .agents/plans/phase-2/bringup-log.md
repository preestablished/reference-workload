# M2 bring-up log (package 06) — dated, no game names

Clean-room reminder: "the demo game" / "the operator-supplied game image"
only. Hashes, verified offsets, and dates are admissible; game-derived
content (ROM, dumps, goldens, script semantics) stays lab-side.

## Gate-clock dates (package 08)

| Event | Date |
|---|---|
| **M2 kickoff** (first M2 work on any package) | 2026-06-10 |
| Engine packages 01–05 landed (repo-side) | 2026-06-10 |
| **4-week calendar backstop** on 01–05 (mandatory gate checkpoint if not landed by) | 2026-07-08 — met early, see above |
| **06 start** (first accuracy-debugging session against the operator ROM) | _TBD — record on day one_ |
| 3-week build-vs-vendor decision due (06 start + 21 days) | _TBD_ |

**Clock-start ratification:** the 3-week clock starting at 06's start (not
M2 kickoff) is this plan's *interpretation* of IMPLEMENTATION-PLAN.md M2
"within 3 weeks of M2 start". Filed as a doc issue
(`~/.agents/projects/determinism/reviews/doc-issues-refwork-m2-plan.md` §1);
operator sign-off required **before 06 starts**: ☐ ratified on ______ by ______.

## Preconditions (all before the clock starts — package 06)

- [ ] aarch64 lab box ("the Spark") provisioned: toolchain installed, repo
      builds, ROM accessible; green synthetic-ROM cross-arch double-run vs
      the Intel box (hash equality). QEMU is not acceptable for the
      M2 evidence run.
- [ ] Interactive environment designated (machine + operator) for
      `ramdiff record --interactive`; windowing smoke test run **on that
      machine**; if not a lab box, operator sign-off on ROM handling
      recorded here.
- [ ] 03 core lane fully landed — done 2026-06-10 (build-order items 1–7,
      plus pre-built modes 3 and 7).
- [ ] `refwork-verify play --continue-past-faults` recon mode working —
      done 2026-06-10 (rejected by map-check/double-run, banner printed).
- [ ] PPU display-test ROMs (03 close-out carry-over): operator fetches the
      chosen public display-test ROMs on the interactive machine, runs them
      over the implemented feature set, records pass/fail here; pin any
      hash-comparable ones in `xtask/test-roms.lock`.

## Option-B pre-survey

Due before the week-2 checkpoint (half-day, lab-side note): candidate
open-source cores, license matrix, thread/float red flags — assessed from
READMEs/licenses/public docs ONLY (clean-room holds until a recorded
Option-B decision plus completed license review lifts it).

- [ ] Pre-survey note written: ______ (lab-side path)

## Weekly checkpoints (3 max once 06 starts)

Template per checkpoint: faults remaining on the route (recon inventory) /
fault-burn-down or heisenbug-chasing? / projected first-room date.

| # | Date | Faults remaining | Health | Projected first-room |
|---|---|---|---|---|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |

## Session log

_(dated entries; newest last)_

- **2026-06-10** — M2 kickoff. Repo-side engine packages landed on
  `phase-2/m2-impl`: 01 (SPC700 core, corpus 256,000/256,000), 03 (PPU
  raster core lane + modes 3/7), 04 (`ramdiff`), 05 (`refwork-script`,
  `refwork-hash`, `refwork-verify`), 02 in progress. No operator-ROM work
  yet; the gate clock has NOT started.

## Provenance block (filled by the final M2 evidence run)

| Item | Value |
|---|---|
| repo git rev of evidence build | |
| rustc version | |
| cart BLAKE3 | |
| `first-room.padlog` BLAKE3 | |
| golden-framebuffer BLAKE3 list | |
| `m2-run.json` digest | |
| chained hash — x86_64 | |
| chained hash — aarch64 | |
| `--continue-past-faults` artifacts in evidence | must be NONE |
