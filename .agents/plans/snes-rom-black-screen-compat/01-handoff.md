# Handoff: finish the refwork-emu APU-handshake fix so the real ROM renders

**For the next coding agent.** Read `00-plan.md` first (the reviewed plan). This
file is the current-state + how-to-finish. Tracking bead: **refwork-94j**.

## TL;DR

A real commercial LoROM SNES game runs in `refwork-emu` (frames advance, no
fault) but renders **black forever** because the APU boot handshake deadlocks.
Diagnosis is **complete and committed**; the **fix is not started**. The fix is
a determinism-sensitive APU↔CPU port-synchronization change. Your job: implement
it, keep determinism intact, and prove the real ROM renders.

## Status

| Phase | State |
|---|---|
| Plan (reviewed by 2 agents) | ✅ `00-plan.md` |
| Step 0 — ROM classification | ✅ LoROM, ROM-only, no coprocessor (in scope) |
| Step 1 — diagnostic harness | ✅ committed: `introspect`-gated accessors + `examples/rom_diag.rs` |
| Root cause | ✅ pinpointed (below) |
| Step 2 — the fix | ❌ **not started — your work** |
| Step 3 — regression test | ❌ not started |
| Step 4 — verify + operator cutover | ❌ not started (cutover is operator-gated) |

Commits on `reference-workload@main`: `8eff8d9` (plan + diag tooling),
`8fc43c5` (write-side diag). The earlier `40eaf4f` (refwork-harness SdkPlatform)
is the separate, already-shipped fix that made frames publish at all.

## Root cause (pinpointed to the instruction)

The SNES boot uploads the game's SPC700 audio driver through APU I/O ports
`$2140–$2143` before enabling the display. In `refwork-emu` this **deadlocks**:

- **Main CPU** (65816): does its 6 setup writes, **delivers the `$CC` kick to
  port 0**, then spins at PC `$A857` reading `$2140–$2143` (>500k reads) waiting
  for the SPC to echo.
- **SPC700**: stuck at PC `$FFCC` — the IPL boot ROM's "wait until port 0 ==
  `$CC`" poll loop (`crates/refwork-emu/src/apu/ipl.rs`, label `poll_cc` at
  `$FFC8`).
- **Key signal:** `wr_CC=true` (the kick was written) but `spc_port0_is_CC=false`
  the entire time — when the SPC polls, its port-0 latch is **not** `$CC`.

**Why:** the driver writes `$CC` to port 0 and then overwrites it (with the
running transfer index) *faster than the coarsely-caught-up SPC can observe the
transient `$CC`*. The emulator only steps the SPC (`Apu::advance_master_cycles`
→ `step`) at APU-port access boundaries and at end-of-scanline, advancing it by
the master cycles elapsed since the last catch-up. Between two back-to-back
main-CPU port writes (~a couple master cycles) the SPC advances a fraction of an
SPC cycle — not enough to execute its poll read — so it never sees `$CC`. A
real, concurrently-clocked SPC (and OpenEMU, where this ROM renders) catches it.

So: **APU↔CPU port-synchronization granularity is too coarse; the SPC misses
main-CPU port transients.** This is hypothesis H1 in `00-plan.md`, refined from
"non-standard IPL protocol" to "the sync model loses the kick."

## How to reproduce (2 minutes)

```
# Private ROM (operator-machine only; NEVER commit/print its bytes):
REFWORK_DIAG_ROM=/home/infra-admin/.rbo73/private-rom/game.img \
REFWORK_DIAG_FRAMES=120 \
  cargo run -p refwork-emu --features introspect --example rom_diag
```
Current (broken) output — force_blank stays true, spc_pc stuck `0xffcc`,
`inIPL=true`, `spc_port0_is_CC=false`, `cgram_nz=0` for all frames. When fixed,
you should see force_blank flip to false, `spc_in_ipl` go false (SPC jumps to
uploaded driver), and CGRAM/VRAM become non-zero within a few hundred frames.

`rom_diag` is **clean-room-safe**: it prints only booleans, counts, PC
addresses, and known protocol constants (`$CC`) — never ROM bytes, framebuffer
pixels, memory contents, or APU/DMA payload. Keep it that way.

## Diagnostic tooling already in place (use and extend it)

All `#[cfg(feature = "introspect")]` (compiled out of the guest binary; guest
build stays clean, 174 tests green):
- `Ppu::diag()` → `(force_blank, brightness, bg_mode, tm)`;
  `Ppu::diag_nonzero_counts()` → CGRAM/VRAM/OAM non-zero counts
  (`crates/refwork-emu/src/ppu/mod.rs`).
