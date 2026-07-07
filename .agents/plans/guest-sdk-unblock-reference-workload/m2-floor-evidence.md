# M2 Floor Evidence

RW-0 evidence note for bead `refwork-d7t.1`, recorded during Ralph iteration 1.

Clean-room boundary: this note records command results, hashes, revisions, and
artifact pointers only. It does not include game content, ROM bytes, framebuffer
goldens, WRAM dumps, or padlog semantics.

## Scope Verdict

The repo-side synthetic M2 floor is present and green on this checkout. The
operator-game first-room floor is not present in this checkout, and no durable
operator-approved waiver is currently recorded. Packages 05 and 06 must continue
to treat real operator-game/in-VM evidence as blocked until lab artifacts or an
operator-approved waiver are supplied.

## Local Run Context

| Field | Value |
|---|---|
| Date | 2026-06-21T18:51:50Z |
| Owner | Matt Spurlin (`refwork-d7t.1` owner); recorded by Codex during `/ralph` |
| Machine | `infra-control` |
| Architecture | `x86_64` |
| Branch | `ralph/iteration-1-record-proto-and-m2-floor-evidence` |
| Checked source rev | `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63` |
| Evidence note introduced by | `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1` |
| Rust toolchain | `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-unknown-linux-gnu` |

## `determinism-proto` Source

| Field | Value |
|---|---|
| Source | sibling checkout `../control-plane` |
| Remote | `git@github.com:preestablished/control-plane.git` |
| Branch | `main` |
| Rev | `ca9ee9048d7fca8eec5fe512011b011128e2b0c3` |
| Worktree state | clean (`git -C ../control-plane status --short` produced no output) |
| Build check | `cargo build --locked --manifest-path ../control-plane/Cargo.toml -p determinism-proto --all-features` |
| Result | PASS |

## Local Synthetic Gates

| Command | Result | Notes |
|---|---|---|
| `cargo build --locked --manifest-path ../control-plane/Cargo.toml -p determinism-proto --all-features` | PASS | Built `determinism-proto` from sibling checkout. |
| `cargo fmt --all -- --check` | PASS | No formatting changes required. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | Workspace clippy passed. |
| `cargo test --workspace --locked` | PASS | Includes `refwork-verify` 10k double-run integration test; that test completed in 490.12s. |
| `cargo run --locked -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml` | PASS | Validates the current placeholder feature map and scoring file syntactically. |
| `cargo run --locked -p xtask -- deny` | PASS | `deny: OK -- no banned tokens found.` |
| `cargo test --release --locked -p xtask --test determinism -- --include-ignored` | PASS | `determinism_600_frames` and `determinism_10000_frames` passed; command completed in 76.88s. |
| `cargo test --release --locked -p xtask --test zero_alloc` | PASS | `zero_alloc_per_frame_after_warmup` passed. |
| `cargo run --locked --release -p xtask -- hash-chain --frames 10000` | PASS | `arch=x86_64`, chain `2f785fa912c08408b994c9e06fb7acba155abe7eb5b2504767bfe4175e1fc4af`. |
| `cargo run --locked --release -p xtask -- cpu-tests` | SKIPPED | Optional local corpus gate; `target/test-roms` is not present in this checkout. |
| `cargo run --locked --release -p xtask -- spc-tests` | SKIPPED | Optional local corpus gate; `target/test-roms` is not present in this checkout. |

## Cross-Architecture Synthetic Hash Evidence

Latest successful nightly synthetic cross-arch evidence was downloaded through
`gh run download` from:

- Workflow run: `nightly` run `27900976973`
- Run URL: `https://github.com/preestablished/reference-workload/actions/runs/27900976973`
- Event/date: scheduled run, 2026-06-21T10:07:23Z
- Head SHA: `9afaa0a69a3ea57ed4e10ff29a53b716b5559990`
- Jobs: `deep-determinism (ubuntu-latest)`,
  `deep-determinism (ubuntu-24.04-arm)`, and `cross-arch-100k`
- Local artifact download path used for this note:
  `/tmp/refwork-hash-evidence-27900976973/`

