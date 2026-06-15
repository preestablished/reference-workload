//! 64 KiB Audio RAM for the SPC700 audio processor.
//!
//! The SPC700 has a flat 64 KiB address space backed by on-chip RAM. The
//! power-on fill is not architecturally defined (the hardware resets to an
//! analog-uncertain state), so we choose a fixed documented constant analogous
//! to [`crate::WRAM_INIT_BYTE`] — this guarantees deterministic boot-to-boot
//! behaviour (D3).
//!
//! The IPL ROM overlay lives outside this module: the [`super::Apu`] memory
//! map layer decides whether to return IPL ROM bytes or underlying RAM bytes
//! for addresses $FFC0–$FFFF based on the $F1 control register enable bit.
//! Writes always land in underlying RAM regardless of the overlay state.

/// Fixed ARAM power-on fill pattern (D3): all bytes are `0x00`.
///
/// Rationale: the SPC700's on-chip RAM has no documented power-on value; real
/// hardware shows garbage. We fill with `0x00` rather than a patterned value
/// because the IPL ROM upload protocol stores data starting at the host-
/// supplied address and jumps to it; any clear pattern is equally correct.
pub const ARAM_INIT_BYTE: u8 = 0x00;

/// 64 KiB of audio RAM. Owned and boxed to keep the `Apu` struct itself
/// small on the stack. Allocated once in `Apu::new` (D8).
pub struct Aram {
    pub(super) data: Box<[u8; 0x10000]>,
}

impl Aram {
    /// Allocate and fill with [`ARAM_INIT_BYTE`].
    pub fn new() -> Self {
        Aram {
            data: Box::new([ARAM_INIT_BYTE; 0x10000]),
        }
    }

    /// Read a byte from the raw RAM (no I/O or IPL overlay).
    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    /// Write a byte to RAM. Writes always land here, even when the IPL ROM
    /// overlay is active for reads at that address.
    #[inline]
    pub fn write(&mut self, addr: u16, value: u8) {
        self.data[addr as usize] = value;
    }

    /// Borrow the raw 64 KiB slice mutably. Used by the APU's SPC700 step
    /// trampoline to pass a flat memory view to the core.
    #[inline]
    pub(super) fn as_raw_mut(&mut self) -> &mut [u8; 0x10000] {
        &mut self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_on_fill() {
        let aram = Aram::new();
        assert_eq!(aram.read(0x0000), ARAM_INIT_BYTE);
        assert_eq!(aram.read(0xFFFF), ARAM_INIT_BYTE);
        assert_eq!(aram.read(0x8000), ARAM_INIT_BYTE);
    }

    #[test]
    fn read_write_roundtrip() {
        let mut aram = Aram::new();
        aram.write(0x1234, 0xAB);
        assert_eq!(aram.read(0x1234), 0xAB);
        assert_eq!(aram.read(0x1235), ARAM_INIT_BYTE);
    }

    #[test]
    fn write_always_lands_in_ram() {
        // Even IPL-ROM-mapped range ($FFC0–$FFFF): writes go to underlying RAM.
        let mut aram = Aram::new();
        aram.write(0xFFC0, 0x42);
        assert_eq!(aram.read(0xFFC0), 0x42);
    }
}
