# 02 — `refwork-protocol`: the §3.1 control protocol crate

**Replaces** the stub at `crates/refwork-protocol/src/lib.rs` (placeholder
`CtlMsg { Hello{proto_version: u32}, LoadRom, AdvanceFrames }` — all three
variants are wrong vs spec; delete). The only current consumer is
`refwork-harness::handshake_message()` (itself a stub) — update it in the same
change.

## Deliverables

Exactly the API.md §3.1 wire surface, copied field-for-field (this enum is a
shared contract vendored into guest-sdk — naming and field order must match
the doc):

```rust
pub const PROTO_VERSION: u16 = 1;   // §3.1: "proto_version: u16 = 1"

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CtlMsg {
    // agent → harness
    Hello        { proto_version: u16 },
    LoadGame     { dev_path: String },
    Start        {},
    HashRequest  { frame: u64 },
    Shutdown     {},

    // harness → agent
    HelloAck     { proto_version: u16, emu: String, emu_version: String },
    GameLoaded   { cart_hash: [u8; 32], mapper: String, sram_size: u32 },
    RegisterRegion { name: String, gva: u64, len: u64, writable: bool },
    Ready        { frame: u64 },
    HashReport   { frame: u64, wram: [u8; 32], fb: [u8; 32] },
    Fault        { frame: u64, code: FaultCode, detail: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultCode { BadProto, BadGame, RegionRegFailed, EmuHalt, ProtocolOrder }
```

Notes pinned by the doc (cite in module docs, do not restate normatively):
- One datagram = one postcard-serialized `CtlMsg`, ≤ 4096 B except
  `RegisterRegion` page lists (§3.1). The current `RegisterRegion` shape is a
  single `{gva, len}` span; the page-list variant is the agent's hypervisor
  leg, not this enum — leave as specified.
- Ordering/state machine (§3.2) is enforced by the **harness** (M3), not by
  this crate. This crate is types + encoding only.
- Empty-struct variants `Start {}` / `Shutdown {}` keep braces to match the
  doc (postcard encodes them identically to unit variants; the doc shape wins
  for cross-repo grep-ability).

## Encoding & helpers

- `postcard` (default features **plus** `use-std`, a non-default feature, for
  `to_stdvec`) + `serde`.
- `pub fn encode(msg: &CtlMsg) -> Result<Vec<u8>, postcard::Error>` and
  `pub fn decode(bytes: &[u8]) -> Result<CtlMsg, postcard::Error>` — thin
  wrappers so harness/mock-agent code shares one entry point.
- `pub const MAX_DATAGRAM: usize = 4096;` and `encode` returns a typed error
  (an `EncodeError` enum wrapping `postcard::Error` | `Oversize { len }`)
  when the encoded size exceeds it for non-`RegisterRegion` messages.
  Decision made here: a typed error, NOT `debug_assert` — debug asserts
  vanish in release builds, which is exactly where the M3 harness runs.

## Determinism posture

This crate compiles into the guest harness binary:
- keep `#![forbid(unsafe_code)]`;
- no std::thread/clock/rand/float/HashMap tokens (the deny gate will scan this
  crate after package 04);
- `String` fields are fine (allocation at setup time, not in the frame loop —
  ARCHITECTURE.md §3 sends no per-frame messages).

## Tests

- Postcard round-trip for **every** variant (construct → encode → decode →
  assert equal), including 32-byte hash arrays and a `RegisterRegion` with a
  realistic 128 KiB region.
- **Golden-bytes table covering EVERY variant**: a table-driven test with one
  fixed input value and its exact expected byte vector per `CtlMsg` variant
  (all 11) and per `FaultCode` value (all 5, encoded inside a `Fault`
  message and standalone). A single-variant golden does NOT freeze the wire
  format: postcard encodes the variant *index* as the discriminant, so
  swapping two later variants leaves `Hello`'s bytes untouched while
  silently breaking guest-sdk interop — only per-variant goldens catch
  reordering anywhere in the enum. Comment in the source: variant ORDER is
  wire-significant under postcard; never reorder or insert, only append.
- Size discipline: every non-`RegisterRegion` variant with plausible max
  payloads (e.g. 256-char `detail`) encodes ≤ 4096 B.
- `decode` of truncated/garbage bytes returns Err (no panic).

## Consumer updates in the same change

- `crates/refwork-harness/src/lib.rs`: `handshake_message()` →
  `CtlMsg::HelloAck { proto_version: PROTO_VERSION, emu: "refwork-emu".into(),
  emu_version: refwork_emu::EMU_VERSION.into() }`? — NO: the harness SENDS
  `HelloAck` in response to `Hello`; keep the stub minimal but truthful:
  return `CtlMsg::Hello { proto_version: PROTO_VERSION }` is the AGENT's
  message. Simplest correct placeholder until M3: replace
  `handshake_message()` with `pub fn hello_ack() -> CtlMsg` returning the
  harness-side `HelloAck` (it needs `refwork-emu`'s `EMU_VERSION`, adding a
  dependency edge harness→emu that M3 needs anyway — acceptable now, or keep
  the emu string a parameter to avoid the edge until M3; prefer the
  parameter form: `pub fn hello_ack(emu: &str, emu_version: &str) -> CtlMsg`).
