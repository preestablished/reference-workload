# Phase 2 / M2 — Plan Overview

**Goal:** deliver this repo's Phase-2 scope per
`~/.agents/projects/determinism/phases/phase-2-fork-and-replay.md`: milestone
**M2 — demo game first room, host-side**, which is also the program's
**build-vs-vendor review gate** for the emulator (Phase-2 exit gate item 5).

M2 is the *only* reference-workload work in Phase 2. It is a parallel track
with **zero platform dependencies** — no hypervisor, no snapshot-store, no
guest-sdk; everything here runs host-side. M3 (harness/protocol) and M4
(guest image) are Phase-3 scope and are explicitly out of this plan.

## What M2 requires (IMPLEMENTATION-PLAN.md, verified 2026-06-10)

> Scope: full APU (audio CPU + DSP, fixed-point per D4); PPU mid-frame raster
> effects (HUD split), color math, windows; auto-joypad timing; accuracy
> debugging against the operator-supplied game image on the lab runner (CI
> uses the synthetic ROM only). `ramdiff` MVP
> (record/search/candidates/watch/emit — ARCHITECTURE.md §5) used to validate
> the real feature-map offsets for the operator's ROM revision.
>
> Accept:
> - A hand-authored scripted input list (host-side runner `refwork-verify
>   play --script`) takes the demo game from power-on through the first room
>   transition: `room_id` feature changes to the expected value; framebuffer
>   snapshots at scripted checkpoints match operator-approved goldens (stored
>   in the lab, not the repo).
> - `feature-maps/demo-game.yaml` updated with verified offsets;
>   `refwork-verify map-check` passes (scripted run asserts expected feature
>   trajectory).
> - 100k-frame double-run determinism (host-side) green on x86_64 **and
>   aarch64** (cross-arch identical hashes).
> - **Gate:** if first-room is not achieved within 3 weeks of M2 start,
>   switch to the Option-B port per the ARCHITECTURE.md §2 checklist; M3+ are
>   emulator-agnostic.

## Current state (verified against the working tree, main @ 99e79cd)

