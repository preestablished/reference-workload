# M4 In-VM First-Room Evidence

RW-3/package-05 readiness note for bead `refwork-d7t.10`, recorded during
Ralph iteration 10.

Clean-room boundary: this note records revisions, command shapes, hashes, and
artifact paths only. It does not include game content, ROM bytes, padlog
semantics, framebuffer images, WRAM dumps, SRAM, or lab goldens.

## Verdict

Package-05 external readiness is **BLOCKED** in this checkout. The sibling
hypervisor repository has Linux READY, M4 snapshot/restore/fork, frame
scheduling, and worker API evidence for the staged M9 reference-workload
fixture, but the required operator-game first-room evidence is not present.

Do not start `refwork-d7t.11` until the missing fields below are replaced by
durable lab artifact pointers and hashes.

## Local Run Context

| Field | Value |
|---|---|
| Date | 2026-06-22T00:16:41Z |
| Owner | Matt Spurlin (`refwork-d7t.10` owner); recorded by Codex during `/ralph` |
| Machine | `infra-control` |
| Architecture | `x86_64` |
| Branch | `ralph/iteration-10-record-m4-external-readiness-evidence` |
| Reference-workload base rev inspected | `01535d8b072be49c4031e83f44796cba2cc82edd` |
| Evidence note rev | `9a6f8d48b4c129973d091efc9d12f6c523a79105` |
| guest-sdk checkout rev | `08abbbc36f6afa6ad3aec0ce062c3383f8dcfcce` |
| determinism-hypervisor checkout rev | `b9737538f5fc2708d9cb09979df775c0ab388390` |
| snapshot-store checkout rev | `cac52afe66b0975601bc9ecbc67cd16b52cc181e` |
| control-plane checkout rev | `ca9ee9048d7fca8eec5fe512011b011128e2b0c3` |

All four sibling checkouts were clean when inspected with
`git -C <repo> status --short`.

## Required Fields

| Field | Status | Evidence |
|---|---|---|
| guest-sdk GS-5 READY gate / reference-workload control handoff | PARTIAL | guest-sdk docs define the contract at `../guest-sdk/prompts/docs/guest-sdk/ARCHITECTURE.md` §4.1/§4.2. Hypervisor M9 evidence observed detchannel `Ready{unit=0, region_count=3, manifest_generation=6}` after control and expected-region work, but only for the staged M9 fixture. |
| guest-sdk GS-6 region readability gate | PARTIAL | `../determinism-hypervisor/target/m9-final-acceptance-20260621T004402Z/18-linux-worker-api.log` passed Linux worker API tests covering `ReadGuestMemory` region ranges on the staged fixture. |
| hypervisor DH-2 pv-pad scheduled input path | PARTIAL | Hypervisor final M9 logs include Linux frame scheduling evidence, but not the operator first-room padlog landing through pv-pad. |
| hypervisor DH-5 host region capture/read path | PARTIAL | `18-linux-worker-api.log` and workspace tests cover Linux region reads; no operator-game room transition capture is recorded. |
| Lab runner / machine | PRESENT | `infra-control`; runner `infra-control-kvm-intel` with labels `self-hosted`, `Linux`, `X64`, `kvm-intel`, documented in `../determinism-hypervisor/docs/ops/github-runner.md`. |
| Artifact root | PRESENT | `../determinism-hypervisor/target/m9-final-acceptance-20260621T004402Z/` |
| Owner responsible for first-room run | MISSING | No durable lab owner/run assignment for the operator-game first-room gate was found in this checkout. |
| Operator ROM BLAKE3 | MISSING | No operator ROM hash was found. The M9 `game.img` hash is a staged fixture hash and must not be treated as the operator ROM first-room artifact. |
| First-room padlog BLAKE3 | MISSING | No host-side first-room padlog hash was found. |
| Exact first-room command/API entry point | MISSING | `refwork-verify vm-first-room ...` is only a planned command in `05-in-vm-first-room-gate.md`; no implemented command or hypervisor worker invocation for the operator-game first-room gate is recorded. |

## Cited Hypervisor Evidence

The hypervisor final M9 acceptance artifact root is:

