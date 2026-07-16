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

At the original 2026-06-21 audit, no operator ROM, first-room padlog, lab
`m2-run.json`, lab `map-check` report, golden framebuffer hashes, or real
aarch64 operator-game double-run artifacts were present in this checkout. The
2026-07-07 and 2026-07-11 extensions below supersede that original gate state.

### Waiver Status

| Field | Value |
|---|---|
| Status | Historical 2026-06-21 state; superseded by direct evidence and operator confirmation recorded below. No waiver was used. |
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
| Host-side first-room script and feature-map offset evidence, or explicit waiver | SATISFIED by the 2026-07-07 host-side first-room evidence and `m5-suite-evidence.md` pointers below; no waiver used. |
| x86_64 and aarch64 deterministic hash evidence | SATISFIED by synthetic nightly equality plus the 2026-07-11 operator-game native x86_64/aarch64 100k equality record below. |
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

### 2026-07-07 (later): host-side first-room clause SATISFIED

The operator supplied the game image the same day; the host-side
first-room evidence now exists on the real ROM:

- `refwork-verify play` drove the scripted log host-side (title → 1P
  GAME → Stage 1 "TREETOPS" gameplay); the room feature was discovered
  with `ramdiff` record/search/watch (marked title/menu/stage-card/
  gameplay session; monotone 0→48→167 trajectory, stable in-stage).
- The same padlog ran in-VM through the worker gRPC path
  (`vm-first-room` validating run PASS), and the host-side framebuffer
  dumps at frames 3400/4200 hash **byte-identical** to the in-VM worker
  captures — host and VM execution agree bit-exactly on the real ROM.
- Pointers: `m5-suite-evidence.md` first-room section; report
  `target/m5-acceptance-20260707/vm-first-room-final-report.json`
  (map/expect/padlog are operator-side in the private root, per
  clean-room rules).

`refwork-d7t.1` now waits on exactly two operator one-liners: the
build-vs-vendor confirmation-or-waiver, and the aarch64 operator-game
run-or-defer decision. (The synthetic cross-arch floor is current — see
the table above; only the operator-game real-aarch64 leg is undecided.)

## 2026-07-11 Final M2 Floor Closeout

### Native operator-game x86_64/aarch64 equality

The retained, operator-approved first-room padlog and the single approved ROM
were run through the host-side release verifier for 100,000 requested frames on
the Intel reference host and on the physical aarch64 Spark. This is native
emulator evidence; it does not use KVM or claim ARM hypervisor support.

```sh
refwork-verify double-run --rom <operator-rom> \
  --script <private-first-room.padlog> --frames 100000 \
  --report <private-report.json>
```

| Field | x86_64 Intel | aarch64 Spark |
|---|---|---|
| Result | PASS, internally deterministic | PASS, internally deterministic |
| Frames requested | 100,000 | 100,000 |
| Final chain | `bead3862a94514a0590f370c37b023eb3cf011b9d94e13283234976058f556a8` | `bead3862a94514a0590f370c37b023eb3cf011b9d94e13283234976058f556a8` |
| First divergence | none | none |
| Private report BLAKE3 | `3469a7c2fff1ac422e283207652f3f6fb98349eca76e7aab20567b723cd780cb` | `3469a7c2fff1ac422e283207652f3f6fb98349eca76e7aab20567b723cd780cb` |
| Rust compiler | `rustc 1.97.0 (2d8144b78 2026-07-07)` | `rustc 1.92.0 (ded5c06cf 2025-12-08)` |

Shared provenance:

- UTC completion audit: `2026-07-11T23:38:15Z`;
- reference-workload: `4eb8a3a99197ae9e937c544ed0e4d320ee9da546`;
- control-plane: `66f0f9fd8e0e7bb39fb3331be20c549cde96b2e8`;
- determinism-hypervisor proto source: `6e348e5961b8ba81d91b7bdd4f79af102b809649`;
- guest-sdk sibling: `0fcddf455db6a386aa52d12560b1db74fc6cf4b1`;
- operator ROM BLAKE3: `96cdaa2380b593e1f3377fc5bf23a16a74e0e277a08ce988ea532b5a91c8c194`;
- first-room padlog BLAKE3: `e2565db2d40dfac0a398f15605835cac7fb8b96cf8a1ac24b183c89103fbb65c`.

The report bytes and final chains are identical across architectures despite
different native compilers. No ROM bytes, input semantics, decoded state,
framebuffer pixels, or private retrieval paths are recorded here.

### Build-versus-vendor confirmation

On 2026-07-11, Matt (operator and bead owner) reviewed
`.agents/decisions/2026-07-02-kernel-agent-artifact-split.md`, stated that it
"seems fine," and asked whether there were any concerns with accepting it for
`refwork-d7t.1`. The audit found no blocking concern, so this is the durable
operator confirmation that the decision is the M2 build-versus-vendor record:

- kernel: vendored hash-pinned guest-sdk artifact with provenance build key;
- agent and harness: built from pinned sibling revisions;
- `xtask image build`: enforces both pin paths;
- the documented `--agent-bin` test/escape hatch is non-default and does not
  alter the accepted production policy.

### Final status

All `refwork-d7t.1` acceptance clauses are satisfied by direct evidence; no
waiver or aarch64 deferral was used. The bead may close.
