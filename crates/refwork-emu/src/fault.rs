//! Fault taxonomy (D9) and per-frame diagnostic flags.

/// A contract-relevant anomaly. Faults halt the core: once a fault is
/// recorded, `run_one_frame` returns immediately with
/// [`FrameFlags::FAULTED`] set and emulation never resumes (D9 — fail
/// loudly, never degrade).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// CPU fetched an opcode the core treats as a halt (`STP`).
    CpuStopped { pc: u32 },
    /// Write to an address with no mapped device or memory.
    UnmappedWrite { addr: u32, value: u8 },
    /// A PPU background mode outside the M2-implemented set was selected.
    UnimplementedBgMode { mode: u8 },
    /// An unimplemented PPU feature was enabled (register, value).
    UnimplementedPpuFeature { reg: u8, value: u8 },
    /// A channel was set in both MDMAEN ($420B) and HDMAEN ($420C) simultaneously.
    /// The general-DMA kick is rejected; the channel continues running as HDMA.
    HdmaDmaConflict { channels: u8 },
    /// Cartridge ROM/SRAM geometry violated at runtime.
    CartAccess { addr: u32 },
}

/// Per-frame diagnostic flags returned by `Core::run_one_frame`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameFlags(pub u32);

impl FrameFlags {
    /// The core is halted on a [`Fault`]; the frame was not (fully) emulated.
    pub const FAULTED: FrameFlags = FrameFlags(1 << 0);
    /// The stub APU's ports were accessed this frame (M1 stub — flagged so
    /// M2 acceptance can ban runs that relied on canned audio responses).
    pub const APU_STUB_ACCESS: FrameFlags = FrameFlags(1 << 1);
    /// The stub APU served a handshake-protocol transition this frame.
    pub const APU_STUB_HANDSHAKE: FrameFlags = FrameFlags(1 << 2);

    /// Returns true if every flag in `other` is set in `self`.
    pub fn contains(self, other: FrameFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Set all flags in `other`.
    pub fn insert(&mut self, other: FrameFlags) {
        self.0 |= other.0;
    }
}
