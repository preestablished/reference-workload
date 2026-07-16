# Positive Notes

- `crates/refwork-harness/src/meta.rs:7` preserves the API.md 3.6 byte layout as named constants, which keeps later offset changes reviewable.
- `crates/refwork-harness/src/meta.rs:30` zero-fills the whole meta page before writing version/status, avoiding stale reserved bytes in a published page.
- `crates/refwork-harness/src/meta.rs:127` through `crates/refwork-harness/src/meta.rs:185` tests the concrete byte offsets, padding, truncation, and fault-code mapping. Keep these as the guardrail for guest-sdk and host readers.
- `crates/refwork-harness/src/regions.rs:74` rejects zero-length and non-page-multiple regions before allocation, matching the API.md 3.5 length discipline.
- `crates/refwork-harness/src/regions.rs:223` documents the explicit lifetime widening in the `emu_buffers` unsafe contract instead of hiding the `'static` conversion.
- `crates/refwork-harness/src/regions.rs:209` emits required regions first and optional regions only when configured, matching the intended `wram`, `framebuffer`, `meta`, optional `vram`/`sram` vocabulary.
- `crates/refwork-harness/src/main.rs:19` is honest that the fd-3 loop is not implemented yet, which avoids leaking broader package-02 behavior into this bead.