- `Core::diag_snapshot()` → `DiagSnapshot` with main/SPC PC, force-blank, the
  `$4210/$4211/$4212/$2140-$217F` read counters, APU write count, `wr_cc_port0`,
  `spc_port0_is_cc` (`crates/refwork-emu/src/core_impl.rs`).
- Bus read/write counters on `$4210/$4211/$4212` and `$2140-$217F`
  (`crates/refwork-emu/src/bus.rs`).
- `examples/rom_diag.rs` harness.

**Add more introspection freely** (SPC PC histogram, per-instruction sampling in
the spin window, `cpu_ports`/`spc_ports` change-detection) — but obey the
clean-room rule above. Consider a finer per-instruction SPC-PC trace to confirm
the SPC executes a `$F4` read while `spc_ports[0] == $CC` after the fix.

## The fix (Step 2) — approach

Goal: the SPC700 must observe main-CPU APU-port writes with real-hardware-like
ordering, so the IPL handshake completes.

Where the sync lives:
- `crates/refwork-emu/src/bus.rs`: the `$2140-$217F` read/write arms call
  `self.apu_catch_up()` then `cpu_read_port` / `cpu_write_port`. `apu_catch_up`
  advances the SPC by elapsed master cycles.
- `crates/refwork-emu/src/apu/mod.rs`: `advance_master_cycles` → `step` (SPC700
  + timers + DSP), with the `SPC_NUM/SPC_DEN` (1024/21477) fractional-cycle
  accumulator; DSP clocked 1 sample / 32 SPC cycles.
- `crates/refwork-emu/src/apu/spc700.rs`: the SPC700 core (`pub pc`,
  `execute`).
- `crates/refwork-emu/src/apu/ipl.rs`: the 64-byte clean-room IPL boot ROM
  (fully disassembled in comments; `poll_cc` at `$FFC8`, transfer loop at
  `$FFE0`, kick jump at `$FFFA`).
- Frame driver: `crates/refwork-emu/src/core_impl.rs` `run_one_frame` —
  per-scanline: `start_line`, `run_cpu_until(target)`, `apu_catch_up`,
  `render_scanline`.

Candidate strategies (evaluate; pick the one that is correct AND minimally
perturbs determinism):
1. **Finer interleaving (most faithful):** run the CPU and SPC in smaller
   quanta so a port write is immediately followed by enough SPC steps to react —
   e.g. after any `$2140-$217F` *write*, step the SPC by a bounded number of SPC
   cycles before returning, or interleave `run_cpu_until` in sub-scanline
   chunks. Must stay deterministic (no wall-clock; fixed quanta).
2. **Port-latch "seen" semantics:** model that a port write is observable by the
   SPC until the SPC has had a chance to read it (a hardware-accurate latch with
   a pending/observed flag), so a rapid overwrite can't erase an unobserved
   value before the SPC's next read. Verify against real SNES port semantics
   before adopting — do not invent behavior.
3. **Re-derive the real IPL protocol** if analysis shows the current `ipl.rs`
   sequence is itself wrong (not just the timing). If so, reimplement the real
   boot ROM's *observable* protocol from **public hardware documentation only**
   — never copy Nintendo's IPL ROM bytes. This also requires rewriting the
   synthetic upload test/ROM coded to the current protocol.

Validate the chosen fix with `rom_diag` (the oracle): the SPC must leave the
IPL, the driver must upload, force-blank must lift, graphics must load.

## Determinism guardrails (do not skip)

Any APU/SPC/DSP timing change ripples into deterministic state. Per the review:
- **Golden re-derivation:** re-run `cargo test -p refwork-emu` and `xtask`
  determinism tests; re-derive (never delete) any changed expected hashes.
- **`xtask/tests/determinism.rs` hazard:** it hard-codes WRAM-cell assertions
  (`$7E:10FE`, `$7E:0010`) and "frame 600 not one color." If your change (or a
  new synthetic test ROM) shifts these, re-derive them **in the same commit**.
- **Synthetic upload tests** (`apu/mod.rs` `ipl_upload_roundtrip`, and
  `xtask/src/synth_rom.rs` upload paths) are coded to the current protocol; a
  protocol change **rewrites** them, not just re-pins.
