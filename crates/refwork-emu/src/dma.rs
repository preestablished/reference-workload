//! General (immediate) DMA and HDMA (per-scanline DMA).
//!
//! General DMA: 8 channels, $43x0-$43xA registers, kicked by MDMAEN ($420B).
//! HDMA: same channel register file; enabled per-channel by HDMAEN ($420C).
//! HDMA runs once per scanline (applied in h-blank before render); general DMA
//! runs in response to an MDMAEN kick and consumes the bus for its duration.
//!
//! HDMA table walk (frame init copies A1T → A2A; A2A is the rolling table
//! pointer): each entry starts with a count byte — $00 terminates the
//! channel for the frame, $01-$80 is 1-128 lines with one transfer on the
//! entry's first line only, $81-$FF is 1-127 lines transferring every line
//! (repeat). The live counter is the NTRL register ($43xA), decremented raw
//! each scanline; bits[6:0] reaching 0 loads the next entry.
//!
//! HDMA direct mode (DMAP bit 6 = 0): the pattern's data bytes follow the
//! count byte inline in the table; A2A advances past each transferred byte.
//! HDMA indirect mode (DMAP bit 6 = 1): the count byte is followed by a
//! 2-byte data address loaded into DAS; transfers read from (DASB:DAS),
//! advancing DAS.
//!
//! Channel conflict (same channel enabled in both MDMAEN and HDMAEN): fault (D9).
//!
//! OWNER (implementation): integration agent (executed from `SysBus`).

/// One DMA channel's register file.
#[derive(Debug, Clone, Copy, Default)]
pub struct DmaChannel {
    /// $43x0 DMAP: transfer unit pattern (bits 0-2), fixed-address (bit 3),
    /// A-bus decrement (bit 4), indirect HDMA (bit 6), direction B→A (bit 7).
    pub dmap: u8,
    /// $43x1 BBAD: B-bus address ($21xx low byte).
    pub bbad: u8,
    /// $43x2/3 A1T: A-bus address (HDMA table pointer in direct mode;
    /// indirect: table pointer for the (count, addr) tuples).
    pub a1t: u16,
    /// $43x4 A1B: A-bus bank.
    pub a1b: u8,
    /// $43x5/6 DAS: byte count for general DMA (0 = 65536); indirect HDMA
    /// data address (lo/hi bytes) — overloaded meaning.
    pub das: u16,
    /// $43x7 DASB: indirect HDMA data bank.
    pub dasb: u8,
    /// $43x8/9 A2A: HDMA internal table pointer (auto-updated during HDMA).
    pub a2a: u16,
    /// $43xA NTRL: HDMA line counter (bits 6:0) + repeat flag (bit 7).
    pub ntrl: u8,
}

/// Per-channel HDMA run-time state (D8: no allocations per frame — these are
/// fixed-size fields in `Hdma`, initialized/reset at the start of each frame).
///
/// The line counter itself lives in the channel register file (`ntrl`,
/// readable at $43xA) and decrements raw each scanline, exactly like the
/// hardware register: a count byte of $01-$80 is 1-128 non-repeat lines,
/// $81-$FF is 1-127 repeat lines, and the next entry loads when the
/// decremented value's bits[6:0] reach 0. The rolling data pointers are the
/// registers too: A2A in direct mode, DAS in indirect mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct HdmaState {
    /// True while this channel is actively running this frame (false after
    /// the channel's table has reached its terminator entry).
    pub active: bool,
    /// Transfer on the upcoming scanline? True on the first line of every
    /// table entry; thereafter equal to the entry's repeat flag (NTRL bit 7).
    pub do_transfer: bool,
}

/// The 8-channel DMA register file.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dma {
    pub ch: [DmaChannel; 8],
}

/// Per-channel HDMA run-time state (fixed-size, zero per-frame allocation, D8).
#[derive(Debug, Clone, Copy, Default)]
pub struct Hdma {
    pub state: [HdmaState; 8],
}

impl Dma {
    pub fn new() -> Dma {
        Dma::default()
    }

    /// Read a $43xr register (channel 0..=7, reg 0x0..=0xA). Returns `None`
    /// for unmapped sub-addresses (open bus).
    pub fn read(&self, channel: usize, reg: u8) -> Option<u8> {
        if channel >= 8 {
            return None;
        }
        let ch = &self.ch[channel];
        match reg {
            0x0 => Some(ch.dmap),
            0x1 => Some(ch.bbad),
            0x2 => Some(ch.a1t as u8),
            0x3 => Some((ch.a1t >> 8) as u8),
            0x4 => Some(ch.a1b),
            0x5 => Some(ch.das as u8),
            0x6 => Some((ch.das >> 8) as u8),
            0x7 => Some(ch.dasb),
            0x8 => Some(ch.a2a as u8),
            0x9 => Some((ch.a2a >> 8) as u8),
            0xA => Some(ch.ntrl),
            _ => None,
        }
    }

