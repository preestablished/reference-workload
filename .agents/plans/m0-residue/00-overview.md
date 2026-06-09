# M0 Residue — Plan Overview

**Goal:** close the gap between the repo and milestone **M0** of
`~/.agents/projects/determinism/docs/reference-workload/IMPLEMENTATION-PLAN.md`.
Phase 0 delivered only build-passing skeletons; M0's own acceptance criteria
were never met. Phase 1 (M1, the emulator core) is complete — this plan is the
remaining cumulative-doc debt for this repo, and it unblocks M3 (harness ↔
mock-agent protocol work) in Phase 3 and `state-scorer` integration in Phase 4.

## What M0 requires (IMPLEMENTATION-PLAN.md, verified 2026-06-09)

> Scope: cargo workspace per README layout; `refwork-featuremap` crate (serde
> types, validator per API.md §1.3, `schema/feature-map.schema.json` generated
> from the types); `refwork-protocol` crate (`CtlMsg`, postcard round-trip
> tests, `proto_version`); `scoring/demo-game.yaml` + `feature-maps/demo-game.yaml`
> checked in with placeholder offsets and passing validation; CI skeleton
> (fmt, clippy, deny-list gate for `std::thread`/`tokio`/`rand`/float tokens
> in core crates — ARCHITECTURE.md D1–D4).
>
> Accept:
> - `cargo test` green; `refwork-featuremap validate feature-maps/demo-game.yaml`
>   passes and rejects 10 checked-in negative fixtures (bad offset,
>   volatile-in-predicate, etc.).
> - CI deny-gates demonstrably fail a PR that adds `std::thread` to `refwork-emu`.

## Current state (verified against the working tree)

| M0 item | State today |
|---|---|
| Workspace per README layout | ✓ (plus `xtask/`, M1 crates) |
| `refwork-featuremap` serde types + validator + schema file + CLI | ✗ 40-line stub: one `Feature` struct (name/region/offset/width), trivial `validate()`, no serde, no YAML, no CLI, no schema file |
| `refwork-protocol` `CtlMsg` + postcard + `proto_version` | ✗ stub: 3 placeholder variants, `proto_version: u32` (spec says `u16`), no serde/postcard, no tests |
| `feature-maps/demo-game.yaml`, `scoring/demo-game.yaml` | ✗ missing entirely |
| 10 negative fixtures | ✗ missing |
| CI fmt/clippy/deny gates | ✓ exists (M1 work) — needs: deny scan to cover `refwork-protocol`, a deny self-test that proves the `std::thread` failure mode, and a schema-drift check |

## Work packages (one file each)

| File | Package | Depends on |
|---|---|---|
| `01-featuremap-crate.md` | `refwork-featuremap`: types, validator (§1.3 rules 1–7), scoring-program types (§2), cross-file validation, `validate`/`schema` CLI, JSON-Schema generation | — |
| `02-protocol-crate.md` | `refwork-protocol`: §3.1 `CtlMsg`/`FaultCode` exactly, postcard round-trips, size discipline | — |
| `03-demo-yaml-and-fixtures.md` | `feature-maps/demo-game.yaml`, `scoring/demo-game.yaml` (from API.md §1.4/§2.1 verbatim), ≥10 negative fixtures | 01 |
| `04-ci-and-gates.md` | CI wiring: validate step, schema-drift gate, deny-gate scope + self-test | 01–03 |
| `05-acceptance-checklist.md` | Exit checklist mapping every M0 acceptance clause to a command | 01–04 |

Packages 01 and 02 are independent (parallelizable). 03 needs 01's parser to
exist; 04 is last. Estimated total: well under the original 3-day M0 budget —
the YAML content is specified verbatim in API.md.

## Constraints that apply to all packages

- **Clean-room boundary** (repo README): docs in `~/.agents/projects/determinism/`
  + public references only. No commercial console/game names anywhere in the
  repo. The YAML placeholder offsets come from API.md §1.4 — they are
  explicitly placeholders, not validated game data.
- **MAP.md conventions:** `serde` + `serde_yaml` for YAML artifacts, `postcard`
  for wire messages, explicit `schema_version`/`proto_version` everywhere.
  Parsers reject unknown `schema_version` majors and unknown required-context
  fields, ignore unknown optional fields (API.md preamble, normative).
- `refwork-protocol` is compiled into the guest harness binary: it inherits the
  determinism deny-list (no threads/clocks/rand/floats/HashMap) and
  `#![forbid(unsafe_code)]`. `refwork-featuremap` is host-side (floats/maps
  permitted, but none are needed).
- Workflow: implement → `/review` (dual reviewer) → fix → verify → commit, on a
  branch (continue `phase-1/m1-emulator-core` or cut `m0-residue` from it).

## Out of scope (deliberately)

- M2+ work: full APU, raster effects, `refwork-verify`, `ramdiff`, harness
  binary, image pipeline (their plans exist in IMPLEMENTATION-PLAN.md).
- Real feature-map offsets (operator/`ramdiff` work, M2).
- Vendoring any test content; `m0-proto-client`/`determinism-proto` (already
  satisfied by the Phase-0 skeleton and owned by control-plane).