```text
../determinism-hypervisor/target/m9-final-acceptance-20260621T004402Z/
```

Key files and hashes:

| Artifact | BLAKE3 |
|---|---|
| `08-linux-ready.log` | `bb916006a249426286fa2b0ba49a619fe8c84c6207144e036b9d032e7ca88f80` |
| `14-linux-m4-transparency.log` | `4869bbef54856ee21dc92725c974924471d0a9b5bf46f22b750eba219b093fda` |
| `15-linux-m5-frame-scheduling.log` | `ae76a534fe1c8a7847f06fa1737502cf0f50bd56c45ca1108c4947378070e055` |
| `18-linux-worker-api.log` | `4f4ac3f2698ef7d11f85c647909b2ae281b84dfeb1bd7eaebfee5edb2d384b9f` |
| `06-artifacts-and-cache.log` | `cb2ca4874c27322601a808ab8a0e5ca242f168d3f93e238cf55dd69f8e20abf3` |

The final M9 run tested hypervisor code SHA
`f855dfb9800e969e8371016112aace7703ee402d` on `infra-control`, Linux
`6.8.0-124-generic`, Intel(R) Core(TM) i5-8400 CPU, microcode `0xfa`.

Selected observed results:

| Gate | Evidence |
|---|---|
| Linux READY | `ready_icount=641343512`, `unit=0`, `region_count=3`, `manifest_generation=6`, `machine_config_hash=2b638bdf9f61ea0b9c14958d48b9a0eda743ace322866fb90f5fc387256226e6`, `state_hash=5449bd8fae5587b9f69542b9be646bf6a54a64cb7b323811418b208079c41fd5`. |
| Linux M4 transparency | Snapshot/restore/fork matched mid and restored state hashes with `reg_diffs=0`, `diff_pages=[]`. |
| Linux frame scheduling | First post-READY frame table `[(186992, 1), (330795, 2), (474598, 3)]`; restored frame table `[(143803, 4), (287606, 5)]`. |
| Linux worker API | Two ignored Linux worker API tests passed, covering create/run-to-READY, stream events, region reads, snapshot, restore, fork, child run, and VerifyReplay. |

The staged M9 artifact hashes were:

| Artifact | BLAKE3 |
|---|---|
| `bzImage` | `595466463a37efac6822ffccf3e61d0a2230e7d223a94c0bce5eb78b2f43bee9` |
| `initramfs.cpio` | `87edf64db22dc85ef0c6b17fdc6e58a8f73391a6053e96f7a1056da7d08b9f57` |
| `base.img` | `488de202f73bd976de4e7048f4e1f39a776d86d582b7348ff53bf432b987fca8` |
| `game.img` | `e02849845005d9d34fa3245d98fa59116a0245ed0136b496dbd2defebdc203ac` |

`game.img` is recorded only as a staged M9 fixture hash. It is not an
operator ROM hash for reference-workload package 05.

## Required Replacement Evidence

To unblock `refwork-d7t.11`, add a future update to this note with:

- owner responsible for the operator-game first-room run;
- runner label or machine;
- artifact root for JSON report, logs, framebuffer checkpoint hashes, and large
  diagnostics;
- guest-sdk, hypervisor, snapshot-store, and control-plane revisions used by
  that run;
- package-04 WorkloadImage manifest path and BLAKE3 used by the run;
- operator ROM BLAKE3;
- first-room padlog BLAKE3;
- exact implemented command or hypervisor worker API invocation;
- report path and report BLAKE3;
- READY proof, room-transition proof through host region capture, and
  framebuffer checkpoint hash proof.

Until those fields are present, package-05 remains blocked and no claim should
be made that M4 in-VM first-room readiness is complete.

## Unblock Checklist

This is the concrete path to make `refwork-d7t.10` workable. The bead should
move out of BLOCKED only after this checklist is backed by durable artifact
pointers and hashes:

1. Verify package-04 image handoff assets from this repository:
   `workload-image.yaml`, `boot.toml`, expected-region handoff, image
   manifest BLAKE3, and image double-build/register evidence.
