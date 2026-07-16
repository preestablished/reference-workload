# Positive Notes

- `crates/refwork-harness/src/meta.rs:7` through `crates/refwork-harness/src/meta.rs:14` match the canonical API `meta` layout offsets: version at `0x00`, status at `0x04`, frame at `0x08`, last pad at `0x10`, reserved at `0x12`, fault code at `0x14`, cart hash at `0x18`, and emulator version at `0x38`.

- `crates/refwork-harness/src/meta.rs:127` through `crates/refwork-harness/src/meta.rs:185` are good byte-level tests for the shared page. Keep this style because guest-sdk, hypervisor capture, and observability tools all depend on exact offsets rather than Rust types.

- `crates/refwork-harness/src/regions.rs:7` through `crates/refwork-harness/src/regions.rs:10` define the expected region sizes in one place, and `crates/refwork-harness/src/regions.rs:185` through `crates/refwork-harness/src/regions.rs:190` use those constants for the required regions. The framebuffer size comes from `refwork_emu::FB_BYTES`, which avoids duplicating the 229376-byte geometry.

- `crates/refwork-harness/src/regions.rs:74` through `crates/refwork-harness/src/regions.rs:85` correctly reject zero-sized and non-page-multiple regions before allocation, then request a 4096-byte-aligned allocation.

- `crates/refwork-harness/src/regions.rs:209` through `crates/refwork-harness/src/regions.rs:220` keep descriptor order deterministic: required regions first, optional VRAM/SRAM after. The tests at `crates/refwork-harness/src/regions.rs:295` and `crates/refwork-harness/src/regions.rs:311` preserve that behavior.

- `crates/refwork-harness/src/main.rs:1` through `crates/refwork-harness/src/main.rs:10` keep the placeholder binary deterministic and explicit: help succeeds, unknown arguments exit with status 2, and no production fd-3 behavior is silently pretended to exist.
