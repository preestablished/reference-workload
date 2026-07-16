# Current State (Evidence-Based, Assessed 2026-07-12)

## This Repo

`main` at `4bb7eba` ("Record gate-3 bridge cutover verification: Phase 3
exit gates all green"). The working tree is **not clean**: it holds
`refwork-czi`'s uncommitted in-progress implementation (modified
`refwork-verify` phase4 sources + new `phase4_*.rs` files and a `data/`
directory) — exactly the state czi's 2026-07-11 comment describes
("held open until implementation is committed"). Don't treat that diff
as abandoned work. Phase 3 is closed: M5 20× zero-flake
stamp, first-room in-VM + browser-visible after the
`BRIDGE_REAL_SNAPSHOT_REF` cutover to the `948b73e6` READY snapshot
(record: `rom-operator-bridge/.agents/handoffs/2026-07-12-real-snapshot-cutover-confirmation.md`),
epic `refwork-d7t` closed.

Bead census (`bd list --limit 0`): 7 issues — 6 open, 1 in progress.
The three phase4-labeled ones are the fast-follow's:

- `refwork-czi` (in progress) — capture exporter + synthetic contracts;
  2026-07-11 comment: implementation done at base `4eb8a3a`, all locked
  gates green, **held open until committed + final clean-checkout gate
  recorded**.
- `refwork-20v` (open) — private real feature map / scoring / layout
  validation.
- `refwork-5tk` (open, depends on both) — corpus production/freeze/
  handoff; 2026-07-11 launch decision **no-go** pending an approved
  operator session (full-corpus or first-room fallback).

The remaining four beads are P2/P3 emulator-perf investigations
(`refwork-4nv`, `-rbz`, `-0um`, `-hbh`) — unrelated to M6.

## M6 Tooling Already Built

- `refwork-verify trace` and `phase4-score-plan` subcommands exist
  (`crates/refwork-verify/src/main.rs`, built and test-locked under the
  fast-follow's tooling wave) — the trace→labeled-JSONL conversion M6's
  acceptance names is implemented, unexercised on real hand-play data.
- `scoring/demo-game.yaml` + `feature-maps/demo-game.yaml` exist with
  **placeholder offsets**; the checked-in map is contractually
  disqualified for real work ("no placeholder offsets") — the real
  private pair is `refwork-20v`'s deliverable.
- `ramdiff` record tooling exists (M2 deliverable).

## The Joint Counterpart Does Not Exist Yet

state-scorer is a Phase 0 skeleton (`f3e3592`, stub crates). Its M1→M4
chain was requested today
(`state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/`).
M6's phase-doc dependency is scorer **M2–M3** (program compiles; hash +
goal evaluation); M6's service-level acceptance items (<1 ms/state on a
captured region set, end-to-end smoke) additionally want scorer **M4**'s
running service. The scorer proto is already published
(control-plane tag `proto-v0.2.0`, `determinism/scorer/v1/scorer.proto`).

## Exploration-Readiness Counterparts

- exploration-orchestrator: loop complete on fakes (M0–M5; 6 open
  P2/P3 beads). The M6 smoke ("orchestrator dev-loop … 1,000 bursts,
  zero Faults") needs their loop pointed at the real stack + live
  scorer; input-synthesizer v1 (also requested today) is the input
  source, though the plan allows "even a trivial random-input loop".
- Deployed stack: bridge systemd unit → dh-workerd `6e348e5` → durable
  snapstore copy at `~/.rbo73/m4-regen-20260707/`. Worker + snapstore
  are user processes and die on reboot; smokes need a confirmed-up
  stack. Dangling-intent 503s recover via the bridge's audited
  `clear-dangling-intents` subcommand.

## Program Debt Adjacent To M6 (Not Owned Here)

- The hypervisor RSS-leak fix is deployed but its live verification is
  still open — tracked as **rom-operator-bridge bead
  `rom-operator-bridge-l1w`** (P1, the worker-OOM incident record,
  gated on an operator-authorized pass). Long capture/smoke sessions
  should not overlap live Play until it closes, per the 07-10 program
  flags.
- snapshot-store M8 in progress (`snapshot-store-2dl`, P0). Neither
  item gates M6's acceptance; both gate how hard Phase 5 can lean on
  the stack.
