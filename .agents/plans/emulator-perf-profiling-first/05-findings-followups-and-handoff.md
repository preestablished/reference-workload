# Step 05 — Publish Findings, Price Follow-Ups, And Hand Back

## Findings Document

Create a durable document such as `docs/emulator-performance-profile.md`. It
must include these sections:

1. **Method and reproducibility:** revisions, hardware/toolchain/perf event,
   exact public commands, cases/windows, warmup, repetition/noise handling,
   authoritative-versus-attribution build distinction, raw artifact locations,
   and private-intake procedure.
2. **Results and calibration:** raw and normalized wall/instruction tables for
   boot and steady state, per-workload attribution with >=90% coverage, the
   whole-guest row, named residual rows, and unexplained-gap percentage.
3. **Ranked candidates:** evidence-backed candidate, affected symbols/source,
   estimated upper bound and realistic expected win/range, uncertainty, and
   why the profile supports the rank. Do not sum overlapping candidate wins.
4. **Determinism and re-baseline price:** classify each candidate as
   frame-content-preserving or frame-content-changing and icount-preserving or
   icount-changing. For every icount-changing candidate list epoch-hash chains,
   icount/vns snapshots, vns-budget runs, and cap fixtures requiring review or
   regeneration. Name the proposed owner for each bill; do not implement the
   machinery.
5. **Recommendation:** state whether a 6–7x improvement from 90–115 ms to 16.7
   ms/frame is plausible from the measured emulator shares, what combination of
   non-overlapping candidates would be required, and whether work should wait
   for the A1 frames-versus-vns decision.

An “expected win” is not `100% / current share`. Apply Amdahl's law: a candidate
that completely removes fraction `p` has maximum speedup `1/(1-p)`, and any
realistic estimate must state the assumed improvement within that fraction.
Distinguish instruction reduction from wall-time speedup; sampling share alone
does not prove both change equally.

## Versioning Recommendation

Recommend, without implementing, which public identity changes when an
optimization changes instr/frame: at minimum emulator build/revision identity,
workload-image revision, epoch-chain provenance, snapshot compatibility, and
calibrated execution caps. Assign proposed responsibilities across this repo,
determinism-hypervisor, snapshot-store/consumers, and the bridge. Explicitly say
which frame-content assets survive and which absolute-icount/vns assets do not.

## Follow-Up Beads

Treat a candidate as viable only when it has a measurable attributed share, a
defensible change hypothesis, a benchmark acceptance target, and no exact
existing owner. Create one bead per viable candidate after the findings text is
committed at a stable revision. Each description must include:

- findings document section/table citation and measured workload;
- affected subsystem and estimated win/range;
- blast-radius class in both dimensions (frame content and icount);
- required re-baseline/versioning owner or unresolved ownership decision;
- prerequisite relationship to A1 if applicable;
- acceptance benchmark and determinism gates;
- an explicit “do not implement as part of profiling request” boundary.

Create the busy-scene bead if the fallback was used. Avoid duplicate beads:
search all open/closed tracker state in the appropriate repository first and update/link an existing item
when it already owns the exact candidate.

## External Pointer And Resolution

Inspect and post a concise comment on determinism-hypervisor bead `38b6` from
the determinism-hypervisor checkout, containing the
findings path/revision, directly comparable guest/host figures, residual, the
6–7x plausibility answer, and re-baseline warning. Do not close or reprioritize
the upstream bead unless its owner asks.

Append
`.agents/requests/emulator-perf-profiling-first/04-resolution.md` in the handback
shape requested by `03-verification-offer.md`:

- benchmark invocation and case manifest;
- attribution/findings path and coverage numbers;
- guest calibration/residual summary;
- follow-up bead IDs and blast-radius classes;
- `38b6` comment reference;
- final gate and byte-identity evidence;
- skipped/blocked items with reasons.

Do not mark the request resolved until the external comment and bead creation
actually succeed. Verify tracker sync/persistence rather than assuming local
state is durable. Those state-changing actions are authorized by the request,
but capture their returned IDs/comment reference as evidence, then add them to a
second resolution commit after the findings commit.

## Exit Criteria

The findings answer all five requested questions, estimates obey Amdahl's law,
each viable candidate is priced and tracked without implementation, `38b6` has
the pointer, and the request resolution record is complete and privacy-safe.
