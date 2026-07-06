//! PPU register state: all latched values from $2100–$2133 writes.

/// Tilemap base-address and size bits for one background layer.
#[derive(Clone, Copy, Default)]
pub struct BgSc {
    /// Tilemap base address in VRAM (units of 0x400 words, i.e. reg[7:2] << 10).
    pub base: u16,
    /// H-extend: two screens side by side.
    pub h_wide: bool,
    /// V-extend: two screens stacked.
    pub v_wide: bool,
}

/// Tile-data base addresses for a pair of BG layers ($210B / $210C).
/// Each nibble is a 4 KiW block (addr = nibble << 13 in bytes).
#[derive(Clone, Copy, Default)]
pub struct BgNba {
    pub bg1_base: u16,
    pub bg2_base: u16,
    pub bg3_base: u16,
    pub bg4_base: u16,
}

/// One background scroll register pair (HOFS / VOFS).
#[derive(Clone, Copy, Default)]
pub struct BgScroll {
    pub hofs: u16,
    pub vofs: u16,
}

/// VMAIN ($2115) decoded state.
#[derive(Clone, Copy)]
pub struct Vmain {
    /// Increment step in words: 1, 32, or 128.
    pub step: u16,
    /// Address remapping mode 0-3.
    pub remap: u8,
    /// If true, increment VMADD after high byte access; else after low byte.
    pub inc_on_high: bool,
}

impl Default for Vmain {
    fn default() -> Self {
        Self {
            step: 1,
            remap: 0,
            inc_on_high: false,
        }
    }
}

/// OBSEL ($2101) decoded state.
#[derive(Clone, Copy, Default)]
pub struct Obsel {
    /// Sprite size pair index 0-7.
    pub size: u8,
    /// OBJ tile name base (bits [2:0] of $2101, in 8K-word units).
    pub name_base: u16,
    /// Name-select gap between first and second sprite page (bits [4:3]).
    pub name_select: u16,
}
