#![deny(unsafe_op_in_unsafe_fn)]

use refwork_protocol::{CtlMsg, PROTO_VERSION};

pub mod meta;
pub mod regions;

/// Build the harness-side handshake reply message.
///
/// Returns `CtlMsg::HelloAck` populated with the current protocol version and
/// the supplied emulator identity strings.  The caller supplies `emu` and
/// `emu_version` as parameters to avoid a compile-time dependency on
/// `refwork-emu`; that dependency edge will be added in M3 when the harness
/// gains its full state machine.
///
/// See API.md §3.1 and §3.2 for the handshake state machine.
pub fn hello_ack(emu: &str, emu_version: &str) -> CtlMsg {
    CtlMsg::HelloAck {
        proto_version: PROTO_VERSION,
        emu: emu.into(),
        emu_version: emu_version.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use refwork_protocol::PROTO_VERSION;

    #[test]
    fn hello_ack_contains_correct_proto_version() {
        let msg = hello_ack("refwork-emu", "0.1.0");
        match msg {
            CtlMsg::HelloAck {
                proto_version,
                emu,
                emu_version,
            } => {
                assert_eq!(proto_version, PROTO_VERSION);
                assert_eq!(emu, "refwork-emu");
                assert_eq!(emu_version, "0.1.0");
            }
            other => panic!("expected HelloAck, got {:?}", other),
        }
    }

    #[test]
    fn hello_ack_roundtrips_via_protocol() {
        let msg = hello_ack("test-emu", "1.2.3");
        let bytes = refwork_protocol::encode(&msg).expect("encode");
        let decoded = refwork_protocol::decode(&bytes).expect("decode");
        assert_eq!(msg, decoded);
    }
}