- **Package-06 determinism green stamp** (`determinism.last_green`, see
  `xtask/src/image.rs`) is invalidated by any emulator behavior change —
  trigger the package-06 re-baseline before any image registration.
- **Cross-arch:** re-run the `hash_chain` M2 cross-arch probe on x86_64 **and**
  aarch64 and confirm identical chains.
- `xtask image build` / `image double-build` must remain byte-identical (the
  image is ROM-free; the emulator fix reaches the VM via the worker binary, not
  the image). Confirm this, don't assume.

## Clean-room (hard rules)

The ROM and any framebuffer are operator-private copyrighted material.
- Never commit or print ROM bytes, framebuffer pixels, memory contents,
  APU/DMA payload byte values, or the header title. `.gitignore` already blocks
  `*.sfc`/`*.smc`/`game.img`/`*.fb.bin`.
- Diagnostics emit only booleans, counts, PC/register **addresses**, and known
  **protocol constants** (`$AA`/`$BB`/`$CC`). Header map-mode/chipset may be
  read and reported as **enums** (functional facts).
- Reimplementing hardware protocol from public documentation is fine; copying
  Nintendo's IPL ROM bytes is not. The existing `ipl.rs` is a clean-room
  reimplementation — keep that property.

## Acceptance criteria

1. `rom_diag` on the real ROM: `force_blank` clears **and** `brightness > 0`;
   `spc_in_ipl` false (SPC handed off); CGRAM **and** VRAM non-zero; a
   structural metric (distinct-CGRAM-colors ≥ threshold or non-uniform-tile
   count ≥ threshold) above a floor (rules out noise); two-run frame-hash
   determinism at a fixed late frame — all reported as counts/hashes.
2. Full `refwork-emu` + `xtask` determinism suite green (re-derived where
   needed, nothing disabled); `image double-build` byte-identical; package-06
   green stamp re-baselined; cross-arch chain re-verified.
3. A synthetic in-tree regression (extend `xtask/src/synth_rom.rs`, additive or
   re-derive `determinism.rs` cells in-commit) reproduces the handshake defect
   and passes post-fix.
4. `linux_m5` with the real ROM (in `determinism-hypervisor`, branch
   `codex/determinism-hypervisor-tqvb-phase3-no-frame-restore`, test
   `linux_m5_frame_budget_records_post_ready_frame_marks`): `BUDGET_REACHED`,
   deterministic frame table, and a **non-black** GetFramebuffer at a late frame
   (needs `DH_M9_*` staged — see below and `.agents/plans/snes-rom-black-screen-compat/00-plan.md`).

## Environment / repro handles

- Repos (siblings under `/home/infra-admin/git/preestablished/`):
  `reference-workload` (emulator + image), `determinism-hypervisor` (worker +
  `linux_m5`), `guest-sdk` (pinned rev `487ff56` for image builds), `snapshot-store`.
- Private ROM (operator-only): `/home/infra-admin/.rbo73/private-rom/game.img`
  (1 MiB LoROM, ROM-only). A `game.img` built from it also lives in the M9
  artifact set used by the handoff.
- End-to-end verify via `linux_m5` (real emulator through the worker): stage
  `DH_M9_BZIMAGE/INITRAMFS/BASE_IMAGE/GAME_IMAGE/IMAGE_CACHE`, `DH_M9_GUEST=linux`,
  `DH_M9_ALLOW_SKIP=0`, then
  `cargo test -p dh-worker --test m5_frame_scheduling linux_m5_frame_budget_records_post_ready_frame_marks -- --ignored --nocapture`.
  Note: the test's `FRAME_HARD_CAP=50M` is fixture-calibrated; the real emulator
  is ~25M instr/frame — a DH-side follow-up (`08-followup-frame-hard-cap.md` on
  that branch) asks them to raise it. Raise it locally to validate.
- **Deployed cutover is operator-gated and OUT OF SCOPE**: once the real ROM
  renders in `linux_m5`, regenerate the READY snapshot (dh-m9-ready-handoff with
  the fixed image) and hand the `BRIDGE_REAL_SNAPSHOT_REF` cutover + bridge
  restart to the operator. Do not self-serve it (it invalidates the runtime
  lease).

## Scope guard (from the review)

Cap at **3 fix/verify cycles**. If the real ROM's frame is still black after 3,
stop and deliver the diagnostic table + a ranked residual-gap list rather than
open-ending into a general emulator-completeness project. (Step 0 already ruled
out HiROM and coprocessors, so the machine model is in scope.)
