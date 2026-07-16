# Overview

Branch: `ralph/iteration-1-record-proto-and-m2-floor-evidence`

Date: 2026-06-21

Reviewer: Claude Opus (2nd reviewer)

Summary: This branch adds a detailed reference-workload execution plan plus an RW-0 M2 floor evidence note. The plan mostly preserves the clean-room boundary and correctly keeps real operator-game/in-VM proof out of the repo, but the gate evidence is not yet durable enough: the waiver is recorded as a Codex/owner note rather than an explicitly operator-approved waiver in the phase-2 bring-up record, the cross-arch evidence is from a different SHA without an applicability note, and the RW-2/package-04 dependency language can be read as closing image-handoff without the hypervisor Linux floor that the upstream graph says it depends on.

Verdict: REQUEST_CHANGES

Stats:

| Metric | Value |
|---|---:|
| Files changed | 9 |
| Lines added | 1011 |
| Lines removed | 0 |
| Commits | 1 |

Reviewed commit:

- `34efa45` - `ralph: iteration 1 checkpoint - record M2 floor evidence`
