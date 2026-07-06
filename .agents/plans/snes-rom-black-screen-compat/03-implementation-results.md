# Implementation results: private ROM reaches nonblack frames

This records the implementation pass from `02-reviewed-implementation-plan.md`.
The original black-screen failure is resolved for the private ROM under the
clean-room diagnostic harness.

## Changes implemented

- Replaced the first attempted APU lead-window service with CPU-access-scoped
  pending IPL handoff service:
  - preserves the initial port-0 `$CC` kick until the SPC-side IPL consumes it;
  - defers transfer-loop service across an immediately following non-port-0
    write so port-0 strobes are not consumed before their paired data port;
  - avoids accumulating artificial APU lead while still keeping normal
    catch-up behavior deterministic.
- Added a clean-room HLE implementation of the documented SPC IPL upload
  protocol for production execution:
  - nonzero command byte on port 1 starts a transfer;
  - first transfer byte uses zero-based index `$00`;
  - block termination / jump is a port-0 value at least two greater than the
    previous index;
  - direct-page `$00/$01` mirror the IPL pointer;
  - final jump initializes A/X/Y/PSW consistently with documented IPL handoff.
- Corrected `$F1` port-clear semantics so PC10/PC32 clear only CPU-to-SPC input
  latches (`spc_ports`), not SPC-to-CPU output latches (`cpu_ports`).
- Rewrote the synthetic ROM's APU upload subroutine to the real zero-based IPL
  protocol.
- Expanded `rom_diag` with clean-room-safe fields: brightness, BG mode, TM/TS,
  compositor registers, VRAM/OAM/CGRAM counts, distinct nonzero CGRAM colors,
  aggregate render counters, frame hash/nonzero count, post-`$CC` timing, IPL
  load/jump addresses, IPL byte/block counts, and SPC port / I/O read-write
  counts.
- Fixed PPU color math source selection for `CGWSEL` bit 1: bit 1 clear uses
  fixed-color math rather than the sub-screen layer. Added a regression test for
  the failure mode where the same layer is enabled on both main and sub screens
  and subtract math would otherwise cancel every pixel to black.

## Real ROM diagnostic state

Fresh `rom_diag` runs against the private ROM show:

- IPL upload completes: `spc_in_ipl=false`, first load `0x0d10`, last load
  `0x02c0`, jump `0x02c0`, blocks `2`, bytes `2624`.
- At frame 120: `force_blank=false`, `brightness=15`, `bgmode=1`, `tm=0x04`,
  `ts=0x04`, `cgram_nz=58`, `vram_nz=3925`, `cgram_colors=24`,
  `renderFinal=1680`, `fb_nz=5040`.
- At frame 240: rendering remains nonblack with `renderFinal=3087` and
  `fb_nz=9261`.
- The ROM later force-blanks intentionally while reconfiguring display state
  around frames 333-360, then resumes nonblack rendering:
  - frame 480: `force_blank=false`, `tm=0x01`, `ts=0x01`,
    `renderFinal=1615`, `fb_nz=4458`;
  - frame 600: `force_blank=false`, `renderFinal=1615`, `fb_nz=4428`.
- Repeated diagnostic runs produced the same frame-120 hash
  `d53c133780b8b772` for the completed framebuffer.

All values above are clean-room-safe booleans, counts, register/address values,
and hashes only.

## Verification performed

- `cargo fmt` passes.
- `cargo test -p refwork-emu --features introspect` passes: 181 tests.
- `cargo test -p refwork-emu` passes: 180 tests.
- `cargo test -p xtask --test determinism` passes (`determinism_600_frames`;
  `determinism_10000_frames` remains ignored as before).
- `rom_diag` against the private ROM passes the acceptance signals through 600
  frames: force blank clears, brightness is positive, CGRAM/VRAM populate, the
  SPC leaves IPL, and completed framebuffers become nonblack deterministically.

## Remaining external gates

`linux_m5`, image double-builds, package checks, and cross-architecture gates
were not run in this pass. The code-level and private-ROM diagnostic criteria
from this plan are met locally; those external gates remain separate release
validation work.
