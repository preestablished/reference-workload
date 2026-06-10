//! `refwork-protocol` — harness ↔ agent control protocol types and encoding.
//!
//! This crate implements the shared `CtlMsg`/`FaultCode` wire surface defined in
//! API.md §3.1. One postcard datagram = one `CtlMsg`, ≤ [`MAX_DATAGRAM`] bytes
//! except for `RegisterRegion` page-list messages (API.md §3.1 exemption).
//!
//! **Determinism posture:** this crate is compiled into the guest harness binary
//! and inherits the determinism deny-list (ARCHITECTURE.md D1–D4): no unsafe code,
//! no OS threads, no clocks, no randomness, no floating-point, no HashMap.
//! String fields are allocated only at setup time (never in the per-frame loop).
//!
//! **Wire stability:** variant ORDER is wire-significant under postcard (the
//! discriminant is the variant index). Never reorder or insert variants; only
//! append new variants at the end of each section. API.md §3.1 is authoritative.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// Wire protocol version exchanged in `Hello`/`HelloAck` (API.md §3.1).
pub const PROTO_VERSION: u16 = 1;

/// Maximum datagram size in bytes for all messages except `RegisterRegion`
/// page lists (API.md §3.1 exemption).
pub const MAX_DATAGRAM: usize = 4096;

// ──────────────────────────────────────────────────────────────────────────────
// Fault codes
// ──────────────────────────────────────────────────────────────────────────────

/// Fault reason codes reported in `CtlMsg::Fault` (API.md §3.1).
///
/// WIRE STABILITY: variant ORDER is the postcard discriminant.
/// Never reorder or insert; only append.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultCode {
    BadProto,        // 0
    BadGame,         // 1
    RegionRegFailed, // 2
    EmuHalt,         // 3
    ProtocolOrder,   // 4
}

// ──────────────────────────────────────────────────────────────────────────────
// Control message enum
// ──────────────────────────────────────────────────────────────────────────────

/// Harness ↔ agent control messages (API.md §3.1).
///
/// WIRE STABILITY: variant ORDER is the postcard discriminant (varint index).
/// Never reorder or insert variants into this enum; only append new variants
/// at the end of the appropriate section. Cross-repo consumers (guest-sdk)
/// depend on the discriminant values being stable across crate versions.
///
/// Ordering and state-machine rules are enforced by the harness (M3), not here.
/// This crate is types + encoding only.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CtlMsg {
    // ── agent → harness ─────────────────────────────────────────────────────
    /// Agent opens the handshake. Discriminant = 0.
    Hello { proto_version: u16 },
    /// Agent requests ROM load from block device path. Discriminant = 1.
    LoadGame { dev_path: String },
    /// Agent signals readiness to enter the free-running frame loop. Discriminant = 2.
    Start {},
    /// Agent requests a state hash at a specific frame (suite mode). Discriminant = 3.
    HashRequest { frame: u64 },
    /// Agent requests graceful shutdown. Discriminant = 4.
    Shutdown {},

    // ── harness → agent ─────────────────────────────────────────────────────
    /// Harness acknowledges the handshake. Discriminant = 5.
    HelloAck {
        proto_version: u16,
        emu: String,
        emu_version: String,
    },
    /// Harness confirms ROM load with metadata. Discriminant = 6.
    GameLoaded {
        cart_hash: [u8; 32],
        mapper: String,
        sram_size: u32,
    },
    /// Harness publishes a memory region to the agent for hypervisor registration.
    /// Exempt from the MAX_DATAGRAM size limit (API.md §3.1). Discriminant = 7.
    RegisterRegion {
        name: String,
        gva: u64,
        len: u64,
        writable: bool,
    },
    /// Harness confirms all regions registered; frame loop may begin. Discriminant = 8.
    Ready { frame: u64 },
    /// Harness delivers a determinism-suite hash report. Discriminant = 9.
    HashReport {
        frame: u64,
        wram: [u8; 32],
        fb: [u8; 32],
    },
    /// Harness reports an unrecoverable fault. Discriminant = 10.
    Fault {
        frame: u64,
        code: FaultCode,
        detail: String,
    },
}

// ──────────────────────────────────────────────────────────────────────────────
// Encoding error
// ──────────────────────────────────────────────────────────────────────────────

