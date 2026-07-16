# Critical And Important Issues

## Critical

None.

## Important

### Important: Waiver lacks durable operator approval provenance

Path: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:89`

Description: The note records a waiver with date, owner, reason, scope, non-scope, and follow-up, but it does not cite a durable approval source showing that the named owner actually granted the waiver. The existing phase-2 bring-up log still has an empty provenance block and no operator sign-off. Because RW-0 acceptance allows host-side first-room and feature-map evidence to be replaced only by an explicit operator waiver, this currently risks converting "Codex recorded that a waiver exists" into acceptance evidence.

Suggested fix:

```md
### Waiver

No operator waiver is currently recorded. The phase-2 bring-up log provenance
block is empty, so host-side operator-game first-room, map-check, and real
aarch64 evidence remain BLOCKED for M2 acceptance.

<!-- Replace the block above only after durable approval exists. -->
| Field | Value |
|---|---|
| Date | 2026-06-21 |
| Owner | <approving operator name> |
| Approval source | `.agents/plans/phase-2/bringup-log.md:<line>` or `<lab-note-path>` with report/hash |
| Reason | Operator-game lab artifacts are not available in this checkout, and the feature map remains explicitly placeholder/unvalidated. |
| Scope | Waives only starting synthetic M3 harness/mock-agent and M4 image-handoff preparation before attaching host-side operator-game first-room, map-check, and real-hardware aarch64 demo-game evidence. |
| Non-scope | Does not waive synthetic protocol/hash gates, `determinism-proto` provenance, image reproducibility, in-VM first-room readiness, package 05, package 06, or final M2/M5 lab acceptance. |
```

### Important: Acceptance mapping overstates synthetic cross-arch evidence

Path: `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md:107`

Description: The note correctly says the downloaded x86_64/aarch64 hashes are CI synthetic-ROM evidence and "not a substitute for the operator-game M2 lab run on real aarch64 hardware" at lines 74-76. The acceptance mapping then lists those same synthetic hashes under "x86_64 and aarch64 deterministic hash evidence" without carrying that limitation forward. Future Ralph iterations could read the table as closing host-side M2 cross-arch evidence even though operator-game host-side 100k evidence is still absent or needs a proven waiver.

Suggested fix:

```md
| RW-0 acceptance clause | Evidence |
|---|---|
| x86_64 and aarch64 deterministic hash evidence | Synthetic-only CI evidence: nightly run `27900976973` produced matching 100k-frame hashes for x86_64 and aarch64 on the synthetic ROM. Operator-game host-side 100k x86_64/aarch64 evidence is not recorded here and remains BLOCKED or WAIVED only if the waiver above has a durable approval source. |
```
