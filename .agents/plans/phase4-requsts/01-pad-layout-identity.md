# 01 - Pad Layout Identity

**Purpose:** give downstream services a stable identifier for the canonical demo
pad alphabet while preserving the current mixed-case button names and bit
assignments.

## Recommended Decision

Add `pad_layout.layout_id: console16-12btn-v1` to the generated WorkloadImage
manifest and document that reference-workload owns both:

- the stable layout id `console16-12btn-v1`;
- the exact table of button names, bits, and reserved bits.

This matches the input-synthesizer request and the existing reference-workload
docs, which already mention `console16-12btn-v1` as the demo config name. If the
owner rejects this, close the request with an explicit decision record stating
that reference-workload owns only table equality and input-synthesizer owns the
name. Do not leave ownership implicit.

## Implementation Steps

1. Add a source constant near `PAD_BUTTONS` in `xtask/src/image.rs`:

   ```rust
   const PAD_LAYOUT_ID: &str = "console16-12btn-v1";
   ```

2. Update `write_workload_manifest` in `xtask/src/image.rs` so the generated
   manifest includes:

   ```yaml
   pad_layout:
     layout_id: console16-12btn-v1
     layout_version: 1
     buttons:
       - { name: A, bit: 0 }
       - { name: B, bit: 1 }
       - { name: X, bit: 2 }
       - { name: Y, bit: 3 }
       - { name: L, bit: 4 }
       - { name: R, bit: 5 }
       - { name: Up, bit: 6 }
       - { name: Down, bit: 7 }
       - { name: Left, bit: 8 }
       - { name: Right, bit: 9 }
       - { name: Start, bit: 10 }
       - { name: Select, bit: 11 }
     reserved_bits: [12, 13, 14, 15]
   ```

3. Update `validate_pad_layout` in `xtask/src/image.rs` to require the exact
   `layout_id`. Missing, empty, or different values must fail validation.

4. Add tests beside `validator_rejects_pad_layout_drift`:

   - `validator_rejects_pad_layout_id_drift`
   - `validator_rejects_missing_pad_layout_id`
   - `generated_manifest_contains_pad_layout_id`

5. Update the WorkloadImage manifest example in
   `/home/infra-admin/.agents/projects/determinism/docs/reference-workload/API.md`
   so it includes the `layout_id` field.

6. Update any in-repo generated example, README, or evidence note that quotes the
   manifest pad block. Do not update generated `dist/` output unless the repo
   already tracks it.

## Acceptance

- `cargo test -p xtask` passes.
- `cargo run --locked -p xtask -- image validate <manifest>` rejects a manifest
  whose `pad_layout.layout_id` is missing or not `console16-12btn-v1`.
- A generated manifest contains the id and the existing mixed-case table.
- Reserved bits remain `[12, 13, 14, 15]`.
- No alias, normalization, or all-caps button acceptance is added.

## Stop Conditions

- If adding `layout_id` would break an already-published consumer contract, stop
  and write the explicit ownership decision instead of silently changing the
  contract.
- If a consumer asks for all-caps names, do not add aliases in this repo. Record
  that consumers must update examples or reject invalid packs.
