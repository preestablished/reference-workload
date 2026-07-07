# Requested Work

## What We Need (Behavioral)

The chain your `phase3-m4-first-room-unblock` plan already sequences,
driven to done:

1. **`refwork-gp9` — rebuild + READY regen.** Rebuild the package-04 image
   from current `main` (post-`40eaf4f`, post-emulator-accuracy fixes),
   regenerate the READY snapshot under a locally-launched real worker, and
   record the new snapshot ref + image hashes. Surface the operator-input
   needs (ROM BLAKE3, padlog BLAKE3, run owner) as a single consolidated
   ask the moment you know them — don't let them trickle out one at a time.
2. **`refwork-d7t.11` — first-room in-VM.** Run `refwork-verify
   vm-first-room` end-to-end against the real worker: RestoreSnapshot →
   InjectInputs (the scripted first-room log) → Run → GetFramebuffer shows
   the room. This is Phase 3 exit gate 3 verbatim.
3. **`refwork-d7t.12/.13/.14` — the M5 stamp.** `refwork-verify vm-suite`
   against the real image: boot→N frames with a fixed log twice →
   per-frame RAM+framebuffer hashes identical; snapshot mid-game → restore
   → continue → identical to the uninterrupted run; **20 consecutive runs,
   zero flakes, both legs, on the Intel lab runner**; the `--nondet-test`
   negative proves the suite can fail. Replace
   `determinism.unstamped.yaml` with the green stamp carrying run metadata
   (revs, image hash, host, date; `.12`'s operator lab fields included).
4. **`refwork-d7t.15` — closeout.** Real-worker legs added to the existing
   `vm-gates.yaml`, guest-sdk handoff updated so their
   `guest-sdk-ext-refwork-m5-full-suite` external bead can close.
5. **M2 paper trail (small, don't skip) — resolve `refwork-d7t.1`.** The
   bead (P1, currently blocked, and blocking the `refwork-d7t` epic) is
   the tracked home for exactly what `gaps.md` said was missing. Extend
   `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md`
   with the host-side first-room evidence pointer and the build-vs-vendor
   decision record (or a recorded operator waiver), address or explicitly
   defer the known open question (M2's cross-arch aarch64 double-run),
   then unblock/close the bead. If some M2 substance genuinely remains,
   say so there instead.

## Suggested Sequencing (Yours To Overrule)

First, five minutes of bead hygiene: the sequencing edges `.13→.12(.11)`,
`.14→.13`, `.15→.14` don't exist yet in `bd dep tree` — add them so the
graph matches this chain. The branch question your plan flagged is moot
(all three branches are ancestors of `main`; build from `main`). Then:
1 → 2 are one lab session with us on the cutover (see `03-`). 3 follows
immediately on the same snapshot. 4 and 5 are cleanup that can interleave.

## Acceptance Criteria

1. New READY snapshot ref recorded; image double-build byte-identity
   re-verified at the new rev.
2. First-room evidence: the verifier's output artifact (frame hash +
   framebuffer capture reference) filed under the plan's evidence
   discipline, plus our browser-side confirmation (see `03-`).
3. The M5 green stamp exists in `dist/` with 20/20 clean runs (both legs,
   Intel runner) recorded and the negative test demonstrated;
   `refwork-d7t.12/.13/.14` closed with evidence pointers. Per the
   IMPLEMENTATION-PLAN's own M5 bar: `xtask image --register` refuses
   without a fresh green stamp, and the manifest's `determinism.last_green`
   is populated — include both, or explicitly defer them to `.15` with a
   recorded reason.
4. CI shows the real-worker legs (or a recorded reason they must stay
   lab-manual); guest-sdk notified via their handoff file.
5. `refwork-d7t.1` closed (not just "a record exists") with the extended
   `m2-floor-evidence.md` as its evidence.

## Out Of Scope For This Request

- **M6 scoring/goal integration** — Phase 4 work, gated on scorer M2–M3
  (`phase-4-scoring-and-inputs.md`); the Phase 4 corpus/golden-artifact
  project requests stay tracked where they are. This request only clears
  the real-capture floor they're waiting on.
- Emulator performance work (the hypervisor's 60fps measurement note) —
  separate conversation, not a Phase 3 gate.
- The hypervisor-side test-cap retune — filed against that repo.
