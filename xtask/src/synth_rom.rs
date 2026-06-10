//! Builds the synthetic test ROM from the embedded assembly source.
//!
//! The ROM is 32 KiB (32768 bytes), LoROM-mapped at bank $00 offsets
//! $8000-$FFFF. The cartridge header is at file offset $7FC0; the
//! complement/checksum pair at $7FDC-$7FDF is computed over the final image
//! using the standard convention.

use crate::asm;

/// The assembly source, embedded at compile time.
const SYNTH_SOURCE: &str = include_str!("../asm/synth.s65");

/// ROM size in bytes (32 KiB).
pub const ROM_SIZE: usize = 32768;

/// Cartridge header field offsets within the ROM image (file offsets = cpu_addr - $8000).
/// LoROM: header at CPU address $FFC0 → file offset $7FC0.
pub const HEADER_TITLE_OFFSET: usize = 0x7FC0; // CPU $FFC0
pub const HEADER_MAP_MODE: usize = 0x7FD5; // CPU $FFD5
pub const HEADER_CART_TYPE: usize = 0x7FD6; // CPU $FFD6
pub const HEADER_ROM_SIZE: usize = 0x7FD7; // CPU $FFD7
pub const HEADER_SRAM_SIZE: usize = 0x7FD8; // CPU $FFD8
pub const HEADER_COUNTRY: usize = 0x7FD9; // CPU $FFD9
pub const HEADER_DEV_ID: usize = 0x7FDA; // CPU $FFDA
pub const HEADER_VERSION: usize = 0x7FDB; // CPU $FFDB
pub const HEADER_CHECKSUM_COMPLEMENT: usize = 0x7FDC; // CPU $FFDC — 2 bytes: complement
pub const HEADER_CHECKSUM: usize = 0x7FDE; // CPU $FFDE — 2 bytes: checksum
pub const NATIVE_VECTORS: usize = 0x7FE4; // CPU $FFE4
pub const EMU_VECTORS: usize = 0x7FF4; // CPU $FFF4

/// Build the synthetic test ROM. Deterministic: same bytes every call.
///
/// # Panics
/// Panics if the assembler produces an error (should not happen with the
/// embedded source).
pub fn build_synth_rom() -> Vec<u8> {
    match try_build_synth_rom() {
        Ok(rom) => rom,
        Err(e) => panic!("synth ROM assembly failed: {}", e),
    }
}