/// Error returned by [`encode`].
///
/// `RegisterRegion` messages are exempt from the [`MAX_DATAGRAM`] size check
/// and will never produce `Oversize` (API.md §3.1).
#[derive(Debug)]
pub enum EncodeError {
    /// Postcard serialization failed.
    Postcard(postcard::Error),
    /// Encoded size exceeds [`MAX_DATAGRAM`] for a non-exempt message.
    Oversize { len: usize },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EncodeError::Postcard(e) => write!(f, "postcard encode error: {e}"),
            EncodeError::Oversize { len } => write!(
                f,
                "encoded message size {len} exceeds MAX_DATAGRAM ({MAX_DATAGRAM})"
            ),
        }
    }
}

impl std::error::Error for EncodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncodeError::Postcard(e) => Some(e),
            EncodeError::Oversize { .. } => None,
        }
    }
}

impl From<postcard::Error> for EncodeError {
    fn from(e: postcard::Error) -> Self {
        EncodeError::Postcard(e)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Encode / decode helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Serialize a [`CtlMsg`] to a `Vec<u8>` using the postcard wire format.
///
/// Returns [`EncodeError::Oversize`] if the serialized length exceeds
/// [`MAX_DATAGRAM`] and the message is not a `RegisterRegion`
/// (which is exempt per API.md §3.1).
pub fn encode(msg: &CtlMsg) -> Result<Vec<u8>, EncodeError> {
    let bytes = postcard::to_stdvec(msg)?;
    let exempt = matches!(msg, CtlMsg::RegisterRegion { .. });
    if !exempt && bytes.len() > MAX_DATAGRAM {
        return Err(EncodeError::Oversize { len: bytes.len() });
    }
    Ok(bytes)
}

/// Deserialize a [`CtlMsg`] from a postcard-encoded byte slice.
pub fn decode(bytes: &[u8]) -> Result<CtlMsg, postcard::Error> {
    postcard::from_bytes(bytes)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn rt(msg: CtlMsg) {
        let bytes = encode(&msg).expect("encode");
        let got = decode(&bytes).expect("decode");
        assert_eq!(msg, got, "round-trip mismatch for {:?}", msg);
    }

    // ── round-trip: every variant ────────────────────────────────────────────

    #[test]
    fn round_trip_hello() {
        rt(CtlMsg::Hello {
            proto_version: PROTO_VERSION,
        });
    }

    #[test]
    fn round_trip_load_game() {
        rt(CtlMsg::LoadGame {
            dev_path: "/dev/vdb".into(),
        });
    }

    #[test]
    fn round_trip_start() {
        rt(CtlMsg::Start {});
    }

    #[test]
    fn round_trip_hash_request() {
        rt(CtlMsg::HashRequest {
            frame: 0xDEAD_BEEF_1234_5678,
        });
    }

    #[test]
    fn round_trip_shutdown() {
        rt(CtlMsg::Shutdown {});
    }

    #[test]
    fn round_trip_hello_ack() {
        rt(CtlMsg::HelloAck {
            proto_version: PROTO_VERSION,
            emu: "refwork-emu".into(),
            emu_version: "0.1.0".into(),
        });
    }

    #[test]
    fn round_trip_game_loaded() {
        let hash = [0xABu8; 32];
        rt(CtlMsg::GameLoaded {
            cart_hash: hash,
            mapper: "mmc3".into(),
            sram_size: 8192,
        });
    }

    #[test]
    fn round_trip_register_region() {
        // Realistic 128 KiB region (WRAM)
        rt(CtlMsg::RegisterRegion {
            name: "wram".into(),
            gva: 0x0001_0000,
            len: 131_072,
            writable: false,
        });
    }

    #[test]
    fn round_trip_ready() {
        rt(CtlMsg::Ready { frame: 0 });
    }

    #[test]
    fn round_trip_hash_report() {
        let wram = [0x11u8; 32];
        let fb = [0x22u8; 32];
        rt(CtlMsg::HashReport {
            frame: 42,
            wram,
            fb,
        });
    }

    #[test]
    fn round_trip_fault_all_codes() {
        for code in [
            FaultCode::BadProto,
            FaultCode::BadGame,
            FaultCode::RegionRegFailed,
            FaultCode::EmuHalt,
            FaultCode::ProtocolOrder,
        ] {
            rt(CtlMsg::Fault {
                frame: 1,
                code,
                detail: "test".into(),
            });
        }
    }

    // ── golden-bytes table ────────────────────────────────────────────────────
    //
    // Postcard wire format (postcard 1.x, verified by running cargo test):
    //   - Enum discriminant: varint (ULEB128); variant index 0-127 = one byte.
    //   - u8:    1 byte, raw.
    //   - u16/u32/u64: varint-encoded (NOT little-endian fixed-width).
    //     e.g. u16=1 → [0x01], u64=0 → [0x00], u64=4096 encoded as two
    //     continuation bytes since 4096 > 127.
    //   - bool:  1 byte (0 = false, 1 = true).
    //   - [u8;N]: N raw bytes, no length prefix (fixed-size arrays are NOT
    //     varint-length-prefixed; e.g. [u8;32] is always exactly 32 bytes).
    //   - String/&str: varint byte-length then UTF-8 bytes.
    //   - Empty struct `{}`: zero payload bytes (just the discriminant).
    //
    // Varint (ULEB128) encoding used by postcard for integer fields:
    //   values 0–127 fit in one byte.
    //   4096 = 0x1000: low 7 bits = 0, continuation bit set → 0x80;
    //                  next 7 bits = 0x20 → 0x20. So 4096 → [0x80, 0x20].
    //
    // One fixed input value per variant; expected bytes verified by running.
    // A per-variant golden catches reordering anywhere in the enum that a
    // single-variant golden would miss (reordering changes discriminants).

    fn golden(msg: &CtlMsg, expected: &[u8]) {
        let bytes = encode(msg).expect("encode");
        assert_eq!(
            bytes, expected,
            "golden mismatch for variant {:?}\n  got:      {:02x?}\n  expected: {:02x?}",
            msg, bytes, expected
        );
        // also verify round-trips correctly
        let got = decode(&bytes).expect("decode");
        assert_eq!(*msg, got);
    }

    #[test]
    fn golden_bytes_hello() {
        // discriminant=0 [0x00], proto_version=1 as varint [0x01]
        golden(&CtlMsg::Hello { proto_version: 1 }, &[0x00, 0x01]);
    }

    #[test]
    fn golden_bytes_load_game() {
        // discriminant=1 [0x01], "/dev/vdb" len=8 as varint [0x08], then bytes
        golden(
            &CtlMsg::LoadGame {
                dev_path: "/dev/vdb".into(),
            },
            &[0x01, 0x08, b'/', b'd', b'e', b'v', b'/', b'v', b'd', b'b'],
        );
    }

    #[test]
    fn golden_bytes_start() {
        // discriminant=2 [0x02], no payload
        golden(&CtlMsg::Start {}, &[0x02]);
    }

    #[test]
    fn golden_bytes_hash_request() {
        // discriminant=3 [0x03], frame=1 as varint [0x01]
        golden(&CtlMsg::HashRequest { frame: 1 }, &[0x03, 0x01]);
    }

    #[test]
    fn golden_bytes_shutdown() {
        // discriminant=4 [0x04], no payload
        golden(&CtlMsg::Shutdown {}, &[0x04]);
    }

    #[test]
    fn golden_bytes_hello_ack() {
        // discriminant=5 [0x05]
        // proto_version=1 as varint [0x01]
        // emu="emu" (3 bytes): len=3 [0x03], b'e' b'm' b'u'
        // emu_version="1" (1 byte): len=1 [0x01], b'1'
        golden(
            &CtlMsg::HelloAck {
                proto_version: 1,
                emu: "emu".into(),
                emu_version: "1".into(),
            },
            &[0x05, 0x01, 0x03, b'e', b'm', b'u', 0x01, b'1'],
        );
    }

    #[test]
    fn golden_bytes_game_loaded() {
        // discriminant=6 [0x06]
        // cart_hash=[0x00;32]: 32 raw bytes (fixed-size array, no length prefix)
        // mapper="mmc3" (4 bytes): len=4 [0x04], b'm' b'm' b'c' b'3'
        // sram_size=0 as varint [0x00]
        let mut expected = vec![0x06u8];
        expected.extend_from_slice(&[0u8; 32]); // cart_hash (fixed array, 32 bytes)
        expected.extend_from_slice(&[0x04, b'm', b'm', b'c', b'3']); // mapper
        expected.push(0x00); // sram_size=0 as varint
        golden(
            &CtlMsg::GameLoaded {
                cart_hash: [0u8; 32],
                mapper: "mmc3".into(),
                sram_size: 0,
            },
            &expected,
        );
    }

    #[test]
    fn golden_bytes_register_region() {
        // discriminant=7 [0x07]
        // name="wram" (4 bytes): len=4 [0x04], b'w' b'r' b'a' b'm'
        // gva=0 as varint [0x00]
        // len=4096 as varint: 4096=0x1000, ULEB128 → [0x80, 0x20]
        //   (low 7 bits of 4096 = 0, set continuation → 0x80;
        //    shift right 7: 4096>>7 = 32 = 0x20, fits in 7 bits → 0x20)
        // writable=false [0x00]
        golden(
            &CtlMsg::RegisterRegion {
                name: "wram".into(),
                gva: 0,
                len: 4096,
                writable: false,
            },
            &[0x07, 0x04, b'w', b'r', b'a', b'm', 0x00, 0x80, 0x20, 0x00],
        );
    }

    #[test]
    fn golden_bytes_ready() {
        // discriminant=8 [0x08], frame=0 as varint [0x00]
        golden(&CtlMsg::Ready { frame: 0 }, &[0x08, 0x00]);
    }

    #[test]
    fn golden_bytes_hash_report() {
        // discriminant=9 [0x09]
        // frame=0 as varint [0x00]
        // wram=[0x00;32]: 32 raw bytes (fixed-size array, no length prefix)
        // fb=[0x00;32]: 32 raw bytes
        let mut expected = vec![0x09u8];
        expected.push(0x00); // frame=0 as varint
        expected.extend_from_slice(&[0x00u8; 32]); // wram (fixed array)
        expected.extend_from_slice(&[0x00u8; 32]); // fb (fixed array)
        golden(
            &CtlMsg::HashReport {
                frame: 0,
                wram: [0u8; 32],
                fb: [0u8; 32],
            },
            &expected,
        );
    }

    #[test]
    fn golden_bytes_fault_bad_proto() {
        // discriminant=10 [0x0A]
        // frame=0 as varint [0x00]
        // code=BadProto discriminant=0 as varint [0x00]
        // detail="" len=0 as varint [0x00]
        golden(
            &CtlMsg::Fault {
                frame: 0,
                code: FaultCode::BadProto,
                detail: String::new(),
            },
            &[0x0A, 0x00, 0x00, 0x00],
        );
    }

    #[test]
    fn golden_bytes_fault_bad_game() {
        // code=BadGame discriminant=1 [0x01]
        golden(
            &CtlMsg::Fault {
                frame: 0,
                code: FaultCode::BadGame,
                detail: String::new(),
            },
            &[0x0A, 0x00, 0x01, 0x00],
        );
    }

    #[test]
    fn golden_bytes_fault_region_reg_failed() {
        // code=RegionRegFailed discriminant=2 [0x02]
        golden(
            &CtlMsg::Fault {
                frame: 0,
                code: FaultCode::RegionRegFailed,
                detail: String::new(),
            },
            &[0x0A, 0x00, 0x02, 0x00],
        );
    }

    #[test]
    fn golden_bytes_fault_emu_halt() {
        // code=EmuHalt discriminant=3 [0x03]
        golden(
            &CtlMsg::Fault {
                frame: 0,
                code: FaultCode::EmuHalt,
                detail: String::new(),
            },
            &[0x0A, 0x00, 0x03, 0x00],
        );
    }

    #[test]
    fn golden_bytes_fault_protocol_order() {
        // code=ProtocolOrder discriminant=4 [0x04]
        golden(
            &CtlMsg::Fault {
                frame: 0,
                code: FaultCode::ProtocolOrder,
                detail: String::new(),
            },
            &[0x0A, 0x00, 0x04, 0x00],
        );
    }

    #[test]
    fn golden_bytes_fault_code_standalone() {
        // All FaultCode values encoded standalone via postcard::to_stdvec.
        // Each unit variant encodes as its 1-byte varint discriminant index.
        let cases: &[(FaultCode, u8)] = &[
            (FaultCode::BadProto, 0),
            (FaultCode::BadGame, 1),
            (FaultCode::RegionRegFailed, 2),
            (FaultCode::EmuHalt, 3),
            (FaultCode::ProtocolOrder, 4),
        ];
        for &(code, disc) in cases {
            let bytes = postcard::to_stdvec(&code).expect("encode FaultCode");
            assert_eq!(
                bytes,
                vec![disc],
                "FaultCode::{:?} standalone: expected [{disc:#04x}], got {:02x?}",
                code,
                bytes
            );
            let got: FaultCode = postcard::from_bytes(&bytes).expect("decode FaultCode");
            assert_eq!(got, code);
        }
    }

    // ── size discipline ───────────────────────────────────────────────────────

    #[test]
    fn size_discipline_all_non_register_region_variants() {
        // A 256-char detail string (plausible maximum fault message) must still
        // fit within MAX_DATAGRAM for every non-RegisterRegion variant.
        let long_str: String = "x".repeat(256);
        let long_path: String = "/dev/".to_string() + &"x".repeat(251);

        let msgs = vec![
            CtlMsg::Hello {
                proto_version: PROTO_VERSION,
            },
            CtlMsg::LoadGame {
                dev_path: long_path,
            },
            CtlMsg::Start {},
            CtlMsg::HashRequest { frame: u64::MAX },
            CtlMsg::Shutdown {},
            CtlMsg::HelloAck {
                proto_version: PROTO_VERSION,
                emu: long_str.clone(),
                emu_version: long_str.clone(),
            },
            CtlMsg::GameLoaded {
                cart_hash: [0xFFu8; 32],
                mapper: long_str.clone(),
                sram_size: u32::MAX,
            },
            CtlMsg::Ready { frame: u64::MAX },
            CtlMsg::HashReport {
                frame: u64::MAX,
                wram: [0xFFu8; 32],
                fb: [0xFFu8; 32],
            },
            CtlMsg::Fault {
                frame: u64::MAX,
                code: FaultCode::ProtocolOrder,
                detail: long_str.clone(),
            },
        ];

        for msg in &msgs {
            let bytes = postcard::to_stdvec(msg).expect("encode");
            assert!(
                bytes.len() <= MAX_DATAGRAM,
                "variant {:?} with 256-char strings encoded to {} bytes (> MAX_DATAGRAM={})",
                msg,
                bytes.len(),
                MAX_DATAGRAM
            );
        }
    }

    #[test]
    fn size_discipline_register_region_exempt() {
        // RegisterRegion is exempt from MAX_DATAGRAM; encode() must not return Oversize.
        let large_name = "r".repeat(4096);
        let msg = CtlMsg::RegisterRegion {
            name: large_name,
            gva: 0,
            len: 0x1000,
            writable: false,
        };
        encode(&msg).expect("RegisterRegion must not return EncodeError::Oversize");
    }

    // ── robustness: truncated / garbage input ─────────────────────────────────

    #[test]
    fn decode_truncated_returns_err() {
        let msg = CtlMsg::GameLoaded {
            cart_hash: [0xABu8; 32],
            mapper: "test".into(),
            sram_size: 42,
        };
        let bytes = encode(&msg).expect("encode");
        // Truncate to every length from 0..len-1 and ensure no panic
        for len in 0..bytes.len() {
            let _ = decode(&bytes[..len]); // must not panic; error is fine
        }
    }

    #[test]
    fn decode_garbage_returns_err() {
        let garbage = [0xFFu8; 64];
        let result = decode(&garbage);
        assert!(
            result.is_err(),
            "expected Err for garbage input, got Ok({:?})",
            result.ok()
        );
    }

    #[test]
    fn decode_empty_returns_err() {
        let result = decode(&[]);
        assert!(result.is_err(), "expected Err for empty input");
    }

    #[test]
    fn decode_out_of_range_discriminant_returns_err() {
        // Discriminant 0x7F (127) is not a valid CtlMsg variant.
        let result = decode(&[0x7F, 0x00, 0x00]);
        assert!(
            result.is_err(),
            "expected Err for out-of-range discriminant"
        );
    }
}
