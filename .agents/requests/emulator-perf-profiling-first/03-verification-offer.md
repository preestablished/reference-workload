# Consumers And Handback

## Who Consumes The Findings

- **determinism-hypervisor `38b6`** (epoch-hash M4): its deferral note
  says "needs reference-workload emulator speedup" — the findings doc
  tells them whether that speedup is plausible, how big, and at what
  re-baseline cost. Drop a pointer note in their tracker (a comment on
  `38b6`) when the doc lands.
- **rom-operator-bridge** (interactive-play pacing, `pea`'s bandwidth
  decision context): the fps ceiling derives from instr/frame; the
  attribution tells them whether the ceiling moves this year.
- **The phases track**: the versioning recommendation is the input to
  the eventual ownership decision (who lands an icount-changing build,
  who pays the re-baseline).

## Phases-Track Verification

1. Re-run the bench lane from a clean checkout; check reproducibility
   against the stated noise bounds.
2. Audit the attribution table's coverage claim (≥90%) against the
   methodology.
3. Re-run the determinism gates at the final commit and diff the
   shipped-path instr/frame before/after — the zero-behavior-change
   proof is the acceptance's heart.

## Handback Shape

Append `04-resolution.md` (bench invocation, attribution table,
findings-doc path, follow-up bead list, gate-rerun evidence); we
respond with `05-verification.md`.

## Contact / Tracking

- Upstream measurement provenance: hypervisor `38b6` notes
  (27.8M instr/frame, ~90–115 ms/frame), their
  `08-followup-frame-hard-cap.md` (~25M calibration).
- Sibling open requests in this repo: round-1 (Phase-3 gates, the
  priority), round-2 (corpus, gated) — this request is parallel,
  lab-session-free, and yields to both.
- Open seam cited: review finding A1 (frames-vs-vns), in this repo's
  `reviews/review2-a-technical.md`.
