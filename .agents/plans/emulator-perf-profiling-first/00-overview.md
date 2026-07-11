# Plan: Profile The Emulator Without Changing Shipped Behavior

Plan prepared 2026-07-11 for
`.agents/requests/emulator-perf-profiling-first/`. It is written for a coding
agent working in this repository. Read the complete request before starting;
the request's `04-current-status-2026-07-10.md` supersedes its original
ordering warning: the work is now ungated, while every measurement,
determinism, privacy, and zero-behavior-change constraint remains binding.

## Outcome

Land a repeatable `refwork-emu` benchmark lane, measured boot and steady-state
results for the synthetic ROM and the private first-room workload, at least 90%
host-process instruction attribution, a reconciled host-versus-KVM-guest
calibration, and a findings document that prices follow-up optimizations without
implementing any of them.

The authoritative measurements come from the uninstrumented release binary.
Symbolized builds and a non-default profiling feature may explain that number,
but their counts must be reported separately and must never substitute for the
shipped-like lane.

## Deliverables And Order

| Step | File | Primary deliverable |
|---|---|---|
| 1 | `01-preflight-and-guardrails.md` | Frozen baseline, tool/environment record, workload inventory, byte-identity baseline |
| 2 | `02-benchmark-harness.md` | Dependency-free benchmark executable and clean-checkout synthetic lane |
| 3 | `03-host-measurement-and-attribution.md` | Repeated wall/instruction measurements and >=90% subsystem attribution |
| 4 | `04-private-workload-and-guest-calibration.md` | First-room private run and host↔guest residual reconciliation |
| 5 | `05-findings-followups-and-handoff.md` | Findings, priced beads, `38b6` pointer, request resolution |
| 6 | `06-verification-matrix.md` | Before/after proof, determinism gates, lockfile and binary audit |

Implement steps 1–3 with the synthetic ROM before requesting private inputs.
Steps 4 and 5 require the existing operator/private-lab channel; do not invent,
copy, or commit private paths, ROM data, pad semantics, framebuffers, or memory
dumps. Step 6 runs throughout and is finalized only after all code/docs settle.

## Non-Negotiable Boundaries

- No optimization, refactor-for-speed, altered inline policy, or emulator timing
  change. Profiling must describe the current implementation.
- Add no runtime dependency to `refwork-emu`. Prefer a dedicated workspace
  benchmark crate with path dependencies on `refwork-emu`, `refwork-hash`, and
  (for padlog parsing) `refwork-script`; these reuse repository contracts and
  existing locked packages. Do not add Criterion or a new transitive dependency.
- Any counter hooks inside `refwork-emu` live behind a new non-default feature,
  parallel to `introspect`, and compile completely out when disabled. Do not
  enable that feature from `refwork-harness` or a workspace-wide shared
  dependency declaration.
- Do not add frame pointers, `inline(never)`, logging, timers, or counters to the
  authoritative build. A release-with-debuginfo build is acceptable for sampling
  only after confirming its `.text` matches the authoritative build or clearly
  labeling it a separate attribution build.
- The bench lane is host-only. It must not require KVM, the Intel lab runner, a
  deployed worker, or an operator session for its synthetic workload.
- Never claim the unrelated Phase 3 epic is closed; `refwork-d7t.1` remains a
  separate evidence issue according to the request status.

## Measurement Model

Keep these denominators visibly separate in code, output, and findings:

| Lane | Includes | Purpose |
|---|---|---|
| host authoritative | benchmark process in user mode, uninstrumented release build | exact instructions/frame and wall time/frame |
| host attribution | same workload/window, symbolized sampling and optional compiled-out counters | split host authoritative number by subsystem |
| KVM guest | guest kernel + agent + harness + emulator, `exclude_host` | compare with upstream `38b6`'s 27.8M and 90–115 ms/frame |
| residual | KVM guest minus comparable host emulator lane, adjusted for build/window differences | kernel/agent/harness/build delta, not “emulator other” |

Use a fixed frame window and input schedule per workload. Boot and steady state
are distinct cases, not two averages blended together. Report raw totals as
well as per-frame normalization so reviewers can recompute every percentage.

## Completion Gate

This request is complete only when all acceptance criteria in
`02-requested-work.md` are evidenced, the findings doc and
`.agents/requests/emulator-perf-profiling-first/04-resolution.md` are committed,
the viable-candidate beads exist but contain no implementation, and the
hypervisor `38b6` pointer comment has been posted. If private or KVM calibration
cannot be run, land neither a fabricated residual nor a completion claim:
record the exact missing input and leave the request open.
