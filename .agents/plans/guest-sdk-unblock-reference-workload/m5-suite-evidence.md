# M5 Full-Suite Evidence (`refwork-d7t.12`–`.14`)

Recorded 2026-07-07 by Claude (coding agent) with the operator (Matt
Spurlin, who supplied the game image and lab window). Clean-room
boundary: revisions, command shapes, hashes, and artifact pointers only —
no ROM bytes, padlog semantics, framebuffer images, WRAM dumps, or
goldens.

## Run Context (`refwork-d7t.12` readiness record)

| Field | Value |
|---|---|
| Date | 2026-07-07T21:38:34Z |
| Run owner | Matt Spurlin (operator) |
| Runner | `infra-control` — Intel i5-8400, Linux 6.8.0-124-generic (the Intel lab runner; same host as the `[self-hosted, intel, kvm]` CI label) |
| reference-workload image rev | `7b0c7b2` (manifest `built_from.git_rev`) |
| reference-workload verify rev | `320f425` (includes the vm-suite AgendaNotEmpty split-injection fix) |
| guest-sdk pin | `acb1d3e8` (`image/guest-sdk.lock`) |
| Hypervisor worker | `dh-workerd` at deployed drain-fixed rev `30d0cb9`, locally launched on scratch UDS (never `/run/dh/grpc.sock`) |
| snapshot-store | `snapstore-server` release build from sibling checkout, scratch data root/config |
| Image manifest BLAKE3 | `af14040444db6f5e182f52193d71abdbbfb8085673b45da76c21dc541ac3dceb` |
| Operator ROM BLAKE3 | `96cdaa2380b593e1f3377fc5bf23a16a74e0e277a08ce988ea532b5a91c8c194` |
| Fixed input log BLAKE3 | `af9d57b3ca3534103c69dcae87d3dc533b7788d969ae10676c5ddb2c1b0a9bdf` (M5 suite log; the *first-room* padlog for gate 3 is separate and still operator-side) |
| READY snapshot | regenerated 2026-07-07 via `dh-m9-ready-handoff` from the `7b0c7b2` image + operator game image; TakeSnapshot + RestoreSnapshot verification green; ref lives in the private handoff env (private root outside every checkout), per the runbook's privacy rule |
| Artifact root | `target/m5-acceptance-20260707/` |

Exact invocations (worker UDS/snapshot ref redacted per runbook):

```
refwork-verify vm-suite --worker <scratch-uds> --snapshot-ref <ref> \
  --script <fixed-log>.padlog --frames 600 --snapshot-at 300 \
  [--iterations 20 | --nondet-test] --report <out>.json
```

## Results (`refwork-d7t.13`/`.14`)

| Leg | Result | Report BLAKE3 |
|---|---|---|
| Single iteration, both legs (double-run + restore-continuity) | PASS | `9b3154f82d9629eca10d7d6357b5e09b440270c447a71cf9d459994baed3c676` |
| Negative (`--nondet-test`, one pad word perturbed in run 2) | FAILs as required, divergence localized at frame 301 | `91729d978c9aec3d7e1ac55cebd7b5d6207ee27a0aa3a3b9f132715b3d4a5d83` |
| **20× campaign, both legs each iteration** | **PASS — 20/20, zero flakes, single trajectory hash across all 20** | `a06051df0ce076daa49f48298b25959b7a83dac8deb23cf247177f6c2bbe13c3` |

NOP-ROM rehearsal (same mechanics, placeholder content, run first to
de-risk): 20/20 zero-flake, and it caught the vm-suite mid-run
TakeSnapshot `AgendaNotEmpty` contract violation fixed at `320f425`
(mock now enforces the same precondition in staged CI). Rehearsal
reports are in the same artifact root.

## Green Stamp + Register Gate

- `dist/workload-image-0.1.0/determinism.last_green` written
  2026-07-07T21:38:34Z (replaces `determinism.unstamped.yaml`, which is
  deleted — the two must not coexist); `suite_report_blake3` = the 20×
  report hash above.
- `xtask image validate` → OK with the stamp present.
- `xtask image register --require-green-stamp` → "determinism green
  stamp present" (DirectDistStamped). The refusal path was demonstrated
  live earlier the same day against the unstamped sidecar, and is
  covered by existing xtask tests.

## Scope Notes

- Phase 3 exit gate 1 (M5 20× zero-flake including snapshot/restore
  continuity) is satisfied by the campaign above.
- Exit gate 3 (first room in-VM) remains open: `refwork-verify
  vm-first-room` needs the operator first-room padlog and real
  feature-map/expect goldens (`ramdiff`/`map-check`), plus the bridge
  cutover for the browser-visible half. Tracked in `refwork-d7t.11`.
- The `BRIDGE_REAL_SNAPSHOT_REF` cutover to the new snapshot is
  bridge-executed and pending scheduling; the private handoff env from
  the regen is ready for them.

## 2026-07-07 (later): First Room In-VM — Exit Gate 3 Technical Half (`refwork-d7t.11`)

The operator's game image enabled full feature discovery, so the
first-room gate ran the same day. Clean-room note: the feature map,
vm-expect goldens, and first-room padlog live in the private root
(operator-side, outside every checkout); this note records hashes and
pointers only.

| Item | Value |
|---|---|
| Feature discovery | `ramdiff` record/search/watch against the operator ROM: marked session (title/menu/stage-card/gameplay), room feature at a WRAM u8 offset with monotone trajectory boot→0 → menu-select→48 → first-room→167, stable through all gameplay marks |
| First-room padlog BLAKE3 | `e2565db2d40dfac0a398f15605835cac7fb8b96cf8a1ac24b183c89103fbb65c` (title → 1P GAME → intro skip → Stage 1 "TREETOPS" gameplay) |
| Dry-run (goldens recorded) | PASS, report `e53d2fc8f14eec56f4cff9000d16820d2c95462677530efa341e62d6d879b6ff` |
| **Validating run (goldens enforced)** | **PASS** — `frames_run=4200`, room transition 0→167 `observed_by_frame=1528`, both framebuffer checkpoints (3400, 4200) matched, `pad_trace_ok=true` |
| Ready proof | `meta_status=ready`, `meta_frame=0`, `room=0` at root snapshot, framebuffer 229,376 B `xrgb8888-256x224-stride1024` |
| **Host↔VM bit-exactness** | host-side `play --snap` framebuffer dumps at frames 3400/4200 hash byte-identical to the in-VM worker captures (`2867fa06…`, `5416075f…`) |
| Sequence | `RestoreSnapshot → InjectInputs → Run → GetFramebuffer` end-to-end through the worker gRPC API — Phase 3 exit gate 3's command sequence verbatim |
| Report | `target/m5-acceptance-20260707/vm-first-room-final-report.json` |

Remaining on gate 3: the bridge team's browser-side confirmation after
the coordinated `BRIDGE_REAL_SNAPSHOT_REF` cutover (their standing
offer; human-visible half). Host time to READY: restore-based (the VM
restores a READY snapshot rather than cold-booting), so the bead's
"READY under 2s" clause is satisfied by the restore path; the cold-boot
READY timing was verified worker-side 2026-07-05.
