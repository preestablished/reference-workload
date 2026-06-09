//! Cartridge: ROM image + optional battery save RAM, demo-game mapper
//! ("LoROM"-style bank-switching board — the only mapper M1/M2 need).
//!
//! OWNER (implementation): integration agent.
//!
//! Mapping (M1):
//! - Banks $00-$7D and $80-$FF, offsets $8000-$FFFF: ROM, 32 KiB pages —
//!   `rom[((bank & 0x7F) * 0x8000 + (offset - 0x8000)) & rom_mask]`.
//! - Banks $70-$7D (and mirrors $F0-$FD), offsets $0000-$7FFF: save RAM
//!   when present (wrapped to its size), else open bus on read / fault on
//!   write.
//! - Header at ROM offset $7FC0: 21-byte title, $7FD5 map mode, $7FD6 cart
//!   type, $7FD7 ROM size, $7FFC/D emulation reset vector.
//!
//! Construction validates: ROM length is a non-zero multiple of 32 KiB
//! (power-of-two mirrorable), and the emulation-mode reset vector points
//! into mapped ROM ($8000-$FFFF). The header title is not interpreted.

use crate::core_impl::CoreError;

/// A parsed cartridge.
pub struct Cartridge {
    /// Raw ROM bytes (length validated; mirror mask precomputed).
    pub rom: Vec<u8>,
    /// Mirror mask: `rom.len().next_power_of_two() - 1` style mask used for
    /// out-of-range bank reads.
    pub rom_mask: usize,
    /// Battery save RAM (published `sram` region when enabled).
    pub sram: Option<&'static mut [u8]>,
}

impl Cartridge {
    /// Parse and validate a ROM image. `sram`, when provided, is the
    /// externally-owned published buffer (D7); its length must be a
    /// non-zero power of two ≤ 512 KiB.
    pub fn from_rom(rom: Vec<u8>, sram: Option<&'static mut [u8]>) -> Result<Cartridge, CoreError> {
        let len = rom.len();

        // ROM must be non-zero, a multiple of 32 KiB, and a power of two
        // (for clean mirroring).
        if len == 0 || (len & 0x7FFF) != 0 || !len.is_power_of_two() {
            return Err(CoreError::BadRomSize { len });
        }

        // Validate emulation-mode reset vector in $00:FFFC/$00:FFFD.
        // In LoROM, bank $00 offset $8000-$FFFF maps to rom[0x0000..0x8000].
        // The reset vector is at rom offset $7FFC/$7FFD (bank $00 ROM offset).
        if len < 0x8000 {
            return Err(CoreError::BadRomSize { len });
        }
        let vec_lo = rom[0x7FFC] as u16;
        let vec_hi = rom[0x7FFD] as u16;
        let reset_vector = vec_lo | (vec_hi << 8);
        if reset_vector < 0x8000 {
            return Err(CoreError::BadResetVector {
                vector: reset_vector,
            });
        }

        // Validate SRAM size: must be power-of-two, 2 KiB ≤ size ≤ 512 KiB.
        if let Some(ref s) = sram {
            let slen = s.len();
            if slen == 0 || !slen.is_power_of_two() || !(2048..=512 * 1024).contains(&slen) {
                return Err(CoreError::BadSramSize { len: slen });
            }
        }

        let rom_mask = len - 1; // valid because len is power-of-two
        Ok(Cartridge {
            rom,
            rom_mask,
            sram,
        })
    }

    /// ROM/SRAM read for a 24-bit address; `None` when the address does not
    /// decode to the cartridge (open bus).
    pub fn read(&self, addr: u32) -> Option<u8> {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        match bank {
            // Banks $00-$7D and $80-$FF, offset $8000-$FFFF: ROM pages.
            0x00..=0x7D | 0x80..=0xFF if off >= 0x8000 => {
                let page = (bank & 0x7F) as usize;
                let rom_off = (page * 0x8000 + (off as usize - 0x8000)) & self.rom_mask;
                Some(self.rom[rom_off])
            }
            // Banks $70-$7D and mirrors $F0-$FD, offset $0000-$7FFF: SRAM.
            0x70..=0x7D | 0xF0..=0xFD if off < 0x8000 => {
                if let Some(ref sram) = self.sram {
                    let sram_off = (addr as usize) & (sram.len() - 1);
                    Some(sram[sram_off])
                } else {
                    None // open bus
                }
            }
            _ => None,
        }
    }