/// Build the synthetic test ROM, returning an error on assembler failure.
pub fn try_build_synth_rom() -> Result<Vec<u8>, asm::AsmError> {
    // Assemble the source.
    let (asm_bytes, base) = asm::assemble(SYNTH_SOURCE)?;

    // The base address from .org directives should be $8000 (= 32768 decimal).
    // Our ROM image is 32 KiB, so file offset 0 corresponds to address $8000.
    // asm_bytes[i] = byte at address (base + i).

    let mut rom = vec![0u8; ROM_SIZE];

    // Map assembled bytes into ROM: address $8000..=$FFFF → offset $0000..$7FFF.
    // base is $8000, so offset = addr - $8000.
    if base < 0x8000 && base != 0 {
        // Something weird; use raw overlay at offset 0.
        let copy_len = asm_bytes.len().min(ROM_SIZE);
        rom[..copy_len].copy_from_slice(&asm_bytes[..copy_len]);
    } else {
        let start_offset = (base as usize).wrapping_sub(0x8000);
        let copy_len = asm_bytes.len().min(ROM_SIZE.saturating_sub(start_offset));
        if start_offset < ROM_SIZE {
            rom[start_offset..start_offset + copy_len].copy_from_slice(&asm_bytes[..copy_len]);
        }
    }

    // Fix checksum/complement pair.
    // Convention: preset checksum fields as $FF $FF $00 $00 (complement=$FFFF, checksum=$0000),
    // then sum all bytes; the resulting sum IS the checksum.
    // complement = checksum ^ 0xFFFF.
    // We do it by: zero the fields, sum, that's checksum.
    rom[HEADER_CHECKSUM_COMPLEMENT] = 0xFF;
    rom[HEADER_CHECKSUM_COMPLEMENT + 1] = 0xFF;
    rom[HEADER_CHECKSUM] = 0x00;
    rom[HEADER_CHECKSUM + 1] = 0x00;

    let sum: u32 = rom.iter().map(|&b| b as u32).sum();
    let checksum = (sum & 0xFFFF) as u16;
    let complement = checksum ^ 0xFFFF;

    rom[HEADER_CHECKSUM_COMPLEMENT] = complement as u8;
    rom[HEADER_CHECKSUM_COMPLEMENT + 1] = (complement >> 8) as u8;
    rom[HEADER_CHECKSUM] = checksum as u8;
    rom[HEADER_CHECKSUM + 1] = (checksum >> 8) as u8;

    Ok(rom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synth_rom_length() {
        let rom = build_synth_rom();
        assert_eq!(rom.len(), ROM_SIZE, "ROM must be exactly 32 KiB");
    }

    #[test]
    fn synth_rom_header_bytes() {
        let rom = build_synth_rom();

        // Title: "REFWORK SYNTH ROM    " (21 bytes, space-padded)
        let title = &rom[HEADER_TITLE_OFFSET..HEADER_TITLE_OFFSET + 21];
        let title_str = std::str::from_utf8(title).expect("title is ASCII");
        assert!(
            title_str.starts_with("REFWORK SYNTH ROM"),
            "unexpected title: {:?}",
            title_str
        );

        assert_eq!(
            rom[HEADER_MAP_MODE], 0x20,
            "map mode should be $20 (LoROM slow)"
        );
        assert_eq!(
            rom[HEADER_CART_TYPE], 0x00,
            "cart type should be $00 (ROM only)"
        );
        assert_eq!(
            rom[HEADER_ROM_SIZE], 0x05,
            "ROM size field should be $05 (32 KiB)"
        );
        assert_eq!(rom[HEADER_SRAM_SIZE], 0x00, "SRAM size should be $00");
    }

    #[test]
    fn synth_rom_checksum_valid() {
        let rom = build_synth_rom();

        let complement = (rom[HEADER_CHECKSUM_COMPLEMENT] as u16)
            | ((rom[HEADER_CHECKSUM_COMPLEMENT + 1] as u16) << 8);
        let checksum = (rom[HEADER_CHECKSUM] as u16) | ((rom[HEADER_CHECKSUM + 1] as u16) << 8);

        // complement XOR checksum must be $FFFF
        assert_eq!(
            complement ^ checksum,
            0xFFFF,
            "checksum complement pair invalid"
        );

        // verify the checksum matches: reset fields and re-sum
        let mut verify = rom.clone();
        verify[HEADER_CHECKSUM_COMPLEMENT] = 0xFF;
        verify[HEADER_CHECKSUM_COMPLEMENT + 1] = 0xFF;
        verify[HEADER_CHECKSUM] = 0x00;
        verify[HEADER_CHECKSUM + 1] = 0x00;
        let sum: u32 = verify.iter().map(|&b| b as u32).sum();
        let expected_cksum = (sum & 0xFFFF) as u16;
        assert_eq!(
            checksum, expected_cksum,
            "checksum does not match sum of ROM bytes"
        );
    }

    #[test]
    fn synth_rom_reset_vector_in_range() {
        let rom = build_synth_rom();
        // Emulation-mode reset vector at $7FFC-$7FFD (file offsets)
        let vec_lo = rom[0x7FFC] as u16;
        let vec_hi = rom[0x7FFD] as u16;
        let reset_vec = vec_lo | (vec_hi << 8);
        assert!(
            reset_vec >= 0x8000,
            "reset vector ${:04X} not in ROM range $8000-$FFFF",
            reset_vec
        );
    }

    #[test]
    fn synth_rom_deterministic() {
        let rom1 = build_synth_rom();
        let rom2 = build_synth_rom();
        assert_eq!(rom1, rom2, "build_synth_rom() must be bit-deterministic");

        let h1 = blake3::hash(&rom1);
        let h2 = blake3::hash(&rom2);
        assert_eq!(h1, h2, "blake3 hashes must match across calls");
    }
}