    /// Write a $43xr register.
    pub fn write(&mut self, channel: usize, reg: u8, value: u8) {
        if channel >= 8 {
            return;
        }
        let ch = &mut self.ch[channel];
        match reg {
            0x0 => ch.dmap = value,
            0x1 => ch.bbad = value,
            0x2 => {
                let hi = ch.a1t & 0xFF00;
                ch.a1t = hi | value as u16;
            }
            0x3 => {
                let lo = ch.a1t & 0x00FF;
                ch.a1t = lo | ((value as u16) << 8);
            }
            0x4 => ch.a1b = value,
            0x5 => {
                let hi = ch.das & 0xFF00;
                ch.das = hi | value as u16;
            }
            0x6 => {
                let lo = ch.das & 0x00FF;
                ch.das = lo | ((value as u16) << 8);
            }
            0x7 => ch.dasb = value,
            0x8 => {
                let hi = ch.a2a & 0xFF00;
                ch.a2a = hi | value as u16;
            }
            0x9 => {
                let lo = ch.a2a & 0x00FF;
                ch.a2a = lo | ((value as u16) << 8);
            }
            0xA => ch.ntrl = value,
            _ => {} // unmapped
        }
    }
}

impl Hdma {
    pub fn new() -> Hdma {
        Hdma::default()
    }
}

/// Returns the B-bus register offset sequence for a DMA transfer unit pattern.
///
/// DMAP bits 0-2 select the pattern:
/// 0: [0]           1 byte, B-addr fixed
/// 1: [0,1]         2 bytes, alternating
/// 2: [0,0]         2 bytes, same reg twice
/// 3: [0,0,1,1]     4 bytes, two-each
/// 4: [0,1,2,3]     4 bytes, sequential
/// 5: [0,1,0,1]     4 bytes (behaviorally identical to 1 under the cycling
///                  executor; kept distinct to mirror the register encoding)
/// 6: [0,0]         same as 2
/// 7: [0,0,1,1]     same as 3
pub fn unit_pattern(dmap: u8) -> &'static [u8] {
    match dmap & 0x07 {
        0 => &[0],
        1 => &[0, 1],
        2 => &[0, 0],
        3 => &[0, 0, 1, 1],
        4 => &[0, 1, 2, 3],
        5 => &[0, 1, 0, 1],
        6 => &[0, 0],
        7 => &[0, 0, 1, 1],
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_read_write_roundtrip() {
        let mut dma = Dma::new();
        // Channel 3
        dma.write(3, 0x0, 0x85); // DMAP
        dma.write(3, 0x1, 0x18); // BBAD
        dma.write(3, 0x2, 0x34); // A1T lo
        dma.write(3, 0x3, 0x12); // A1T hi
        dma.write(3, 0x4, 0x7E); // A1B
        dma.write(3, 0x5, 0x00); // DAS lo (0 → 65536)
        dma.write(3, 0x6, 0x00); // DAS hi
        dma.write(3, 0x7, 0xAB); // DASB
        dma.write(3, 0x8, 0xCD); // A2A lo
        dma.write(3, 0x9, 0xEF); // A2A hi
        dma.write(3, 0xA, 0x01); // NTRL

        assert_eq!(dma.read(3, 0x0), Some(0x85));
        assert_eq!(dma.read(3, 0x1), Some(0x18));
        assert_eq!(dma.read(3, 0x2), Some(0x34));
        assert_eq!(dma.read(3, 0x3), Some(0x12));
        assert_eq!(dma.read(3, 0x4), Some(0x7E));
        assert_eq!(dma.read(3, 0x5), Some(0x00));
        assert_eq!(dma.read(3, 0x6), Some(0x00));
        assert_eq!(dma.read(3, 0x7), Some(0xAB));
        assert_eq!(dma.read(3, 0x8), Some(0xCD));
        assert_eq!(dma.read(3, 0x9), Some(0xEF));
        assert_eq!(dma.read(3, 0xA), Some(0x01));
    }

    #[test]
    fn unmapped_reg_returns_none() {
        let dma = Dma::new();
        assert_eq!(dma.read(0, 0xB), None);
        assert_eq!(dma.read(0, 0xFF), None);
    }

    #[test]
    fn das_zero_means_65536() {
        let mut dma = Dma::new();
        dma.write(0, 0x5, 0x00); // DAS lo
        dma.write(0, 0x6, 0x00); // DAS hi
                                 // das == 0 → documented as 65536 bytes; we store 0 and the executor
                                 // interprets das==0 as 65536.
        assert_eq!(dma.ch[0].das, 0);
    }

    #[test]
    fn unit_patterns_cover_all_modes() {
        assert_eq!(unit_pattern(0), &[0u8][..]);
        assert_eq!(unit_pattern(1), &[0u8, 1][..]);
        assert_eq!(unit_pattern(2), &[0u8, 0][..]);
        assert_eq!(unit_pattern(3), &[0u8, 0, 1, 1][..]);
        assert_eq!(unit_pattern(4), &[0u8, 1, 2, 3][..]);
        assert_eq!(unit_pattern(5), &[0u8, 1, 0, 1][..]);
        assert_eq!(unit_pattern(6), &[0u8, 0][..]);
        assert_eq!(unit_pattern(7), &[0u8, 0, 1, 1][..]);
    }

    /// HDMA state initializes to inactive.
    #[test]
    fn hdma_state_default_inactive() {
        let h = Hdma::new();
        for ch in 0..8 {
            assert!(
                !h.state[ch].active,
                "HDMA ch {ch} should be inactive by default"
            );
        }
    }

    /// HDMA channel: DMAP bit 6 encodes indirect mode.
    #[test]
    fn hdma_dmap_indirect_bit() {
        let mut dma = Dma::new();
        dma.write(0, 0x0, 0x40); // bit 6 = indirect
        assert_eq!(dma.ch[0].dmap & 0x40, 0x40);
    }
}
