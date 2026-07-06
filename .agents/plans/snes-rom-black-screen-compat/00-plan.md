# Plan: fix refwork-emu so a real commercial SNES ROM renders

Tracking: **refwork-94j**. Revised after two adversarial reviews (emulator
correctness + scope/clean-room/determinism).

## Problem

A real 1 MiB commercial SNES `.sfc` (operator-private; never committed/printed)
loads and runs in `refwork-emu` — `Cartridge::from_rom` accepts it, the CPU
executes, `frame_counter` advances past 1200 frames with **no fault** — but the
framebuffer stays **entirely zero (black) for ~20 s** of emulated time. The same
ROM renders in OpenEMU. `refwork-emu` renders a synthetic test ROM (colored within
30 frames) and the M4 fixture. So this is a **refwork-emu compatibility bug specific
to this title**, not the workload wiring, hypervisor, or bridge (all verified
end-to-end).

**Critical caveat surfaced in review: "loads and runs, no fault" proves nothing
about correct mapping or a stock machine.** `crates/refwork-emu/src/cart.rs` is
**LoROM-only** — `from_rom` reads the reset vector from `rom[0x7FFC]` (the LoROM
location, cart.rs:51-52) and implements one fixed LoROM decode; it never reads the
header map byte (`$FFD5`) or chipset byte (`$FFD6`). A **HiROM** 1 MiB image would
load with a garbage reset vector and wrong bank decode and then execute
non-faulting garbage — *exactly* the "runs, no fault, black" symptom. So the very
first question is **map mode + chipset**, not the APU.

## Goal / deliverable boundary

`refwork-emu` runs this ROM to where it clears force-blank and renders
deterministic non-black frames, proven by a green `linux_m5` on the real ROM plus
the diagnostic evidence. **The deployed snapshot regeneration + `BRIDGE_REAL_SNAPSHOT_REF`
cutover + bridge restart are operator-gated (they invalidate the runtime lease) and
are OUT OF SCOPE for this plan** — hand off to the operator once `linux_m5` is
green. This plan touches only `refwork-emu` (and `xtask/synth_rom` for the
regression); guest-sdk.lock, the image contract, and the bridge are untouched.

## Step 0 — Header classification (do this FIRST; cheapest plan-killer check)

Before any diagnostic, classify the ROM from its header (clean-room safe: emit
**enums/facts only**, never bytes or the title string):

1. Score both header locations — `$7FC0` (LoROM) and `$FFC0` (HiROM) — by the
   checksum/complement pair (`$xFDC`/`$xFDE`) and reset-vector plausibility. Emit
   the classification: **LoROM vs HiROM**.
2. Read the `$FFD6` cart-type nibble and emit its **class** (e.g. `ROM-only`,
   `ROM+RAM`, `ROM+DSP1`, `SA-1`, `SuperFX`, `CX4`, …) — never the raw byte in a
   context that reconstructs code.

Early exits (**STOP and report — new epic, not this fix**):
- **HiROM** → `cart.rs` cannot map it at all (LoROM-hardwired). This is likely THE
  root cause; if so, the fix is "add HiROM mapping to `cart.rs`," a bounded but real
  task — surface it immediately instead of chasing the APU.
- **Any on-cart coprocessor** (SA-1, SuperFX/GSU, DSP-1/2/3/4, CX4, SPC7110, S-DD1)
  → no emulation and no detection exists; out of scope, report as a separate epic.

Only if it classifies **LoROM + plain ROM/(RAM)** do we proceed to Step 1.

## Step 1 — Diagnostic harness (clean-room-safe)

Add `refwork-emu` diagnostics behind **`#[cfg(any(test, feature = "diag"))]`** (a
`#[cfg(feature="diag")] mod diag` + accessors, and `rom_diag` as a `diag`-gated
`example`/`bin`) — **nothing temporary ships and no manual revert exists**; default
builds gain zero public API. `rom_diag` reads the ROM path from an env var (no
default, no committed path), builds a `Core` (`RegionBuffers` via `Box::leak` of
zeroed buffers), runs N frames, and reports per sampled frame:

- **PPU display state:** `force_blank`, `brightness`, `bg_mode`, `tm`/`ts`
  (main/sub layer-enable), and non-zero **counts** of CGRAM, VRAM, OAM.
- **Main-CPU + SPC700 PC histograms** (both — SPC PC via `apu.cpu.pc`), sampled
  per-scanline, with per-**instruction** sampling inside a suspected spin window to
  catch a tight 3–6-instruction loop.
- **Status-register polling:** read **counts and last values** of `$4210` RDNMI,
  `$4211` TIMEUP, `$4212` HVBJOY (these are emulator-generated hardware-status
  bits — not ROM content — so values are safe to log).
