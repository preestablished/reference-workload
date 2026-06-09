#![forbid(unsafe_code)]

pub const PROTO_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CtlMsg {
    Hello { proto_version: u32 },
    LoadRom { rom_hash: [u8; 32] },
    AdvanceFrames { frames: u32 },
}

impl CtlMsg {
    pub fn hello() -> Self {
        Self::Hello {
            proto_version: PROTO_VERSION,
        }
    }
}
