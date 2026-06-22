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
