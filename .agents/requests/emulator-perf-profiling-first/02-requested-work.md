# Requested Work

## Ordering Rule (Read First)

Single-agent case: **round 1
(`phase3-m4-first-room-gate-and-m5-stamp/`) first, full stop** — it is
the program long pole. This request is for a *second* agent or the
window after. Rev-stability hazard regardless of who executes: **do not
merge this request's commits to `main` between `refwork-gp9`'s image
rebuild and the M5 stamp landing** — profiling commits change
`git_rev` and can force snapshot/stamp regeneration; if a merge in that
window is unavoidable, re-verify image byte-identity. The bench lane
must never require the Intel lab runner or an operator session.

## What We Need (Behavioral)

1. **Bench harness — bespoke and zero-dependency preferred.** A
   repeatable benchmark lane for `refwork-emu`. Dependency rules,
   concretely (there is no written workspace policy; the binding text
   is the `refwork-emu/Cargo.toml` comment): **no new
   `[dependencies]` in `refwork-emu`, ever**; counting instrumentation
   only behind a non-default feature following the existing
   `introspect` pattern ("compiled out of the guest binary");
   dev/bench dependencies allowed in a separate crate **subject to the
   lockfile guard in AC0** — which is why a bespoke harness (no
   criterion, no transitive deps) is the recommended default.
   Workloads: synthetic-ROM boot (`xtask/src/synth_rom.rs` — clean-
   checkout runnable), the real-ROM first room via round-1's scripted
   log (private ROM; documented private-intake steps, not
   clean-checkout), and a busy scene **only if** a scripted log for
   one exists — it won't until round-2's hand-play session, so the
   predefined fallback is: two workloads acceptable, busy-scene filed
   as a follow-up bead citing the round-2 trajectory. Metrics: host
   user-mode instructions and wall time per emulated frame,
   steady-state vs boot separated.
2. **Calibration — answer the question in its own units.** The
   consumer's figure (hypervisor `38b6`: 27.8M instr/frame,
   90–115 ms/frame) is **KVM guest-mode retired instructions for the
   whole guest** (kernel + agent + harness + emulator), measured via
   `PERF_COUNT_HW_INSTRUCTIONS` with exclude_host. Host-process
   profiling is a different denominator on a different build. So:
   measure host user-mode instr/frame on a build profile matching the
   shipped one (`--locked --release`, musl target where practical)
   over the same frame window, reconcile against the guest-mode
   figure, and report the **residual** (guest kernel + harness +
   build delta) as its own attribution row. An unexplained
   host↔guest gap above ~15–20% fails the same way an unattributed
   residual does.
3. **Attribution.** Break the host-lane instr/frame down by
   subsystem: CPU interpreter (dispatch vs addressing vs ALU), APU
   catch-up (SPC700 vs DSP), PPU scanline rendering, bus/`mem_speed`
   clock accounting, everything-else. Method yours (perf counters,
   sampling, bench-only counting feature) — shipped path unchanged
   (AC0/AC3).
4. **The findings doc.** Committed in-repo:
   - attribution table + methodology + the calibration row;
   - ranked optimization candidates, each with estimated win and
     **determinism blast radius** (frame-content-preserving?
     icount-changing? both?) and, for icount-changing ones, the
     re-baseline bill (epoch-hash chains, snapshots, cap fixtures);
   - **the `38b6` answer in `38b6`'s terms**: their deferral needs
     ~6–7× (90–115 ms → 16.7 ms/frame); state whether the attribution
     makes that multiple plausible, and post a pointer comment on the
     bead;
   - versioning recommendation (what an instr/frame-changing build
     bumps; who re-baselines what);
   - sequencing advice vs the open A1 frames-vs-vns seam — including
     "don't optimize yet" if the data says so.
5. **File the follow-up beads, don't do them.** One bead per viable
   candidate, its description carrying the findings-doc citation
   **and its blast-radius class**, so the eventual owner inherits the
   pricing.

## Acceptance Criteria

0. **Shipped-binary byte identity:** the
   `-p refwork-harness --locked --release` musl binary is
   byte-identical before vs after this request's commits — or the
   `Cargo.lock` diff provably touches no package in the harness
   closure. (This is what protects round-1's own byte-identity gate.)
1. Bench lane runs with documented invocation — clean-checkout
   reproducible for the synthetic lanes; documented private-intake
   for the real-ROM lane; wall-time numbers reproducible within
   stated noise bounds across ≥3 runs. Instruction counts, measured
   user-only, should be **exactly identical** across runs (D1–D4: no
   threads/clocks/RNG/floats) — treat any instr-count jitter as a
   finding, not noise.
2. Attribution covers ≥90% of the **host-lane** instr/frame, and the
   host↔guest calibration gap is quantified within ~15–20% (an
   unexplained gap above that fails).
3. Zero behavior change, proven procedurally: run the bench binary
   (built once) against the emulator at the pre-request rev and at
   HEAD — identical user-mode instr/frame; determinism gates re-run
   green at the final commit (host double-run; hash-pinned test-ROM
   suites; the cross-arch gate if runnable — if not, record it
   skipped with the reason and rely on the host gates, stated
   explicitly).
4. Findings doc committed with all five sections; the `38b6` pointer
   comment posted; follow-up beads filed with citations +
   blast-radius classes; no optimization landed.

## Out Of Scope For This Request

- **Any optimization.** Even "obviously safe" ones — the point is a
  priced menu, not a nibble.
- The versioning/re-baseline machinery — designed when an
  optimization is chosen.
- Resolving the A1 frames-vs-vns seam — cited, not settled.
- Round-1/round-2 scopes — untouched; see the ordering rule.