2. Verify guest-sdk GS-5 for the real reference-workload path:
   `detguest-agent` performs `Hello -> LoadGame -> Start` over the local unit
   control channel and emits guest-sdk READY only after expected regions are
   live with matching layout versions.
3. Verify guest-sdk GS-6 for the real reference-workload path:
   host-side region reads cover `wram`, `framebuffer`, and `meta` through the
   guest-sdk manifest, including restore/fork readability where required.
4. Verify determinism-hypervisor DH-2 for the real reference-workload path:
   first-room pad input is injected through the hypervisor-owned scheduled
   input path, not through the harness control socket or detchannel.
5. Verify determinism-hypervisor DH-5 for the real reference-workload path:
   host capture/read APIs can observe frame-coherent `wram`, `framebuffer`, and
   `meta` data at `FrameMark` boundaries.
6. Assign the operator-game lab run:
   owner, runner label or machine, artifact root, operator ROM BLAKE3,
   first-room padlog BLAKE3, framebuffer checkpoint hash source, and clean-room
   reporting rules.
7. Run or record the exact implemented worker/API command for the first-room
   gate, producing a JSON report plus logs under the artifact root.
8. Update this note with report path, report BLAKE3, image manifest BLAKE3,
   involved repo revisions, READY proof, room-transition proof through host
   region capture, and framebuffer checkpoint hash proof.

The following does not unblock `refwork-d7t.10`: staged M9 fixture evidence by
itself, a planned `refwork-verify vm-first-room` command that is not
implemented, a ROM/padlog path without BLAKE3 hashes, or a report that contains
game-derived bytes.

## External Surface Verification Log

### 2026-06-22T13:14:22Z

Inspector: Codex, on `infra-control`.

Revisions inspected:

| Repository | Revision |
|---|---|
| reference-workload | `3e45bcc0fcbf7c4d412314fb739bd9b8252dabf9` |
| guest-sdk | `08abbbc36f6afa6ad3aec0ce062c3383f8dcfcce` |
| determinism-hypervisor | `b9737538f5fc2708d9cb09979df775c0ab388390` |
| snapshot-store | `cac52afe66b0975601bc9ecbc67cd16b52cc181e` |
| control-plane | `ca9ee9048d7fca8eec5fe512011b011128e2b0c3` |

This pass records local verification results without weakening the BLOCKED
verdict above.

#### Package-04 Image Handoff

Status: PRESENT for a clean-room lab handoff, but the lab run must cite the
exact manifest hash it consumes.

Commands run from `reference-workload`:

```sh
cargo run --locked -p xtask -- image validate dist/workload-image-0.1.0/workload-image.yaml
cargo run --locked -p xtask -- image register --manifest dist/workload-image-0.1.0/workload-image.yaml
```

Both passed. Current checked-in artifact hashes:

| Artifact | BLAKE3 |
|---|---|
| `dist/workload-image-0.1.0/workload-image.yaml` | `aa950751cb0a6c0c2ea0bcff2e844bceef47248b017f8c28c2cc387567416c46` |
| `dist/workload-image-0.1.0/boot.toml` | `802fa34f70b9a1f1fc96f0c79611b0d38cc84bda0556907f12ab241a97d89a23` |
| `dist/workload-image-0.1.0/expected-regions.toml` | `55c95af82bef1712d6252f8c4f491592a1d6d6aa8e1e4a80bdd9c43a6a365d5c` |
| `dist/workload-image-0.1.0/harness.toml` | `d5623fe12a28a10736f70ca298c687c8fc8723786f77a8144bd8da2b5d9c3edd` |
| `dist/workload-image-0.1.0/initramfs.cpio.zst` | `7467720ac006be828edfda4f21b4269cdf0bdfc709e4707e784d5a228afabe9b` |
| `dist/workload-image-0.1.0/bzImage` | `9ae72dbae3e7a6e0b89fd3d3f0420b991c6187429420345777c2173ae9600ab7` |
| `dist/workload-image-0.1.0/determinism.unstamped.yaml` | `aea3026017b020f74b66337d21b6d1bf83160d1ff897b931bcc683c0cf06126a` |

