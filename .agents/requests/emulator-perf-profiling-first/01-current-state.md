# Current State (Evidence-Based)

Assessed 2026-07-07 (round 3). Two open requests precede this one in
this repo (round-1 Phase-3 gates; round-2 corpus fast-follow); both are
unexecuted and neither touches performance — round-1 explicitly
out-scoped it ("Emulator performance work … separate conversation, not
a Phase 3 gate").

## Where The Number Comes From

- The ~25M instr/frame figure surfaced on the **hypervisor side** as
  test-cap calibration fallout
  (`.agents/plans/snes-rom-black-screen-compat/01-handoff.md:200`;
  the bridge's `08-followup-frame-hard-cap.md` — filed *to* the
  hypervisor — asked them to raise the cap, which their round-1
  request now covers); their bead `38b6` later measured 27.8M and
  ~90–115 ms/frame guest execution, deferring their epoch-hash M4 on
  "needs reference-workload emulator speedup."
- **In this repo: nothing.** No `[[bench]]`, no criterion, no
  `benches/`, no flamegraph artifacts. The only executable harness is
  `examples/rom_diag.rs` (introspection, not timing).

## The Architecture (What The Instructions Are Probably Doing)

`crates/refwork-emu` (~15.5k LOC), cycle-accurate interpreted SNES,
zero-dependency, no threads/clocks/RNG/floats (the D1–D4 contract):

- CPU: 65C816 interpreter, flat 256-opcode `match` dispatch
  (`cpu/exec.rs`, ~1800 lines).
- Bus: **every** byte access advances the master clock via
  `mem_speed()` (`bus.rs` ~1840 lines, `timing.rs:50`) — per-access
  accounting.
- Frame loop: 262 scanlines, `run_cpu_until` per line, then
  `ppu.render_scanline`, then `apu_catch_up` (`core_impl.rs:158`).
- APU: a full second interpreted CPU (SPC700 + DSP + timers, ~5.7k
  LOC) on a scanline-boundary catch-up model.
- PPU: per-scanline software renderer (~3.5k LOC).

Two nested interpreters + per-access clock accounting + per-scanline
rendering is exactly the shape that yields tens of millions of host
instructions per emulated frame. Which of those dominates is
**unknown** — that's this request.

## The Determinism Coupling (Why "Just Optimize" Is A Trap)

- icount = host guest-mode retired instructions
  (hypervisor `dh-detclock`, `PERF_COUNT_HW_INSTRUCTIONS`,
  exclude_host); virtual time `vns = icount × clock_num/clock_den` —
  "a pure function of retired instructions" (this repo's ARCHITECTURE
  D2).
- **Survives a semantics-preserving speedup:** frame-content hashes
  `(wram, fb)` — the M1 10k double-run, the M2 100k cross-arch gate,
  the M5 suite — and input alignment, which is `at_frame` resolved
  through the runtime-recorded frame→icount table.
- **Breaks:** anything keyed on absolute icount/vns recorded under the
  old build — epoch-hash chains (boundaries every `epoch_len` icount),
  icount-addressed snapshots, `vns_budget` runs, and the hypervisor's
  cap fixtures calibrated to ~25M. A speedup is a **versioned build
  change with a re-baseline bill**; the bill's owner doesn't exist yet.
- Open architectural seam: review finding A1 (frames-vs-vns) disputes
  exactly the quantity a speedup perturbs. Optimizing before A1
  settles risks doing it twice.

## Protection A Profiling Pass Enjoys

Measurement is determinism-neutral: a bench harness and attribution
instrumentation (behind a non-default feature or bench profile) retire
zero extra instructions on the shipped path, touch no gate, and
invalidate no artifact. The strong existing gates (per-opcode tests,
hash-pinned test-ROM suites, the cross-arch double-runs) then serve as
the proof harness: re-run green ⇒ the profiling work changed nothing.
