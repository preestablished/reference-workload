# Request: Profile The Emulator — Zero Behavior Change — So The Unowned Speedup Gets An Owner And A Price

> **CURRENT STATUS (2026-07-10):** Open and now startable by the primary agent.
> Read `04-current-status-2026-07-10.md` for the ordering update.

## Who Is Asking

The phases track, round 3 (2026-07-07), on behalf of the three consumers
who keep citing a speedup nobody owns: determinism-hypervisor bead
`38b6` (epoch-hash M4, "measured-and-deferred … needs reference-workload
emulator speedup"), the bridge's interactive-play tier (8.5 fps today,
60 unreachable), and any future watchable-play ambition. This is
explicitly **not** an optimization request — the assessment that shaped
it found optimization would be premature three ways over.

## Why Profiling-First, Not Speedup

1. **No emulator-subsystem attribution exists.** The hypervisor has
   wall-time-level measurements (their
   `play-60fps-decouple-hash-from-frames/05-measurements.md`: guest
   execution ~90–115 ms/frame dominant, epoch links ~50 ms, RPC
   overhead) — but *inside the emulator* there is nothing: zero
   benchmarks, zero profiling artifacts, no attribution across the CPU
   interpreter vs APU catch-up vs PPU rendering vs per-access clock
   accounting. The 25–28M instr/frame figure is a guest-mode icount
   byproduct of test-cap calibration. You cannot scope an optimization
   you cannot see.
2. **A speedup is determinism-sensitive in a non-obvious way.** icount
   is host guest-mode retired instructions; virtual time is a pure
   function of it. Fewer instructions/frame ⇒ epoch boundaries land at
   different points ⇒ **recorded epoch-hash chains and
   icount-addressed snapshots stop verifying against the new build.**
   Frame-content hashes and `at_frame` input scheduling survive a
   semantics-preserving speedup (the D17 frame-budget decision made
   frame-quantized stops the platform's contract, which is what
   insulates input alignment); absolute-icount artifacts do not. The
   optimization therefore needs a versioning/re-baseline decision —
   who bumps what, who re-records which artifacts — that doesn't
   exist yet. Optimizing before that bill has an owner means paying
   it by surprise.
3. **No milestone requires it.** The repo plan (M0–M6) has no perf
   milestone; Phase 7/8's watchable output is *encode* throughput over
   a recorded stream, not live emulator fps. The pressure is real but
   lives in review commentary — which is exactly what makes profiling
   (cheap, determinism-neutral, landable now) the right first
   commitment and an optimization (unowned, gate-less,
   baseline-invalidating) the wrong one.

## The Ask In One Paragraph

Build the missing measurement layer — a benchmark harness over
`refwork-emu` (bench profile, representative workloads: boot, first
room, busy scene) and per-subsystem attribution of the ~25–28M
instructions/frame (CPU interpreter dispatch, APU/SPC700 catch-up, DSP,
PPU scanline rendering, bus `mem_speed` clock accounting) — then write
the findings doc: ranked optimization candidates, each priced with its
expected win *and its determinism blast radius* (frame-content
preserving? icount-changing? both?), the re-baseline cost an
icount-changing optimization triggers, and a recommendation on who
should own that re-baseline before any optimization is chosen. **Land
no behavior change**: same instructions retired per frame on the
shipped path, all determinism gates untouched and re-run green as
proof.

## Files In This Request

| File | Contents |
|---|---|
| `01-current-state.md` | Evidence: no perf data, the architecture, the icount coupling |
| `02-requested-work.md` | The ask, acceptance criteria, out of scope |
| `03-verification-offer.md` | Consumers of the findings; handback |