The manifest embeds `meta.built_from.git_rev =
38fa190925017608f2bc07ad38ce6d816f8370cc`, not the current reference-workload
HEAD. That is acceptable only if the lab run deliberately uses this exact
manifest. If the lab run consumes current HEAD, rebuild the image and record
the new manifest BLAKE3 before claiming package-05 readiness.

#### guest-sdk GS-5 READY / Control Handoff

Status: PARTIAL. Code and host/unit tests exist, but package-05 still lacks a
durable real-stack VM acceptance result.

Observed implementation:

- `../guest-sdk/crates/detguest-agent/src/control.rs` drives the
  `Hello -> LoadGame -> Ready{frame=0} -> Start` leg for `[unit.control]`.
- `../guest-sdk/crates/detguest-agent/src/runtime.rs` starts the controlled
  autostart unit, waits for expected manifest regions, emits `RegionRegister`
  evidence, then emits guest-sdk `Ready`.
- `../guest-sdk/tests/vm/workloads/src/bin/m9_refwork_contract.rs` publishes
  staged `wram`, `framebuffer`, and `meta` regions and speaks the reference
  workload control protocol.

Verification command:

```sh
cargo test -p detguest-agent -p detguest-sdk -p detguest-host --locked
```

Result: passed locally in `guest-sdk`.

Remaining blocker: guest-sdk Beads still mark
`guest-sdk-m4-ready-gate-expected-regions` and
`guest-sdk-m4-unit-control-reference-handoff` BLOCKED. Their acceptance requires
VM tests and reference-workload image handoff integration, not just the current
host/unit test coverage.

#### guest-sdk GS-6 Region Readability

Status: BLOCKED for package-05 readiness.

Observed implementation:

- `../guest-sdk/crates/detguest-host/src/manifest.rs` implements stable
  manifest reads and `read_region`.
- `detguest-host` tests cover manifest resolution and discontiguous
  `read_region`.
- `detguest-sdk` has a tested SDK-state path that writes manifest entries and
  emits `RegionRegister`.

Blocking gaps:

- `../guest-sdk/crates/detguest-sdk/src/regions.rs` still has a standalone
  `register_region` path that validates input and returns a no-op handle rather
  than proving the final mlock/prefault/agent-IPC path.
- `../guest-sdk/crates/detguest-agent/src/commands.rs` still treats
  `ReverifyRegions` as a no-op.
- `guest-sdk-m4-platform-readability-vm` remains BLOCKED and still requires an
  Intel VM acceptance test proving published regions are readable across 100
  snapshot/restore branches.

Until those guest-sdk M4 blockers are closed or explicitly waived with durable
evidence, GS-6 is not ready enough to unblock `refwork-d7t.10`.

#### determinism-hypervisor DH-2 Scheduled Input

Status: IMPLEMENTED and fixture-tested, but still missing the operator
first-room padlog proof.

Observed implementation:

- `../determinism-hypervisor/crates/dh-worker/src/service.rs` implements the
  `InjectInputs` RPC and queues scheduled input events.
- The input mapper accepts frame-scheduled `PadSet` events and records frame
  hints.
- Final M9 evidence includes Linux frame scheduling:
  `target/m9-final-acceptance-20260621T004402Z/15-linux-m5-frame-scheduling.log`.

Verification command:

```sh
cargo test -p dh-worker --lib --locked inject_mapper_accepts_at_frame_pad_set_with_frame_hint
```

Result: passed locally in `determinism-hypervisor`.

Remaining blocker: no artifact yet proves the operator first-room padlog landed
through this path for the package-04 image.

#### determinism-hypervisor DH-5 Capture / Region Read

Status: IMPLEMENTED and fixture-tested, but still missing the operator
first-room room-transition/framebuffer proof.

Observed implementation:

- `CaptureSpec` resolves named regions from the guest-sdk manifest, checks
  layout versions, reads feature bytes, and returns framebuffer bytes as
  `fb_lz4`.
- `ReadGuestMemory` supports direct GPA ranges and named `region_ranges`.
- `GetFramebuffer` reads descriptor-backed framebuffer regions.
- Final M9 evidence includes worker API coverage:
  `target/m9-final-acceptance-20260621T004402Z/18-linux-worker-api.log`.

