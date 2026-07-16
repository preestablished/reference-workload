# Package 07 — Handoff Surface (Item 6)

**Gate:** any GATE-RECORD branch; runnable once package 04 (or, fallback,
package 03's reduced pass) has produced its evidence — the handoff should
describe a validated image, not a hoped-for one.

## Decision: registration vs manifest-only

Check whether control-plane's `WorkloadImage` resource API exists **at
execution time** (their repo/API.md — do not rely on this plan's snapshot):

- **Exists:** register the workload image; record the resource id and the
  registration evidence. Validate jointly that what the control-plane serves
  back matches the `dist/` manifest hashes.
- **Doesn't exist:** ship the manifest + `dist/` layout the hypervisor
  consumes directly (current layout: `dist/workload-image-0.1.0/` —
  `workload-image.yaml`, `boot.toml`, `harness.toml`, `expected-regions.toml`,
  `bzImage`, `initramfs.cpio.zst`, `determinism.last_green`), and record the
  registration as control-plane's follow-on in the resolution — a named
  follow-on, not a silent omission.

## Contents check before shipping either way

- Manifest hashes match the actual `dist/` files.
- `determinism.last_green` reflects the build actually smoked in package 06
  (same image id) — if package 06 ran a newer image than `dist/` holds,
  reconcile before handoff; never hand off an image that isn't the one the
  evidence covers.
- The scoring-relevant references (feature-map/scoring-program refs, if the
  manifest carries them) point at the real 20v pair identifiers, not the
  placeholder demo pair.

## Exit Signal

Disposition recorded (registered + id, or manifest+dist + follow-on note),
image id consistent with the smoke evidence.
