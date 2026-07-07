# What The Bridge Provides For Verification

The deployed bridge (rombridge.birb.homes, `backend_mode: real`) is the
human-visible half of your gates 2 and 3, and we own the cutover procedure
your step 1 ends in.

## Standing Offer

1. **Cutover, coordinated.** When your regenerated READY snapshot exists,
   update the private handoff env file channel and we execute the
   restart procedure (worker + bridge, lease-invalidation caveat included,
   `rom-operator-bridge-72o`), so your lab session doesn't fight our live
   sessions. We schedule a window on request — same-day is normally fine.
2. **First-room, human-visible.** After cutover we run start →
   `RestoreSnapshot` → scripted input → `GetFramebuffer` → browser preview
   and file the result (frame PNG reference, frame counter, gRPC trail)
   back into this request directory — the same evidence format as
   `../determinism-hypervisor/.agents/requests/rom-bridge-getframebuffer-region-contract/06-deployed-verification.md`.
3. **The frame path is proven end-to-end.** With the fixed emulator and
   drain-fixed worker (`30d0cb9`, snapshot `69b96799`), the path —
   Run → BUDGET_REACHED, frame_counter=1, a correct 229,376-byte XRGB8888
   256×224 frame served as PNG, and browser preview returning it over
   HTTP 200 — is verified (bridge bead `bvq`; an earlier `preview=false`
   503 report in that bead was investigated and retracted — the repro had
   omitted requested capabilities; the UI requests preview correctly).
   The only remaining gap on our side is cosmetic (`resume()`'s response
   lags `current_frame`; the UI polls the correct endpoint). The real gap
   is content: the deployed `game.img` is ~96% zeros, so the frame is
   blank until your regenerated snapshot + a real ROM cut over.

## Runtime Facts You May Need

- Deployed worker: drain-fixed build `30d0cb9` over UDS
  `/run/dh/grpc.sock`. Snapshot refs in circulation: `22dc5b40` (the older
  deployed READY, pre-emulator-fix — not a baseline for your new image)
  and `69b96799` (the newer ref `bvq`'s verification used). We will
  confirm which ref `BRIDGE_REAL_SNAPSHOT_REF` serves at cutover time —
  ask us rather than assuming.
- Bridge restarts orphan live slots until the worker restarts (`72o`);
  that's inside the procedure we run, not something you need to handle.

## Contact / Tracking

- Bridge beads: `bvq` (first real frame — needs your snapshot + a real
  ROM *and* our own preview-capability/current_frame fixes), `9xo`
  (no-frame trail, open P0 — closes with your steps 1–2 + cutover), `72o`
  (slot leases).
- Your driving plan: `.agents/plans/phase3-m4-first-room-unblock/`.
- Operator-decision items we'll co-surface: ROM BLAKE3 / padlog BLAKE3 /
  run owner; branch merge path.
