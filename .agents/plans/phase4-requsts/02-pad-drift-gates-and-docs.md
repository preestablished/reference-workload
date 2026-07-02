# 02 - Pad Drift Gates And Docs

**Purpose:** make the pad table a durable source of truth across source code,
generated manifests, script docs, and project docs.

## Canonical Table

The canonical table is:

| Button | Bit |
|---|---:|
| A | 0 |
| B | 1 |
| X | 2 |
| Y | 3 |
| L | 4 |
| R | 5 |
| Up | 6 |
| Down | 7 |
| Left | 8 |
| Right | 9 |
| Start | 10 |
| Select | 11 |

Bits 12, 13, 14, and 15 are reserved and must be zero in every pad word.

The casing is part of the contract. `UP`, `DOWN`, `LEFT`, `RIGHT`, `START`,
and `SELECT` are not aliases.

## Implementation Steps

1. Update `crates/refwork-script/FORMAT.md` to name the stable layout id:

   - layout id: `console16-12btn-v1`
   - layout version: `1`
   - exact mixed-case button list
   - reserved-bit rule

2. Update `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md`
   in both relevant places:

   - section 3.4 input bit table: state that the table is named
     `console16-12btn-v1`.
   - section 4 WorkloadImage manifest: include `pad_layout.layout_id` and state that
     consumers must compare names exactly.

3. Update `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/INTEGRATION.md`
   where it describes input delivery. It should say input-synthesizer's demo pad
   config cites `pad_layout.layout_id` and resolves button names exactly.

4. Keep `xtask/src/image.rs` as the generator source for the WorkloadImage
   inline table. Do not create a second unchecked source of truth.

5. Extend test coverage so drift is caught before image handoff:

   - wrong `layout_id` fails;
   - wrong mixed-case name fails;
   - wrong bit fails;
   - wrong reserved-bit list fails.

6. If a small machine-readable table outside the generated WorkloadImage is
   needed for consumers, add it only if it is generated or validated from the
   same `PAD_BUTTONS`/`PAD_LAYOUT_ID` constants. Do not hand-maintain a second
   YAML table that can drift.

## Suggested Validation Commands

```sh
cargo test --locked -p xtask
cargo run --locked -p xtask -- image validate <manifest>
```

Run the second command only after an implementation package has generated a
fresh image bundle. Pass one explicit manifest path. If validating multiple
bundles, use a shell loop rather than relying on a glob that expands to multiple
arguments.

## Acceptance

- Source and docs agree on `console16-12btn-v1`, layout version `1`, exact
  mixed-case names, bit positions, and reserved bits.
- The WorkloadImage manifest is the canonical machine-readable artifact for
  consumers.
- `refwork-script` docs state that `.padlog` words use the same layout and that
  bits 12-15 are parse errors when set.
- No code path accepts all-caps names or performs case normalization for the
  reference-workload pad layout.

## Stop Conditions

- If updating project docs under `/home/infra-admin/.agents/projects/determinism/`
  is outside the implementation agent's write scope, record the exact patch needed
  in the closeout notes and do not claim the docs acceptance item is complete.
- If a drift test parses markdown prose, replace it with a source or manifest
  validation test. Markdown wording should not be a runtime dependency.
