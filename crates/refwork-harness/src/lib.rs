#![forbid(unsafe_code)]

pub fn handshake_message() -> refwork_protocol::CtlMsg {
    refwork_protocol::CtlMsg::hello()
}
