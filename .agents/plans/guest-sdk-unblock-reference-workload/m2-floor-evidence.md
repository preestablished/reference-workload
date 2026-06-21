# M2 Floor Evidence

RW-0 evidence note for bead `refwork-d7t.1`, recorded during Ralph iteration 1.

Clean-room boundary: this note records command results, hashes, revisions, and
artifact pointers only. It does not include game content, ROM bytes, framebuffer
goldens, WRAM dumps, or padlog semantics.

## Scope Verdict

The repo-side synthetic M2 floor is present and green on this checkout. The
operator-game first-room floor is not present in this checkout and is covered by
the narrow waiver below; packages 05 and 06 must continue to treat real
operator-game/in-VM evidence as blocked until lab artifacts are supplied.

## Local Run Context

| Field | Value |
|---|---|
| Date | 2026-06-21T18:51:50Z |
| Owner | Matt Spurlin (`refwork-d7t.1` owner); recorded by Codex during `/ralph` |
| Machine | `infra-control` |
| Architecture | `x86_64` |
| Branch | `ralph/iteration-1-record-proto-and-m2-floor-evidence` |
| Starting repo rev | `8c21d5d3fc76c2ea16ab3f76ea168218b8ac4c63` |
| Rust toolchain | `rustc 1.96.0 (ac68faa20 2026-05-25)`, host `x86_64-unknown-linux-gnu` |

## `determinism-proto` Source

| Field | Value |
|---|---|
| Source | sibling checkout `../control-plane` |
| Remote | `git@github.com:preestablished/control-plane.git` |
| Branch | `main` |
| Rev | `ca9ee9048d7fca8eec5fe512011b011128e2b0c3` |
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

## Operator-Game Evidence / Waiver

`feature-maps/demo-game.yaml` still begins with:

> PLACEHOLDER FILE -- offsets shown here are NOT validated game addresses.

No operator ROM, first-room padlog, lab `m2-run.json`, lab `map-check` report,
golden framebuffer hashes, or real aarch64 operator-game double-run artifacts
are present in this checkout. The phase-2 bring-up log also leaves the M2
operator-game provenance block empty.

### Waiver

| Field | Value |
|---|---|
| Date | 2026-06-21 |
| Owner | Matt Spurlin (`refwork-d7t.1` owner); recorded by Codex during `/ralph` |
| Reason | Operator-game lab artifacts are not available in this checkout, and the feature map remains explicitly placeholder/unvalidated. |
| Scope | Waives only the requirement to attach host-side operator-game first-room, map-check, and real-hardware aarch64 demo-game evidence before starting synthetic M3 harness/mock-agent and M4 image-handoff preparation. |
| Non-scope | Does not waive synthetic protocol/hash gates, `determinism-proto` provenance, image reproducibility, in-VM first-room readiness, package 05, package 06, or final M2/M5 lab acceptance. |
| Required follow-up | Before package 05/06 closure, replace this waiver with lab artifact pointers for `refwork-verify play`, `refwork-verify map-check`, and `refwork-verify double-run --frames 100000` on x86_64 and real aarch64 hardware. |

## Acceptance Mapping

| RW-0 acceptance clause | Evidence |
|---|---|
| `m2-floor-evidence.md` exists and maps acceptance clauses | This file. |
| M2 engine packages and `refwork-verify` are present and build | Workspace clippy/test gates passed; `refwork-verify` integration tests passed. |
| Host-side first-room script and feature-map offset evidence, or explicit waiver | Narrow waiver recorded above; placeholder feature map remains a stop condition for package 05/06. |
| x86_64 and aarch64 deterministic hash evidence | Local x86_64 10k hash plus latest nightly x86_64/aarch64 100k synthetic artifact hashes above. |
| `determinism-proto` source recorded and buildable | Sibling `../control-plane` provenance and successful build command above. |
| No game content committed | This note records only revisions, command results, hashes, and artifact pointers. |