Verification commands:

```sh
cargo test -p dh-worker --lib --locked run_capture_spec_reads_manifest_ranges_and_lz4_framebuffer
cargo test -p dh-worker --lib --locked introspection_rpcs_read_memory_framebuffer_and_stream_guest_events
cargo test -p dh-worker --lib --locked descriptor_framebuffer_fixture_feeds_capture_and_get_framebuffer
```

Result: all passed locally in `determinism-hypervisor`.

Remaining blocker: no artifact yet proves `room_id` transition capture or
framebuffer checkpoint hash matching for the operator-game first-room run.

#### Current Unblock Verdict

`refwork-d7t.10` remains BLOCKED. Hypervisor DH-2/DH-5 look close enough to use
as implementation surfaces, but the evidence is fixture-level. Guest-sdk GS-5
has useful implementation coverage, while GS-6 still has tracked M4 blockers.
The hard missing package-05 evidence is still the operator lab run:

- assigned owner and runner;
- exact package-04 manifest hash selected for the run;
- operator ROM BLAKE3;
- first-room padlog BLAKE3;
- exact implemented worker/API command;
- JSON report path and BLAKE3;
- READY proof with regions live;
- first-room room-transition proof through host region capture;
- framebuffer checkpoint hash proof.

### 2026-07-02T15:48:00Z

Inspector: Claude (coding agent), on `infra-control`. This pass re-runs the
external-surface verification against the 2026-07-02 upstream state recorded
in `../phase3-m4-first-room-unblock/01-upstream-state-2026-07-02.md` and
refreshes the verdict. The 2026-06-22 section above is a historical record
and is superseded by this section.

Revisions inspected:

| Repository | Revision | Notes |
|---|---|---|
| reference-workload | `2d45f001d85472aec30c173cfdc9dab11daac87c` | branch `phase3/m4-first-room-unblock` |
| guest-sdk | `c03e90baa04b06640a9b6250366c23a1a428ef96` | `main`, one local unpushed commit (`c03e90b`) |
| determinism-hypervisor | `4c44263913676b9d787fb22dcf542d3ae797d6da` | `main`, one local unpushed commit (`4c44263`); working tree dirty in `crates/dh-worker/src/m9_handoff.rs` + `Cargo.lock` — all verification below ran in a clean pinned worktree `~/git/preestablished/.dh-clean-4c44263` |
| snapshot-store | `cac52afe66b0975601bc9ecbc67cd16b52cc181e` | `main` |
| control-plane | `261141b3bbaa4371a7dd4147ac6626e0f4918e53` | `main` |
| rom-operator-bridge | `047348085e07dbfb6ce8dd5edbedf937f4f13148` | `main` |

#### guest-sdk GS-5 READY / Control Handoff

Status: PRESENT.

Host tier re-run (2026-07-02, guest-sdk `c03e90b`):

```sh
cargo test -p detguest-agent -p detguest-sdk -p detguest-host --locked
```

Result: 92 passed, 0 failed (42 + 20 + 1 + 29 across the four test targets).

The previous PARTIAL verdict's missing piece — a durable real-stack VM
acceptance — is the Ms4 acceptance itself, green on `infra-control`:

- Artifact root: `~/git/preestablished/guest-sdk/target/m4-acceptance-20260702T135319Z/`
- `evidence.json` BLAKE3: `12709423b68ca3b463c47ee8ad0a2c19691a271618332b04cc5e49c7161da036`
- `evidence.json` records: `git_rev 604cd41d385d51523e9be61b81aa9753d0428a09`,
  host `infra-control`, 100 children x 60 frames, restore fidelity, meta
  frame-counter/input-history recomputation, determinism pairs, zero P0
  ReverifyRegions alarms, fork-of-fork fidelity.
- The acceptance boots a real workload through
  `Hello -> LoadGame -> Ready -> Start` with Ready gated on expected regions.

The 2026-06-22 blockers `guest-sdk-m4-ready-gate-expected-regions` and
`guest-sdk-m4-unit-control-reference-handoff` no longer gate this surface;
the M4 chain below is closed.