    /// SRAM write; returns false when the address does not decode to a
    /// writable cartridge region (caller faults per D9).
    pub fn write(&mut self, addr: u32, value: u8) -> bool {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        match bank {
            0x70..=0x7D | 0xF0..=0xFD if off < 0x8000 => {
                if let Some(ref mut sram) = self.sram {
                    let sram_off = (addr as usize) & (sram.len() - 1);
                    sram[sram_off] = value;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid 32 KiB ROM with a reset vector pointing at $8000.
    fn make_rom(size: usize) -> Vec<u8> {
        let mut rom = vec![0u8; size];
        // Place a valid reset vector (little-endian $8000) at $7FFC/$7FFD.
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80;
        rom
    }

    #[test]
    fn valid_32k_rom() {
        let rom = make_rom(0x8000);
        assert!(Cartridge::from_rom(rom, None).is_ok());
    }

    #[test]
    fn valid_256k_rom() {
        let rom = make_rom(256 * 1024);
        assert!(Cartridge::from_rom(rom, None).is_ok());
    }

    #[test]
    fn bad_rom_size_zero() {
        let result = Cartridge::from_rom(vec![], None);
        assert!(
            matches!(result, Err(CoreError::BadRomSize { len: 0 })),
            "expected BadRomSize(0), got: {:?}",
            result.err()
        );
    }

    #[test]
    fn bad_rom_size_not_power_of_two() {
        // 48 KiB = 3 * 16 KiB — not a power of two.
        let rom = make_rom(48 * 1024);
        let result = Cartridge::from_rom(rom, None);
        assert!(
            matches!(result, Err(CoreError::BadRomSize { .. })),
            "expected BadRomSize, got: {:?}",
            result.err()
        );
    }

    #[test]
    fn bad_reset_vector() {
        let mut rom = make_rom(0x8000);
        // Place reset vector at $1234 (below $8000).
        rom[0x7FFC] = 0x34;
        rom[0x7FFD] = 0x12;
        let result = Cartridge::from_rom(rom, None);
        assert!(
            matches!(result, Err(CoreError::BadResetVector { vector: 0x1234 })),
            "expected BadResetVector(0x1234), got: {:?}",
            result.err()
        );
    }

    #[test]
    fn rom_mirror_32k() {
        let mut rom = make_rom(0x8000);
        rom[0x0000] = 0xAB; // First byte of ROM page 0 → bank $00 offset $8000.
        let cart = Cartridge::from_rom(rom, None).unwrap();
        // Bank $00, offset $8000 → rom[0].
        assert_eq!(cart.read(0x00_8000), Some(0xAB));
        // Bank $01 mirrors bank $00 for 32 KiB ROM (mask = 0x7FFF).
        assert_eq!(cart.read(0x01_8000), Some(0xAB));
    }

    #[test]
    fn sram_read_write() {
        let rom = make_rom(0x8000);
        // 8 KiB SRAM.
        let sram_buf: &'static mut [u8] = Box::leak(Box::new([0u8; 8192]));
        let mut cart = Cartridge::from_rom(rom, Some(sram_buf)).unwrap();
        // Bank $70, offset $0000 → SRAM[0].
        assert!(cart.write(0x70_0000, 0x77));
        assert_eq!(cart.read(0x70_0000), Some(0x77));
    }

    #[test]
    fn sram_absent_read_is_none() {
        let rom = make_rom(0x8000);
        let cart = Cartridge::from_rom(rom, None).unwrap();
        assert_eq!(cart.read(0x70_0000), None);
    }

    #[test]
    fn sram_absent_write_returns_false() {
        let rom = make_rom(0x8000);
        let mut cart = Cartridge::from_rom(rom, None).unwrap();
        assert!(!cart.write(0x70_0000, 0xFF));
    }

    #[test]
    fn bad_sram_size_1k() {
        let rom = make_rom(0x8000);
        let sram_buf: &'static mut [u8] = Box::leak(Box::new([0u8; 1024]));
        assert!(matches!(
            Cartridge::from_rom(rom, Some(sram_buf)),
            Err(CoreError::BadSramSize { len: 1024 })
        ));
    }

    #[test]
    fn reset_vector_exactly_at_8000() {
        let mut rom = make_rom(0x8000);
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80; // $8000
        assert!(Cartridge::from_rom(rom, None).is_ok());
    }

    #[test]
    fn sram_mirror_bank_f0() {
        let rom = make_rom(0x8000);
        let sram_buf: &'static mut [u8] = Box::leak(Box::new([0u8; 8192]));
        let mut cart = Cartridge::from_rom(rom, Some(sram_buf)).unwrap();
        // Banks $F0-$FD also map to SRAM.
        assert!(cart.write(0xF0_0000, 0x55));
        assert_eq!(cart.read(0xF0_0000), Some(0x55));
    }
}