- **APU handshake progress (payload-safe):** read/write **counts and access
  addresses** for `$2140–$2143`, plus a rolling **BLAKE3** of the port write stream
  and of `cpu_ports`/`spc_ports` snapshots — **NEVER the raw port values** (that
  stream *is* the game's copyrighted SPC driver in flight).
- **Interrupt/DMA config:** `$4200` NMITIMEN value + an NMI-taken and IRQ-taken
  count vs frame count; `$420B` MDMAEN / `$420C` HDMAEN kick counts + per-channel
  transfer counts.

**Fork:** does force-blank ever clear?
- **Never clears** → stuck in init before display-enable → read where the hot loop
  is: SPC parked in the IPL transfer loop and `cpu_ports` BLAKE3 static ⇒ H1; main
  CPU spinning on `$4210/$4212` ⇒ H2; NMITIMEN never sets bit 7 / NMI-count flat ⇒
  H3; hot PC at an unexpected/garbage address ⇒ H5 (or a Step-0 mis-map slipped
  through).
- **Clears but black** → CGRAM/VRAM stayed zero (DMA gap, H4) or `brightness==0` or
  a compositor gap (H6).

Clean-room invariant for `rom_diag` (hard rule): it may emit only **booleans,
non-zero/access counts, PC/register addresses, and BLAKE3 hashes**. It must never
emit ROM bytes, framebuffer pixels, APU/DMA payload bytes, memory contents,
register **data values** (except the hardware-status regs `$4210/$4211/$4212`), or
the header title.

## Step 2 — Ranked hypotheses and fixes

0. **Mis-mapped HiROM (from Step 0) — check first.** If Step 0 says HiROM, this is
   the fix: implement HiROM address decode + header/reset-vector selection in
   `cart.rs`. Bounded, well-specified, and probably the whole bug.
1. **Non-standard APU IPL protocol (H1).** `apu/ipl.rs` is a *deliberately
   invented* clean-room protocol (self-documented: kick = strobe-0, 1-based
   indices, ipl.rs:14-22,125-141). The **real** boot ROM every commercial audio
   driver targets writes its first data index as `$00`, which this IPL misreads as
   a kick → SPC jumps to a zeroed load address with zero bytes uploaded and stops
   echoing indices → the main CPU's uploader spins forever. Matches the symptom, and
   explains "synthetic renders (authored to the custom protocol), real title
   black." **Fix = reimplement the real IPL's observable upload semantics**
   (zero-based index echo, port1/port0 block-vs-run signaling, address re-latch) —
   a **multi-day clean-room reimplementation**, not a timing patch — and **rewrite**
   the synthetic upload test/ROM to the real protocol (see determinism note).
2. **Status-flag polling (H2).** `$4212` HVBJOY H-blank/auto-joy-busy is
   self-labeled an "APPROXIMATION" (bus.rs:844-863); a game spinning on bit 6 or
   bit 0 can stall on a phase error. `$4210` RDNMI read-clear looks correct
   (bus.rs:827-834). *Fix:* correct the status-bit semantics/timing.
3. **NMI/IRQ cadence (H3).** NMI edge is gated on `nmitimen & 0x80` (bus.rs:344-347);
   if the game never sets bit 7 an NMI-driven init stalls. Distinct path: `$4211`
   TIMEUP / V-H IRQ (`recompute_irq_target`, nmitimen bits 4/5, bus.rs:198-199) — a
   separate stall the plan must instrument (F5). *Fix:* correct NMI/IRQ
   enable/ack/timing.
4. **DMA/HDMA (H4).** Signal: force-blank clears but VRAM/CGRAM stay zero. *Fix:*
   the channel behavior in `dma.rs`.
5. **CPU opcode/addressing bug (H5).** Signal: hot PC at an unexpected address.
   *Fix:* the opcode. Hardest to localize — use the PC histogram + per-instruction
   logging around the stuck PC. (Also the fallback if a Step-0 mis-map executes
   garbage.)
6. **Render-path gap (H6).** Display on, data present, still black. Lower
   likelihood (unsupported BG modes 2/4/5/6 *fault*, and we see none).

## Step 3 — Regression test (no ROM bytes; determinism-safe)

Reproduce the identified defect with a **small synthetic ROM built in-tree**
(`xtask/src/synth_rom.rs`) exercising the specific behavior (HiROM decode, a
real-protocol APU upload, a `$4212` poll, or a VRAM DMA) and assert a byte-exact
state/frame result. **Determinism guard:** `xtask/tests/determinism.rs` hard-codes
WRAM-cell assertions (`$7E:10FE`, `$7E:0010`) and "frame 600 not one color"; Step 3
must be **additive/back-compatible** to the existing synth program, OR those cells
and hashes must be **deliberately re-derived in the same commit** — never disabled.
For the H1 fix specifically, the existing `ipl_upload_roundtrip` and any synthetic
upload artifact are coded to the *old* protocol and will **functionally break** —
they must be **rewritten** to the real protocol, not merely re-pinned.

## Step 4 — Verify (clean-room correctness bar) and hand off

Success is NOT "non-zero pixels" (passes on noise; `brightness==0` renders black
anyway). Require ALL, reported as counts/hashes only:
1. `force_blank` cleared **and** `brightness > 0`;
2. CGRAM **and** VRAM non-zero;
3. a **structural** metric above a floor — distinct-CGRAM-color count ≥ threshold
   or non-uniform-tile count ≥ threshold (rules out flat/noise fills);
4. **two-run frame-hash determinism** at a fixed late frame (same host), with the
   durable expected value recorded via the **synthetic** regression — never the real
   ROM.
Then: **confirm** `image double-build` is still byte-identical (expected — the fix
touches only the emulator crate, not the ROM-free image; the behavior reaches the VM
via the worker/emulator binary). Run `linux_m5` with the real ROM: `BUDGET_REACHED`,
deterministic frame table, and a non-black GetFramebuffer (non-zero count) at a late
frame.

**Then STOP and hand off:** deliver the diagnostic evidence + green `linux_m5`. The
deployed snapshot regen + `BRIDGE_REAL_SNAPSHOT_REF` cutover + bridge restart are
operator-gated (runtime-lease invalidation) — coordinate with the operator; not this
plan's authority.

## Determinism impact (corrected)

`determinism.rs` and `hash_chain.rs` are **double-run self-consistency** checks, not
committed goldens — an emulator timing fix re-derives automatically. The real
hazards are: (a) **Step-3 synth edits** colliding with `determinism.rs`'s hardcoded
WRAM-cell/frame-600 assertions (handle additively or re-derive in-commit); (b) the
**package-06 determinism green stamp** (`determinism.last_green`, image.rs:16-17,403)
is invalidated by any emulator behavior change — trigger the package-06 re-baseline
before any registration; (c) **cross-arch** identical-hash guarantee (hash_chain M2)
isn't proven by single-machine double-run — re-run the cross-arch probe on both
arches after the fix. Protocol-level fixes (H1) invalidate the **inputs** of
APU-touching fixtures, requiring rewrite, not just re-pin.

## Scope guard

- **Step 0 gates everything.** HiROM or coprocessor → STOP and report as a separate
  epic with the classification evidence.
- Otherwise cap at **3 fix/verify cycles**. If frame 600 is still black after 3,
  STOP and deliver the diagnostic table + a ranked residual-gap list rather than
  continuing into an open-ended emulator-completeness project.

## Clean-room

The ROM and any framebuffer are operator-private: never committed, never printed as
bytes. `rom_diag` takes the ROM path from env only; the ROM/`game.img` and any
framebuffer buffer stay in RAM or an operator-private, git-ignored dir. Add a
`.gitignore`/pre-commit guard rejecting `*.sfc`/`*.smc`/`game.img` and framebuffer
dumps. Payload byte-streams (APU upload, DMA) are treated as ROM bytes — only
counts/addresses/hashes may be emitted. Header map-mode/chipset bytes may be read
and reported as **enums** (functional facts). Regression uses an in-tree synthetic
ROM.

## Acceptance criteria

1. Step-0 classification recorded (map mode + chipset) with the go/stop decision.
2. If in scope: `rom_diag` shows force-blank cleared, `brightness>0`, CGRAM+VRAM
   populated, structural metric above floor, two-run determinism — all as
   counts/hashes.
3. A synthetic in-tree regression reproduces the defect and passes post-fix; the
   full `refwork-emu` + `xtask` determinism suite is green (re-derived where needed,
   nothing disabled); `image double-build` byte-identical; package-06 green stamp
   re-baselined; cross-arch chain re-verified.
4. `linux_m5` with the real ROM: `BUDGET_REACHED`, deterministic frame table,
   non-black late frame.
5. Cutover handed off to the operator (out of scope here).

## Bead mapping

- **refwork-94j** (P1, this plan) — top-level compat fix.
- Sub-work to file after Step 0 resolves the root cause (one of): "Add HiROM
  mapping to cart.rs"; "Reimplement real SPC IPL upload protocol"; "Fix
  `$4212`/NMI/IRQ timing"; "Fix DMA VRAM/CGRAM upload". Coprocessor need → separate
  epic.