| Artifact | Runner | Architecture | Frames | Hash |
|---|---|---|---:|---|
| `hash-100k-ubuntu-latest/hash-x86_64.txt` | GitHub Actions `ubuntu-latest` | `x86_64` | 100000 | `f90055376352e1cc46104d3b575232574dad4ebb6694f77f41a1dcdf8bb793f1` |
| `hash-100k-ubuntu-24.04-arm/hash-aarch64.txt` | GitHub Actions `ubuntu-24.04-arm` | `aarch64` | 100000 | `f90055376352e1cc46104d3b575232574dad4ebb6694f77f41a1dcdf8bb793f1` |

The 100k synthetic hashes are equal across x86_64 and aarch64. This is CI
synthetic-ROM evidence only; it is not a substitute for the operator-game M2 lab
run on real aarch64 hardware.

## Cross-Architecture Evidence Applicability

The downloaded nightly evidence was produced at
`9afaa0a69a3ea57ed4e10ff29a53b716b5559990`. The checked source rev for this
evidence note is `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63`; the checkpoint that
introduced this note is `34efa457f7ba2a4403bb3e1e9dac89b7baafeda1`.

Applicability check:

```sh
git diff --name-only 9afaa0a69a3ea57ed4e10ff29a53b716b5559990..8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63
```

Result:

```text
.beads/.gitignore
.beads/README.md
.beads/config.yaml
.beads/metadata.json
.gitignore
```

Only bead metadata and `.gitignore` changed between the nightly evidence SHA and
the checked source rev; no crates, Cargo manifests, feature maps, scoring files,
xtask gates, or CI workflow inputs changed. The branch checkpoint adds only
Markdown plan/evidence files. If any source, test, gate, feature-map, scoring,
or workflow input differs in a future branch, rerun the cross-arch hash job at
that branch or base SHA before citing it as RW-0 synthetic evidence.

## Operator-Game Evidence / Waiver

`feature-maps/demo-game.yaml` still begins with:

> PLACEHOLDER FILE — offsets shown here are NOT validated game addresses.

No operator ROM, first-room padlog, lab `m2-run.json`, lab `map-check` report,
golden framebuffer hashes, or real aarch64 operator-game double-run artifacts
are present in this checkout. The phase-2 bring-up log also leaves the M2
operator-game provenance block empty.

### Waiver Status

| Field | Value |
|---|---|
| Status | BLOCKED: no operator-approved waiver is currently recorded. |
| Checked date | 2026-06-21 |
| Checked by | Codex during `/ralph` |
| Bead owner | Matt Spurlin (`refwork-d7t.1` owner) |
| Approval source | None found in this checkout; `.agents/plans/phase-2/bringup-log.md` still has an empty M2 provenance block. |
| Reason | Operator-game lab artifacts are not available in this checkout, and the feature map remains explicitly placeholder/unvalidated. |
| Required approval shape | Operator name/role plus durable approval artifact such as a bead comment, issue comment, or lab note path, cross-linked from the phase-2 bring-up log. |
| Permitted before approval | Synthetic M3 harness/mock-agent work and asset-only M4 preparation may proceed if their own gates pass. |
| Not permitted before approval or lab evidence | Closing RW-0 as full M2 acceptance, package 05, package 06, final M2/M5 lab acceptance, or any claim that operator-game first-room/map/real-aarch64 evidence is complete. |
| Required follow-up | Replace this blocked status with lab artifact pointers for `refwork-verify play`, `refwork-verify map-check`, and `refwork-verify double-run --frames 100000` on x86_64 and real aarch64 hardware, or with a durable operator-approved waiver. |

## Acceptance Mapping

| RW-0 acceptance clause | Evidence |
|---|---|
| `m2-floor-evidence.md` exists and maps acceptance clauses | This file. |
| M2 engine packages and `refwork-verify` are present and build | Workspace clippy/test gates passed; `refwork-verify` integration tests passed. |
| Host-side first-room script and feature-map offset evidence, or explicit waiver | BLOCKED for operator-game M2 acceptance: no lab artifacts and no operator-approved waiver are recorded. Placeholder feature map remains a stop condition for package 05/06. |
| x86_64 and aarch64 deterministic hash evidence | Synthetic-only evidence: local x86_64 10k hash and nightly run `27900976973` matching 100k-frame x86_64/aarch64 hashes on the synthetic ROM. Operator-game host-side 100k x86_64/aarch64 evidence is not recorded here and remains BLOCKED unless replaced by lab artifacts or a durable operator-approved waiver. |
| `determinism-proto` source recorded and buildable | Sibling `../control-plane` provenance and successful build command above. |
| No game content committed | This note records only revisions, command results, hashes, and artifact pointers. |