| M2 ingredient | State today |
|---|---|
| Audio CPU + DSP | ✗ `ApuStub` (`crates/refwork-emu/src/apu.rs`): deterministic handshake/echo simulator only; module doc marks it for M2 replacement; `FrameFlags::APU_STUB_ACCESS`/`APU_STUB_HANDSHAKE` harvested per frame |
| PPU modes 0–1, scanline renderer, sprites | ✓ M1 (`ppu/mod.rs` `render_mode0`/`render_mode1`) |
| PPU modes 2–7, color math, windows, mosaic, HDMA | ✗ registers stored; enables `Fault::UnimplementedBgMode` / `Fault::UnimplementedPpuFeature` (D9 — correct M1 behavior) |
| H/V counter latch | ✓ mostly M1: $2137 SLHV latch, OPVCT, the $213C/$213D two-read flip-flops, and $213F latch-clear all work (`ppu/mod.rs` ~670, ~727–760); ✗ OPHCT always 0 — no H dot counter |
| Auto-joypad read | ✓ latch at v-blank + busy flag (`bus.rs` `auto_joy_busy`, set line 225 / cleared line 228, read in $4212 bit 0); ✗ $4218/$4219 expose the new latch immediately during the busy window instead of stale data |
| `ramdiff` crate | ✗ does not exist |
| `refwork-verify` crate | ✗ does not exist |
| `feature-maps/demo-game.yaml` | placeholder offsets, explicitly marked unvalidated |
| SPC700 single-step gate | corpus already pinned (`xtask/test-roms.lock` `spc700-singlestep` @ 67d15f49, BLAKE3'd); runner is a skeleton (`xtask spc-tests`), not in CI |
| Cross-arch determinism | `xtask hash-chain` probe exists; no aarch64 CI/lab gate yet |
| Determinism gates (10k double-run, zero-alloc, deny, schema-drift) | ✓ green in CI — must stay green throughout M2 |

## Work packages (one file each)

| File | Package | Depends on |
|---|---|---|
| `01-apu-spc700-core.md` | SPC700 audio-CPU core + ARAM + timers + IPL bootloader; single-step corpus gate | — |
| `02-apu-dsp-and-integration.md` | DSP (fixed-point per D4), CPU↔APU scheduling, `ApuStub` retirement | 01 |
| `03-ppu-raster-effects.md` | Core lane (blocks 06): HDMA, color math, windows, mosaic, OPHCT dot counter, auto-joypad stale reads. On-demand lane (open during 06): BG modes 2–7, mid-scanline contingency | — |
| `04-ramdiff.md` | `crates/ramdiff` MVP: record (incl. interactive script authoring) / search / candidates / watch / emit | `refwork-script` (05) |
| `05-refwork-verify.md` | `refwork-script` micro-crate (`.padlog` format — **day-1 deliverable**), `crates/refwork-verify`: `play --script`, `map-check`, `double-run`; `refwork-hash` shared hashing | — |
| `06-accuracy-bringup-and-feature-map.md` | Lab-runner bring-up loop against the operator ROM; first-room script; verified `demo-game.yaml` offsets; golden checkpoints | 02, 03-core, 04, 05 + lab preconditions (see 06) |
| `07-ci-and-cross-arch.md` | CI: SPC corpus gate, 100k double-run, aarch64 cross-arch hash compare, gate hygiene for new crates | 01–05 (incremental) |
| `08-review-gate-and-acceptance.md` | Build-vs-vendor decision procedure (3-week clock, Option-B trigger + pre-survey), full M2 acceptance checklist | 06, 07 |

## Dependency graph / parallelism

```
05a (refwork-script, day 1) ──► 04, 05
01 ──► 02 ──────┐
03 (core lane) ─┼──► 06 ──► 08
04 ─────────────┤    ▲
05 ─────────────┘    │
03 (on-demand lane: BG modes, reopened by 06 faults)
07 (incremental, must be done before 08 signs off)
```

After the day-1 `.padlog` format lands (05a), packages 01, 03-core, 04, 05
are mutually independent and parallelizable. 02 needs 01's core. 06 — the
long pole and the part with schedule risk — needs 02, 03-core, 04, 05 *and*
the lab preconditions listed in 06 (Spark provisioned, interactive
environment designated). Note that 03's on-demand lane **reopens during
06**: D9 halts on the first fault, so each lab run surfaces one missing
feature at a time — the recon mode in 05 and the pre-build recommendation
in 03 exist to break that serialization.

**Gate clock:** the 3-week build-vs-vendor clock starts when 06 starts
(first accuracy-debugging session against the operator ROM). This is an
*interpretation* of IMPLEMENTATION-PLAN.md's "within 3 weeks of M2 start" —
the plain reading could mean calendar start of all M2 work. Package 08
requires getting this reading ratified by the operator (filed as a doc
issue) **and** sets a calendar backstop on the engine packages so the
interpretation can't absorb unbounded slip: if 01–05 have not landed within
4 weeks of M2 kickoff, that is itself a mandatory gate checkpoint. Record
both dates (M2 kickoff, 06 start) in the gate log.

## Standing constraints (apply to every package)

- **Clean-room source boundary** (reviews/clean-room-source-boundary.md):
  never name the commercial console, the game, or excluded third-party
  platforms in code, comments, commits, or docs. Chip part names from public
  hardware references (65C816, SPC700) are fine; ROM files use `.rom`; the
  game is "the demo game" / "operator-supplied game image". Allowed sources:
  these docs, public hardware references and public test-ROM suites, public
  Rust/crate docs, operator-supplied artifacts. Do not consult other
  emulators' source code — vendor the *test knowledge*, not the code.
- **Determinism contract D1–D9** (ARCHITECTURE.md §1) is a design input for
  every new line in `refwork-emu`: no threads, no clocks, no RNG, **no
  floats** (D4 — the deny gate already scans `refwork-emu`; new APU/PPU code
  is inside its scope automatically), all state in plain `Core` fields (D5),
  zero per-frame allocations after frame 1 (D8 — ARAM/DSP/window buffers
  allocated in `Core::new`), fault loudly on anything unimplemented (D9).
- **CI never sees game content.** The synthetic ROM remains CI's only
  workload. Everything involving the operator ROM happens on lab runners;
  goldens and the ROM itself are stored in the lab, never the repo.
- **Existing gates stay green on every PR**: 10k double-run, zero-alloc,
  deny, fmt/clippy, featuremap validate, schema drift.

## Out of scope for M2 (do not build now)

- `refwork-harness` state machine, fd-3 control loop, mock agent → M3.
- `xtask image`, guest image pipeline, `audit-syms` → M4 (M3 for audit-syms).
- `refwork-verify trace` (labeled feature trajectory JSONL) and the
  full-stack double-run/snapshot-restore suite → M5/M6. Design 05's script
  format so these can be added without breaking it.
