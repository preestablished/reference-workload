# 08 — Build-vs-vendor review gate + M2 acceptance checklist

**Depends on:** 06, 07. This package is process + evidence, not code. It
closes Phase-2 exit-gate item 5: "reference-workload M2 review gate decided
(build vs vendor), first room playable host-side from a scripted input log."

## The gate (ARCHITECTURE.md §2, IMPLEMENTATION-PLAN.md M2)

- **Clock:** starts the day package 06 starts (first accuracy-debugging
  session against the operator ROM). Record it in
  `.agents/plans/phase-2/bringup-log.md` on day one. The trigger condition
  is *"first-room not achieved within 3 weeks of M2 start"*.
- **Ratify the clock-start reading before relying on it.** "Within 3 weeks
  of M2 start" plainly read could mean calendar start of all M2 work, not
  06's start; starting the clock at 06 is this plan's interpretation
  (defensible — accuracy debugging can't begin before the tools exist) and
  it moves the Option-B decision weeks later than the plain reading. File
  it as a doc issue against IMPLEMENTATION-PLAN.md M2 and get the
  operator's explicit sign-off, recorded in the bring-up log, *before* 06
  starts.
- **Calendar backstop:** the 06-start clock must not absorb unbounded
  engine-package slip. If packages 01–05 have not all landed within
  **4 weeks of M2 kickoff** (record the kickoff date in the bring-up log
  on day one of *any* M2 work), that is a mandatory gate checkpoint with
  the operator — same agenda as the weekly checkpoints, plus "is Option B
  already the faster path to first-room?".
- **Weekly checkpoints** (3 max): at each, the bring-up log answers — what
  faults remain on the route (per the recon-mode inventory)? is progress
  fault-burn-down (healthy) or heisenbug-chasing (unhealthy)? projected
  first-room date?
- **Option-B pre-survey (de-risk the fallback before it's needed):**
  schedule a half-day, before the week-2 checkpoint, producing a lab-side
  note: candidate open-source cores for this console family, license
  matrix (GPL vs permissive — GPL forces a program-level decision), and
  thread/float red flags — assessed from **READMEs, license files, and
  public docs only, never their source code** (the clean-room rule holds
  until a decision lifts it). If the gate fires, the switch starts from
  this note instead of losing a week of a 4–7-week effort to survey work.
- **Clean-room lift condition (state it, don't improvise it):** reading
  another emulator's source becomes permissible **only after** a recorded
  Option-B decision *and* a completed license review for the chosen core.
  Until both exist, the boundary stands unchanged.
- **Decision at week 3 (or earlier if first-room lands):**
  - **First room achieved → Option A confirmed.** Record the decision.
  - **Not achieved → switch to Option B** (port an existing open-source
    core) per the normative porting checklist in ARCHITECTURE.md §2 items
    1–9, starting from the pre-survey note. Key consequences to state in
    the decision record: license review before any code enters the repo;
    the determinism suite (`refwork-verify`) is emulator-agnostic and
    becomes the port's acceptance test — which holds because 04/05 are
    required to access the core only through the Core API facade
    (`Core::new`/`run_one_frame`/`blit_completed_frame`/…), the surface
    the port must implement; packages 04/05/07 outputs all survive
    unchanged; M3+ are emulator-agnostic by design.
  - A *near-miss* (clear fault burn-down, first room days away) may extend
    by ≤1 week **only** by explicit operator say-so — log it; the default
    is the switch.
- **Decision record:** write
  `~/.agents/projects/determinism/reviews/refwork-m2-build-vs-vendor-decision.md`
  (program-level doc, outside the repo — it may discuss schedule and
  alternatives freely but still names no excluded sources), and a one-line
  pointer in the repo bring-up log. Phase-2's exit gate asks for the
  decision, not just the outcome — write it even when Option A wins.

## M2 acceptance checklist (every clause → a command / evidence item)

| # | Acceptance clause (IMPLEMENTATION-PLAN.md M2) | Verification |
|---|---|---|
| 1 | Scripted input list plays power-on → first room transition | Lab: `refwork-verify play --rom <operator>.rom --script first-room.padlog --map feature-maps/demo-game.yaml --report m2-run.json` — report shows `room_id` change to expected value, zero faults |
| 2 | Framebuffer checkpoints match operator-approved goldens | Lab: same run with `--snap <frame>=…` per checkpoint; byte-compare against lab golden store; operator sign-off recorded in bring-up log |
| 3 | `demo-game.yaml` has verified offsets, validates | Repo CI: `cargo run -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml`; placeholder preamble removed; every entry traceable to a `ramdiff watch` confirmation in the bring-up log |
| 4 | `map-check` passes | Lab: `refwork-verify map-check --rom <operator>.rom --map feature-maps/demo-game.yaml --script first-room.padlog --expect first-room-expect.yaml` exit 0 |
| 5 | 100k-frame double-run, x86_64 **and** aarch64, identical cross-arch hashes | Lab (demo game): `refwork-verify double-run --frames 100000` on both boxes (the provisioned Spark, not QEMU), chained hashes equal. CI (synthetic ROM): nightly cross-arch job green (07) |
| 6 | Gate decided | Decision record exists (above); bring-up log closed; clock-start ratification on record |
| 7 | Evidence run auditable | Provenance block in the bring-up log (06): git rev + rustc of the evidence build, cart BLAKE3, padlog BLAKE3, golden BLAKE3 list, `m2-run.json` digest, both arches' chained hashes. No `--continue-past-faults` artifact anywhere in evidence |
| — | Standing gates never regressed | CI green on main: deny, zero-alloc, 10k double-run, SPC + 65816 corpora, schema drift, fmt/clippy |

## Close-out

- Update the program status memory and
  `~/.agents/projects/determinism/phases/` tracking with: M2 done, gate
  outcome, date.
- File doc issues (same pattern as
  `reviews/doc-issues-refwork-scorer-drift.md`) for any spec drift found
  during bring-up. Two are already known and should be filed **up front**,
  not at close-out: (a) the gate clock-start reading (above); (b) the
  mid-scanline-latching discrepancy — ARCHITECTURE.md §2 says mid-scanline
  register latching "is required" while this plan ships scanline
  granularity with mid-scanline as an on-demand contingency (03 item 2).
  Also propose a normative API.md home for the `.padlog`/expectations
  formats rather than letting the implementation be the only spec.
- Confirm what M2 hands Phase 3: a deterministic core that plays the first
  room (or a decided Option-B port plan), `refwork-verify` as the
  emulator-agnostic harness for M3's mock-agent work, and a real feature
  map for `state-scorer` integration later.
