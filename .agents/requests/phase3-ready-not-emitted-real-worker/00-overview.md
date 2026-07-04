# Request: Guest-SDK Ready Never Emitted Under The Real Worker

Filed 2026-07-04 by the rom-operator-bridge session driving Phase 3
step 2. This is a joint refwork-harness + guest-sdk agent issue; filed
here because the visible failure is in `refwork-harness`'s control leg /
frame loop, but the agent (`guest-sdk`) owns the Ready emission — see
`01-evidence.md` for who owns which half.

## How Far We Got

Four blockers were found and cleared on the first real boot of the
package-04 workload image; this is the fifth and last-standing:

1. ✅ `/init` shell-script-in-a-shell-less-image → agent symlink
   (reference-workload `fbf32c6`).
2. ✅ Game-device materialization (guest-sdk `f143ffc`: agent reads the
   game from pv-blk into tmpfs before LoadGame).
3. ✅ `RegisterRegion` (refwork-ctl tag 7) decode gap — the real harness
   sends region announcements the staged fixture never did; agent now
   skips them (guest-sdk `322c331`).
4. ✅ Adopted in this repo (`cdcb372`): `game_source = "pv-blk"`, lock at
   `322c331`.

The boot now clears kernel → agent-as-PID-1 → pv-blk materialize
(checksummed 32,768-byte game) → LoadGame → **all three regions
register** (`wram`/`framebuffer`/`meta`, `manifest_generation 6`, at
icount ~643 M — matching the staged fixture's ~641 M READY point).

## The Blocker

**Then nothing.** Under the real worker the guest runs 9.3 billion more
instructions to the hard cap without emitting the guest-sdk `Ready` SDK
event, so the M9 handoff's `Run{until: NextSdkEvent(Ready)}` never
stops. Under the device-less probe harness the *same image* emits
`Ready { region_count: 3, manifest_generation: 6 }` immediately after
the `meta` registration — then dies in the harness frame loop with
`control I/O error: control socket closed`. So there are two linked
symptoms, environment-dependent; `01-evidence.md` has the side-by-side.

## The Ask

Make the workload reach and hold guest-sdk `Ready` under the real
`dh-worker` (not just the probe), so the M9 handoff can snapshot it.
That means: the agent must emit `Ready` after region registration + Start
under the real worker, and the harness frame loop must not tear down the
control socket it still needs. `02-repro.md` gives the exact
reproduction (no operator ROM needed — the synthetic 32 KiB game
triggers it) and what green looks like.

## Files

| File | Contents |
|---|---|
| `01-evidence.md` | Side-by-side probe vs real-worker event trails; ownership split |
| `02-repro.md` | Exact repro, the boot-probe tool, and the exit evidence step 2 needs |