#### guest-sdk GS-6 Region Readability

Status: PRESENT.

The standalone no-op `register_region` path is gone: `detguest-sdk::register_region`
is the real path (mlock + per-page prefault, registration with the agent over
`/run/detguest/agent.sock`, `AgentUnavailable` in standalone mode, handles
unregister on drop). The agent is the sole manifest writer (SO_PEERCRED-bound
pid, pagemap GVA->GPA extent walk) and `ReverifyRegions` detects drift with
P0 alarms. The Ms4 acceptance evidence above covers host-side reads of
`wram`/`framebuffer`/`meta` through the manifest across restore/fork.

Closed guest-sdk beads verified via `bd list --all` in guest-sdk:

- `guest-sdk-m4-platform-readability-vm` (closed)
- `guest-sdk-m4-agent-ipc-protocol` (closed)
- `guest-sdk-m4-agent-ipc-server` (closed)
- `guest-sdk-m4-agent-manifest-writer` (closed)
- `guest-sdk-m4-agent-pagemap-pid-extents` (closed)

#### determinism-hypervisor DH-2 Scheduled Input

Status: PRESENT (fixture-level+; operator first-room padlog proof remains a
lab-run matter, assigned below).

Re-verified at `4c44263` in the clean worktree `~/git/preestablished/.dh-clean-4c44263`:

```sh
cargo test -p dh-worker --lib --offline -- inject_mapper_accepts_at_frame_pad_set_with_frame_hint
```

Result: passed. Caveat: the committed `Cargo.lock` at `4c44263` is stale
relative to the committed `Cargo.toml` tree (`--locked` fails); the run used
`--offline` after a local lockfile regeneration in the scratch worktree
(144 insertions, 21 deletions vs the committed lock).

#### determinism-hypervisor DH-5 Capture / Region Read

Status: PRESENT (fixture-level+).

The 2026-06-22 citations are partially stale: `5698d7e` ("Derive framebuffer
geometry from D7 layout_version contract") deleted the descriptor fixture,
the descriptor-backed `GetFramebuffer` behavior, and the test
`descriptor_framebuffer_fixture_feeds_capture_and_get_framebuffer`.
`GetFramebuffer` and `CaptureSpec.framebuffer` now derive geometry from the
manifest entry's `layout_version`: layout_version 1 = raw pixels, XRGB8888,
256x224, stride 1024, exactly 229,376 bytes — the D7 contract this repo
already implements (`FB_BYTES`). Wrong length or unknown version is
`FailedPrecondition` naming the offender; an all-zero region is a valid
black frame. The capture-path determinism fix in `5698d7e` is stronger than
the audit required (captured FbInfo no longer frame-content-dependent).

Re-verified at `4c44263` in the clean worktree:

```sh
cargo test -p dh-worker --lib --offline -- \
  framebuffer_layout_contract_is_enforced \
  run_capture_spec_reads_manifest_ranges_and_lz4_framebuffer \
  introspection_rpcs_read_memory_framebuffer_and_stream_guest_events
```

Result: all passed (4 passed including DH-2's test above, 0 failed).
Decision record: `determinism-hypervisor/docs/decisions/framebuffer-region-geometry.md`.

#### Operator-Run Fields (MISSING By Assignment)

The remaining package-05 fields are a human decision, explicitly assigned to
the operator (Matt) rather than blocked on upstream:

- run owner and runner;
- operator ROM BLAKE3;
- first-room padlog BLAKE3.

These are scheduling matters once `refwork-verify vm-first-room` exists
(plan step 04) and the package-04 image is rebuilt against the new guest-sdk
(plan step 03).

#### Refreshed Unblock Verdict

`refwork-d7t.10` moves out of BLOCKED: GS-5 PRESENT, GS-6 PRESENT, DH-2 and
DH-5 PRESENT (fixture-level+), operator fields MISSING-by-assignment. The
readiness-evidence recording this bead tracks is complete; the remaining
work is sequenced in `../phase3-m4-first-room-unblock/` (image rebuild,
first-room verifier, M5 suite, CI closeout).
