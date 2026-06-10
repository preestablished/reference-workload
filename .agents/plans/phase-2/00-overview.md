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
| PPU modes 2–7, color math, windows, mosaic, HDMA, H/V counter latch | ✗ registers stored; enables `Fault::UnimplementedBgMode` / `Fault::UnimplementedPpuFeature` (D9 — correct M1 behavior); OPHCT/OPVCT always read 0 |
| Auto-joypad read | ✓ latch at v-blank; ✗ no busy-window timing (scanlines 225–227) |
| `ramdiff` crate | ✗ does not exist |
| `refwork-verify` crate | ✗ does not exist |
| `feature-maps/demo-game.yaml` | placeholder offsets, explicitly marked unvalidated |
| SPC700 single-step gate | skeleton only (`xtask spc-tests`, not in CI, corpus not pinned) |
| Cross-arch determinism | `xtask hash-chain` probe exists; no aarch64 CI/lab gate yet |
| Determinism gates (10k double-run, zero-alloc, deny, schema-drift) | ✓ green in CI — must stay green throughout M2 |

## Work packages (one file each)

| File | Package | Depends on |
|---|---|---|
| `01-apu-spc700-core.md` | SPC700 audio-CPU core + ARAM + timers + IPL bootloader; single-step corpus gate | — |
| `02-apu-dsp-and-integration.md` | DSP (fixed-point per D4), CPU↔APU scheduling, `ApuStub` retirement | 01 |
| `03-ppu-raster-effects.md` | HDMA, color math, windows, mosaic, H/V counter latching, BG modes on demand, auto-joypad busy timing | — |
| `04-ramdiff.md` | `crates/ramdiff` MVP: record (incl. interactive script authoring) / search / candidates / watch / emit | — |
| `05-refwork-verify.md` | `crates/refwork-verify`: input-script format, `play --script`, `map-check` | — |
| `06-accuracy-bringup-and-feature-map.md` | Lab-runner bring-up loop against the operator ROM; first-room script; verified `demo-game.yaml` offsets; golden checkpoints | 02, 03, 04, 05 |
| `07-ci-and-cross-arch.md` | CI: SPC corpus gate, 100k double-run, aarch64 cross-arch hash compare, gate hygiene for new crates | 01–05 (incremental) |
| `08-review-gate-and-acceptance.md` | Build-vs-vendor decision procedure (3-week clock, Option-B trigger), full M2 acceptance checklist | 06, 07 |

## Dependency graph / parallelism

```
01 ──► 02 ──┐
03 ─────────┼──► 06 ──► 08
04 ─────────┤         ▲
05 ─────────┘         │
07 (incremental, must be done before 08 signs off)
```

Packages 01, 03, 04, 05 are mutually independent and parallelizable. 02
needs 01's core. 06 — the long pole and the part with schedule risk — needs
all four engine/tool packages. The **3-week gate clock starts when 06
starts** (first accuracy-debugging session against the operator ROM), per
the IMPLEMENTATION-PLAN wording "within 3 weeks of M2 start"; record the
start date in the gate log (08).

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
