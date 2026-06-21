# 05 - In-VM First-Room Gate

**Upstream package:** RW-3.

**Purpose:** prove the image and harness work through the real guest-sdk agent,
real hypervisor input path, and real host region capture. This is the M4 join
point with guest-sdk and determinism-hypervisor.

## External Dependencies

Do not start this package until these are available and cited in the evidence
note:

- RW-2/package 04 image handoff assets.
- guest-sdk GS-5 READY gate and reference-workload control handoff.
- guest-sdk GS-6 region readability gate.
- determinism-hypervisor DH-2 pv-pad scheduled input path.
- determinism-hypervisor DH-5 host region capture/read path.
- A lab operator ROM path and host-side first-room padlog from package 01.

## Deliverables

1. Add a `refwork-verify` integration command or lab harness for the real stack.
   Name it clearly, for example:

   ```sh
   refwork-verify vm-first-room --image <workload-image.yaml> --rom <operator>.rom --script <first-room>.padlog --map feature-maps/demo-game.yaml --expect <lab>/first-room-expect.yaml --report <lab>/rw3-report.json
   ```

   The exact hypervisor invocation should use the hypervisor-owned API or CLI
   available at implementation time. This repo should orchestrate the gate, not
   define hypervisor schemas.
2. Boot and READY validation:
   - Boot the package-04 image with the operator ROM attached read-only as the
     game-image device.
   - Confirm the real `detguest-agent` drives
     `Hello -> LoadGame -> Start` locally over fd 3.
   - Confirm the harness publishes `wram`, `framebuffer`, and `meta`.
   - Confirm READY occurs only after expected regions are live and layout
     versions match.
   - Distinguish the two READY concepts in the report:
     - harness protocol `Ready { frame: 0 }`: `meta.status = ready`,
       `meta.frame = 0`;
     - guest-sdk READY beacon/root snapshot: `Start {}` has already been
       delivered, no frame has completed yet, and `meta.status` is still
       `ready`, `meta.frame = 0`.
   - Confirm the VM reaches guest-sdk READY in under 2 seconds of host time,
     matching the reference-workload M4 acceptance criterion. This measurement
     is a host/lab metric only; no workload code may read wall-clock time.
3. Root restore behavior:
   - Take the root snapshot after the guest-sdk READY point.
   - Restore it in a fresh worker.
   - Confirm no host command is required after restore; the harness continues
     directly in the free-running frame loop.
4. Input path:
   - Inject the first-room script through the hypervisor-owned input path.
   - Do not send pad data over the control socket or detchannel.
   - Confirm `meta.last_pad` follows the scheduled pad sequence at frame
     boundaries.
5. Region capture:
   - Capture `wram` at frame boundaries and decode `room_id` with
     `feature-maps/demo-game.yaml`.
   - Capture `framebuffer` checkpoints through the host region path.
   - Compare framebuffer checkpoint hashes against lab goldens stored outside
     the repo.
   - Confirm `meta.frame` matches the hypervisor frame table.
6. Evidence report:
   - JSON report with image manifest hash, repo git rev, guest-sdk rev,
     hypervisor rev, snapshot-store rev if used, operator ROM BLAKE3, padlog
     BLAKE3, first-room transition frame, region hashes, framebuffer checkpoint
     hashes, and pass/fail status.
   - No ROM bytes, WRAM dumps, framebuffer images, or script semantics in the
     repo.

## Lab Evidence Configuration

Before running this gate, record these fields in the evidence note:

- owner responsible for the run;
- runner label or machine name;
- artifact root for reports and large logs;
- guest-sdk, hypervisor, snapshot-store, and control-plane revisions;
- operator ROM BLAKE3 and padlog BLAKE3;
- exact command used to invoke the hypervisor/worker API.

## Acceptance

- Scripted input injected through the real hypervisor path reaches the first-room
  transition in-VM.
- Host-side region capture observes the transition through `room_id`.
- Framebuffer capture returns expected checkpoint hashes.
- Harness registers regions and reaches READY through real `detguest-agent`.
- VM boots to guest-sdk READY in under 2 seconds of host time.
- A restored root fork continues directly into the frame loop with no host
  `Start` or `LoadGame` command.
- Failure reports identify whether the failure is boot/READY, input landing,
  region capture, feature decode, or framebuffer checkpoint mismatch.

## Stop Conditions

- If the hypervisor cannot pause at `FrameMark` boundaries, stop. Frame-coherent
  capture is a hypervisor contract package 06 depends on.
- If the agent requires a host command after restore to start the harness, stop
  and fix guest-sdk/handoff behavior before continuing.
- If the harness reaches READY before regions are published, stop and fix the
  package-02/package-04 ordering.
