# 01 — SPC700 audio-CPU core, ARAM, timers, IPL bootloader

**Replaces nothing yet** — `ApuStub` stays wired into the bus until package
02 integrates the full unit. This package builds the audio CPU as a
self-contained module inside `refwork-emu`, proven against a pinned
single-step corpus the same way the 65C816 core was.

## Deliverables

1. `crates/refwork-emu/src/apu/` module tree (convert `apu.rs` into
   `apu/mod.rs` + submodules; keep `ApuStub` compiling in place until 02):
   - `apu/spc700.rs` — the SPC700 CPU core: full instruction set (all 256
     opcodes), all addressing modes, PSW flags, direct-page semantics,
     `MUL`/`DIV`, `DAA`/`DAS`, `SLEEP`/`STOP` (fault per D9 — the demo game
     never legitimately reaches them mid-run), cycle counts per opcode from
     public references. Table-driven like `cpu/`, same style.
   - `apu/aram.rs` — 64 KiB audio RAM, owned `Box<[u8; 0x10000]>` inside the
     APU struct (plain memory, D5). Fixed init pattern analogous to
     `WRAM_INIT_BYTE` (D3): pick and document a constant fill (the console's
     real ARAM power-on is analog-uncertain → fixed documented constant).
   - `apu/timers.rs` — the three timers (two 8 kHz, one 64 kHz): target
     registers, 4-bit up-counters, enable/clear semantics, divider behavior.
   - `apu/ipl.rs` — **our own** 64-byte IPL boot program implementing the
     publicly documented upload protocol ($AA/$BB ready signature, port-0
     index echo, address/kick handshake). Written fresh from register-level
     public docs; do **not** copy the original ROM bytes or any emulator's
     reimplementation (clean-room boundary + license). Mapped at $FFC0 with
     the documented enable/disable control bit.
2. Register surface: the four I/O ports (CPU side $2140–$2143 ↔ SPC side
   $F4–$F7), control register $F1 (timer enables, port clears, IPL enable),
   DSP address/data ports $F2/$F3 (stub target until 02 — reads/writes go to
   a 128-byte register array), test register $F0 (fault on nonzero write,
   D9).
3. `xtask spc-tests` finished (the skeleton exists in
   `xtask/src/spc_tests.rs`): load the public SPC700 single-step JSON corpus,
   compare registers/PSW/cycles per instruction via a `cfg(feature =
   "introspect")` hook, `--filter`/`--max-fail` like `cpu-tests`. Pin the
   corpus (URL + BLAKE3) in `xtask/test-roms.lock` next to the 65816 entry;
   fetched by `cargo xtask fetch-test-roms`, never committed.

## Design constraints

- **No floats, no threads, no clocks** — the deny gate already covers
  `refwork-emu`; this package adds nothing host-environmental. Cycle math is
  integer.
- The APU struct owns *all* its state (CPU regs, ARAM, timers, ports, DSP
  register array) as plain fields — snapshot semantics come free via D5.
- Allocation only in the constructor (D8): `Apu::new()` boxes ARAM once;
  nothing grows after init.
- Keep the SPC700 core's step function shaped like the 65C816's
  (`step() -> cycles`) so package 02 can drive it from a master-clock
  accumulator.

## Acceptance (package-local)

- SPC700 single-step corpus passes 100% via
  `cargo xtask spc-tests --dir <fetched>` (target the same bar as the 65816
  corpus: every case, zero tolerated failures; if specific corpus cases are
  demonstrably wrong vs. public hardware docs, exclude by name in
  `test-roms.lock` with a comment — same mechanism as M1 used, if any).
- Timer unit tests: divider/target/counter behavior vectors hand-derived
  from public docs, including the stage-counter quirks documented there.
- IPL upload protocol unit test: a scripted CPU-side port sequence
  (ready-wait → transfer blocks → kick) round-trips a payload into ARAM and
  jumps to it — asserted with `debug_peek`-style introspection. This test
  replicates what `ApuStub` faked, against the real core.
- `cargo xtask deny` and zero-alloc/double-run gates stay green (the module
  is not yet on the bus path, so the latter is trivially true — the gate run
  proves no accidental wiring).
