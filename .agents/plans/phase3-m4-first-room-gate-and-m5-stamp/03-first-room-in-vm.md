# Step 03 — First Room In-VM Through Worker gRPC (`refwork-d7t.11`)

Phase 3 exit gate 3 verbatim: a scripted input log plays the first room
in-VM, driven entirely through the worker gRPC API —
`RestoreSnapshot → InjectInputs → Run → GetFramebuffer` shows the room.

The verifier already exists (`refwork-verify vm-first-room`,
`crates/refwork-dh-client`; 6 staged-fixture tests green). This step is
the **lab run**, not implementation. Prior-plan reference:
`.agents/plans/phase3-m4-first-room-unblock/04-first-room-verifier.md`.

## Preconditions

- Step 02's regenerated READY snapshot exists.
- Operator lab fields in hand (ROM BLAKE3, padlog BLAKE3, run owner) —
  hard-required by the evidence record.
- For the bridge-visible half: cutover executed by the bridge team
  (their procedure, their window). The lab run itself can go first
  against the **local** worker + new snapshot without waiting.

## Work

1. **Local lab leg**: run `refwork-verify vm-first-room` against the
   locally-launched real worker (step 02's scratch UDS) with the new
   snapshot and the operator's first-room padlog. The bead's acceptance
   criteria are the checklist:
   - regions (`wram`/`framebuffer`/`meta`) live before guest-sdk READY;
   - `meta.status=ready`, `frame=0` at the root snapshot;
   - READY under 2 seconds host time;
   - first-room transition observed via host wram capture (room_id
     decode);
   - framebuffer checkpoint hashes match lab goldens;
   - `meta.frame` matches the hypervisor frame table;
   - restore continues directly into the frame loop without host
     Start/LoadGame.
2. **File the evidence artifact**: verifier output (frame hash +
   framebuffer capture *reference* — never the image itself) under the
   evidence discipline, new dated section in
   `m4-in-vm-first-room-evidence.md`, including the operator fields.
3. **Bridge confirmation**: after their cutover, the bridge runs start →
   RestoreSnapshot → scripted input → GetFramebuffer → browser preview
   and files the result into the request directory
   (`.agents/requests/phase3-m4-first-room-gate-and-m5-stamp/`) — their
   standing offer. Ask them to confirm which ref
   `BRIDGE_REAL_SNAPSHOT_REF` serves rather than assuming.

## Failure Triage

- No frame / blank frame: check the worker rev is drain-fixed
  (`30d0cb9`+) and the harness isn't on `NoopPlatform` (that class of bug
  was fixed in `refwork-4qj` / `40eaf4f` — a recurrence means a config
  regression, not a new mystery).
- No READY: read the `boot: *` breadcrumbs in the dump; decision table in
  `.agents/plans/phase3-ready-not-emitted-real-worker/01-diagnosis-breadcrumbs.md` §5.
- Worker RPC failures: gRPC code + message at
  `journalctl -u rom-operator-bridge` (bridge side); the hypervisor names
  offenders precisely (layout version, byte counts).

## Exit Criteria (closes `refwork-d7t.11`)

- All bead acceptance items pass on the real worker + new snapshot.
- Evidence section filed with operator fields; bead closed citing it.
- Bridge browser-side confirmation filed (or noted pending with their
  window scheduled — don't block step 04 on the bridge's paperwork).