## 2026-07-07 Extension (plan phase3-m4-first-room-gate-and-m5-stamp, step 06)

Recorded by Claude (coding agent), `main` at `7b0c7b2`. This section
refreshes the synthetic floor at the current rev and records the state of
each item `gaps.md` flagged, per the request's acceptance §5
(`refwork-d7t.1` is to close on evidence, not implication).

### Synthetic floor re-verification at current rev

| Item | Result |
|---|---|
| `cargo test --workspace` | PASS at `7b0c7b2` (after the `refwork-dh-client` mock `build_profile` fix, `34f034d`) |
| `xtask hash-chain --frames 10000` (local, x86_64) | `6d4133144b7f08b9a6ae7fb16241b733beb6f4ca01fc0959b4025b84f4a108a9` |
| Nightly cross-arch 100k (run `28857976642`, 2026-07-07, head `2ea42ad`) | x86_64 == aarch64 == `aed8e9f8fff1c83e254ad9f45769a296733c29e33b0aebdcfe9bafabb74cd94b` |

The 10k chain hash differs from the 2026-06-21 value (`2f785fa9…`) —
expected: the SNES accuracy chain (`84933d9`, `8eff8d9`, `2ea42ad`)
deliberately changed emulation output. Cross-arch equality is what M2
asserts, and it holds at the new behavior on both architectures.

Applicability: `git diff --name-only 2ea42ad..7b0c7b2` touches only
`.agents/` docs, `crates/refwork-dh-client/src/mock.rs` (test mock),
`.github/workflows/vm-gates.yaml`, and `image/guest-sdk.lock` — no
emulator, hash, feature-map, or xtask gate inputs. The nightly evidence
at `2ea42ad` therefore applies to `7b0c7b2`.

### Host-side first-room evidence

Still pending the operator-gated inputs (real feature map + expect
goldens, ROM/padlog hashes) — tracked as the step 01 consolidated
operator ask in `.agents/plans/phase3-m4-first-room-gate-and-m5-stamp/`.
Nearest existing substance: the real-ROM render evidence in
`.agents/plans/snes-rom-black-screen-compat/03-implementation-results.md`
(non-black frames with render counters at multiple checkpoints) and the
first real emulator+game READY on the real worker
(`.agents/requests/phase3-ready-not-emitted-real-worker/04-verification.md`,
2026-07-05). Neither is a scripted first-room `play`/`map-check` run;
this clause stays open pending the lab session (plan step 03), or closes
via the weaker host-side fallback only with an explicit flag in the
closure reason.

### Build-vs-vendor decision record

Proposed resolution for operator confirmation: the kernel/agent artifact
split decision
(`.agents/decisions/2026-07-02-kernel-agent-artifact-split.md`) IS the
build-vs-vendor record in substance — kernel = vendored hash-pinned
guest-sdk artifact with provenance build key; agent + harness = built
from pinned revs; `xtask image build` enforces both pins. If the
operator agrees, a one-line confirmation (bead comment on
`refwork-d7t.1` or a note here) closes this clause; if not, an explicit
waiver with owner + date is required per the Waiver Status table above.

### aarch64 operator-game double-run

Unchanged: synthetic cross-arch evidence is current (table above), but
the operator-game real-aarch64 run remains not-run. Decision requested
from the operator (run it in the lab session, or defer with a recorded
reason + tracking bead) — surfaced in the step 01 consolidated ask.

### Bead status

`refwork-d7t.1` remains BLOCKED pending exactly three operator inputs:
(1) first-room lab artifacts or flagged fallback closure, (2)
build-vs-vendor confirmation or waiver, (3) aarch64 run-or-defer
decision. Everything agent-derivable at the current rev is recorded
above; no further agent work exists on this bead.
