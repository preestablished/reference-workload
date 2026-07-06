# Reviewed implementation plan: APU port-0 scheduling

This note applies the two requested subagent reviews to `00-plan.md` and
`01-handoff.md`. The accepted implementation path is a narrow, deterministic
APU scheduling fix first, with a separate diagnostic fork for IPL protocol
semantics if the timing fix exposes that as the next blocker.

## Evidence to preserve

- Step 0 already classified the private ROM as in-scope LoROM, ROM-only.
- The existing clean-room diagnostic reproduces the failure: by frame 60 the
  main CPU has written the fixed `$CC` kick (`wr_CC=true`) and is polling APU
  ports heavily, while the SPC remains in the IPL `$CC` poll (`spc_pc=$FFCC`,
  `spc_in_ipl=true`) and the SPC-visible port 0 latch is no longer `$CC`.
- All new diagnostics must stay clean-room-safe: booleans, counts, PC/register
  addresses, timing deltas, structural counts, and hashes only; never ROM bytes,
  framebuffer pixels, APU/DMA payload values, memory contents, or the header
  title.

## Accepted review changes

1. Add one more diagnostic before relying on the fix: after the first CPU write
   of `$CC` to APU port 0, count later port-0 writes and record the master-clock
   delta to the first later port-0 write. Do not log the later value.
2. Implement the scheduling fix as a capped future target, not as additive
   service time. After a CPU write to a port-0 mirror, advance the APU toward
   `mclk_total + WINDOW`, capped by a small maximum lead over `mclk_total`, and
   set `apu_mclk_base` to the target that was serviced. This avoids stacking an
   extra window on every upload strobe.
3. Keep the behavior CPU-access scoped. DMA B-bus APU accesses are a separate
   path and are not part of the diagnosed CPU boot-handshake failure.
4. Share APU halt/fault handling between normal catch-up and the service window.
5. Make the regression bus-level so it covers `apu_catch_up`, `apu_mclk_base`,
   and `$2140-$217F` mirror behavior. Do not rely on direct `Apu::cpu_write_port`
   tests for this bug.
6. Expand `rom_diag` before final acceptance so it prints brightness, BG mode,
   TM, VRAM/OAM/CGRAM counts, a structural metric, and deterministic frame hashes
   as hashes/counts only.
7. Treat real IPL upload semantics as the likely next diagnostic fork, not as a
   silent fallback. If the timing fix gets past `poll_cc` but stalls in transfer,
   update `ipl.rs` only with a public-doc-derived protocol test suite and no
   manufacturer ROM bytes.

## Implementation sequence

1. Add introspect-only fields for post-`$CC` port-0 write count and first delta.
2. Add `SysBus` helpers:
   - `advance_apu_to(target_mclk)` to move the APU to an absolute serviced
     timestamp and update `apu_mclk_base`;
   - a shared APU halt handler used by both `apu_catch_up` and service-window
     advancement;
   - `service_apu_after_cpu_port_write(port)` that applies only to port 0 and
     uses the capped future target.
3. Wire the helper after CPU writes to `$2140-$217F` port 0 mirrors.
4. Add bus-level tests for:
   - rapid post-`$CC` overwrite no longer hides the IPL kick;
   - port-0 mirror writes receive the same service;
   - service-window lead is capped and not double-run by the next catch-up.
5. Expand `rom_diag` output while preserving clean-room limits.
6. Run the private-ROM diagnostic. If the SPC exits IPL, force blank clears,
   brightness is positive, CGRAM/VRAM populate, the structural metric is above a
   small floor, and two runs produce the same late-frame hash, continue to the
   full test suite. If it instead stalls in transfer, stop the timing loop and
   implement the real IPL-protocol fork with synthetic protocol tests.
7. Verify with `cargo test -p refwork-emu --features introspect`, `cargo test -p
   refwork-emu`, the relevant `xtask` determinism tests, and two clean-room
   `rom_diag` runs. Run `linux_m5` if the required sibling-repo artifacts are
   available; otherwise report it as an explicit remaining external gate.

