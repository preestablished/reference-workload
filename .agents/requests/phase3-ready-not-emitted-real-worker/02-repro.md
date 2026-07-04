# Reproduction And What Green Looks Like

No operator ROM needed — the synthetic 32 KiB game triggers all of this.

## Probe (fast, no worker) — shows symptom 2 and that Ready *can* emit

```sh
cd ~/git/preestablished/reference-workload && cargo run -q --locked -p xtask -- image build
cd ~/git/preestablished/guest-sdk
zstd -qf -d -o /tmp/init.cpio \
  ~/git/preestablished/reference-workload/dist/workload-image-0.1.0/initramfs.cpio.zst
# A 512-aligned game image; any 32 KiB blob works for the boot path:
head -c 32768 /dev/zero > /tmp/game.img
BOOT_PROBE_INITRAMFS=/tmp/init.cpio BOOT_PROBE_GAME=/tmp/game.img \
  BOOT_PROBE_SECS=60 cargo test -p detguest-vmtest --test boot_probe -- --nocapture
```

Last events: `Ready { region_count: 3 }` then `frame loop failed:
control socket closed` → `WorkloadExited exit 1`. (`boot_probe.rs` +
`BOOT_PROBE_GAME` were added by guest-sdk `322c331`.)

## Real worker (the actual gate) — shows symptom 1

The full M9 handoff on scratch paths (ops doc
`~/git/preestablished/determinism-hypervisor/docs/ops/rom-bridge-o73-ready-snapshot.md`;
the bridge session has the exact scratch invocation and a built
`dh-m9-ready-handoff` at determinism-hypervisor `44c44f5`). With
`DH_M9_GAME_IMAGE` staged (512-aligned), it now prints the per-region
event trail from `01-evidence.md` and stops at HARD_CAP with no Ready.
Ping the bridge session to run it against a candidate fix; we own that
scratch environment and will turn it around quickly.

## What Green Looks Like

1. A refwork/guest-sdk VM-tier test that boots the **real harness**
   (not the `m9_refwork_contract` fixture) through to a *held* guest-sdk
   `Ready` and past the first frame boundary, under a device set that
   matches the real worker (real pv-pad, real pv-blk) — i.e. the frame
   loop must not tear down the control socket it needs, and the agent
   must emit and hold Ready. Negative-tested per convention.
2. The bridge-run real-worker handoff reaches `Ready` and snapshots:
   the step-2 exit evidence (READY icount ≈ 643 M, region count 3 /
   manifest_generation 6, state hash) lands in the handoff's public
   summary — unblocking READY-snapshot regeneration (step 3), the
   `BRIDGE_REAL_SNAPSHOT_REF` cutover (step 4), and the first real frame
   in the browser.

## Handback

`03-resolution.md` here per the series convention. We re-run the probe
and the real-worker handoff and answer with `04-verification.md`. The
adoption (`game_source`, lock `322c331`) is already on `main`
(`cdcb372`); a further lock bump for the fix is a one-liner on our side.
