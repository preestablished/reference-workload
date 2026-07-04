# Implementation Status (2026-07-04)

Implemented the same day the plan was filed; full evidence in the
request's `03-resolution.md`.

| Step | Status |
|---|---|
| 01 breadcrumbs + drop-counter | **Done** (breadcrumbs merged, guest-sdk `678dc81`; probe run recorded; pair-count check confirmed the wedge-window reasoning — 6 pairs on a good boot, real worker showed 3). Drop-counter check in the dump: ask the bridge with the re-run. |
| 02 socket lifetime fix | **Done** (`678dc81`), negative-tested at unit and VM tier. Probe holds Ready to deadline. |
| 03 Ready-emission fix | **Pending the bridge re-run** — that run either succeeds (symptom 1 gone) or produces a dump whose last breadcrumb selects the pre-written hypothesis branch. Wedge-to-fault hardening (both poll caps re-sized to fault inside a 10 B-instruction budget) landed in `678dc81`. |
| 04 VM-tier held-Ready test | **Done** (`914dbde`, `refwork_ready_hold.rs`), verified failing against a `322c331` agent. |
| 05 adoption + handback | **Done** — lock at `914dbde`, image rebuilt (initramfs sha256 `fc64b3d4…`), `03-resolution.md` filed. Awaiting the bridge's `04-verification.md`. |

Corrections discovered while implementing:

- `02-repro.md`'s "any 32 KiB blob" is stale: the harness faults
  `BadResetVector` on a zero game. Use a NOP ROM (0xEA fill, reset
  vector 0x8000). The plan's step-01 §3 command block inherits this
  correction.
- READY icount shifts slightly (added pre-Ready breadcrumb work) — noted
  to the bridge for the snapshot regeneration.
