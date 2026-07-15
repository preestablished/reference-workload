//! Background layer rendering helpers for modes 0 and 1.

use super::regs::BgSc;
use super::regs::BgScroll;

/// A single pixel's BG color contribution.
#[derive(Clone, Copy)]
pub struct BgPixel {
    /// Raw color index within the tile's palette (0 = transparent).
    pub color_idx: u8,
    /// Absolute CGRAM index for this pixel.
    pub cgram_idx: u8,
    /// Tilemap palette-group bits, used by direct-color modes.
    pub palette_group: u8,
    /// Tile priority bit.
    pub priority: bool,
}

impl BgPixel {
    pub const TRANSPARENT: BgPixel = BgPixel {
        color_idx: 0,
        cgram_idx: 0,
        palette_group: 0,
        priority: false,
    };

    #[inline]
    pub fn is_transparent(self) -> bool {
        self.color_idx == 0
    }
}

/// Decode one 16-bit tilemap entry.
///
/// Format: `vhopppcc cccccccc`
/// [15]=vflip [14]=hflip [13]=priority [12:10]=palette [9:0]=char
#[inline]
fn decode_tile_entry(entry: u16) -> (u16, u8, bool, bool, bool) {
    let char_no = entry & 0x3FF;
    let palette = ((entry >> 10) & 0x07) as u8;
    let priority = (entry >> 13) & 1 != 0;
    let hflip = (entry >> 14) & 1 != 0;
    let vflip = (entry >> 15) & 1 != 0;
    (char_no, palette, priority, hflip, vflip)
}

/// Fetch tilemap entry from VRAM.
///
/// Screens are laid out in the documented quadrant order:
///   h=0,v=0: A only (32×32)
///   h=1,v=0: A left, B right (A at base, B at base+0x800)
///   h=0,v=1: A top, B bottom (A at base, B at base+0x800)
///   h=1,v=1: A top-left, B top-right, C bot-left, D bot-right
///             (+0x000, +0x800, +0x1000, +0x1800)
#[inline]
pub(super) fn fetch_tilemap(
    vram: &[u8; 0x10000],
    tm_base_bytes: usize,
    tx: u16,
    ty: u16,
    h_wide: bool,
    v_wide: bool,
) -> u16 {
    let h_screen = if h_wide { (tx >> 5) & 1 } else { 0 };
    let v_screen = if v_wide { (ty >> 5) & 1 } else { 0 };

    let screen_offset: usize = match (h_screen, v_screen) {
        (0, 0) => 0x0000,
        (1, 0) => 0x0800,
        (0, 1) => {
            if h_wide {
                0x1000
            } else {
                0x0800
            }
        }
        _ => 0x1800,
    };

    let local_tx = (tx & 31) as usize;
    let local_ty = (ty & 31) as usize;
    let entry_off = (local_ty * 32 + local_tx) * 2;
    let addr = (tm_base_bytes + screen_offset + entry_off) & 0xFFFF;
    let lo = vram[addr] as u16;
    let hi = vram[(addr + 1) & 0xFFFF] as u16;
    lo | (hi << 8)
}

/// Read one pixel from a 2bpp tile.
///
/// `tile_base`: byte address of the first byte of this 8×8 tile's data.
/// `px`, `py`: pixel coordinate within the 8×8 tile (already flip-adjusted).
#[inline]
fn read_2bpp(vram: &[u8; 0x10000], tile_base: usize, px: u16, py: u16) -> u8 {
    // 2bpp layout: each row = 2 bytes (plane0 byte, plane1 byte)
    let row = (tile_base + py as usize * 2) & 0xFFFF;
    let bit = 7 - px;
    let p0 = (vram[row] >> bit) & 1;
    let p1 = (vram[(row + 1) & 0xFFFF] >> bit) & 1;
    p0 | (p1 << 1)
}

/// Read one pixel from a 4bpp tile.
///
/// Layout: plane0+1 rows 0-7 (16 bytes) then plane2+3 rows 0-7 (16 bytes).
#[inline]
fn read_4bpp(vram: &[u8; 0x10000], tile_base: usize, px: u16, py: u16) -> u8 {
    let row = (tile_base + py as usize * 2) & 0xFFFF;
    let bit = 7 - px;
    let p0 = (vram[row] >> bit) & 1;
    let p1 = (vram[(row + 1) & 0xFFFF] >> bit) & 1;
    let p2 = (vram[(row + 16) & 0xFFFF] >> bit) & 1;
    let p3 = (vram[(row + 17) & 0xFFFF] >> bit) & 1;
    p0 | (p1 << 1) | (p2 << 2) | (p3 << 3)
}

#[inline]
fn read_8bpp(vram: &[u8; 0x10000], tile_base: usize, px: u16, py: u16) -> u8 {
    let row = (tile_base + py as usize * 2) & 0xffff;
    let bit = 7 - px;
    let mut color = 0u8;
    for pair in 0..4usize {
        let base = (row + pair * 16) & 0xffff;
        color |= ((vram[base] >> bit) & 1) << (pair * 2);
        color |= ((vram[(base + 1) & 0xffff] >> bit) & 1) << (pair * 2 + 1);
    }
    color
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_bg_pixel(
    vram: &[u8; 0x10000],
    sc: BgSc,
    tile_data_base_bytes: usize,
    bpp: u8,
    cgram_palette_base: u8,
    tile_size_px: u16,
    screen_x: u32,
    screen_y: u32,
) -> BgPixel {
    render_bg_pixel_dims(
        vram,
        sc,
        tile_data_base_bytes,
        bpp,
        cgram_palette_base,
        tile_size_px,
        tile_size_px,
        screen_x,
        screen_y,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_bg_pixel_dims(
    vram: &[u8; 0x10000],
    sc: BgSc,
    tile_data_base_bytes: usize,
    bpp: u8,
    cgram_palette_base: u8,
    tile_width_px: u16,
    tile_height_px: u16,
    screen_x: u32,
    screen_y: u32,
) -> BgPixel {
    let tile_col = (screen_x / tile_width_px as u32) as u16;
    let tile_row = (screen_y / tile_height_px as u32) as u16;
    let raw_tx = (screen_x % tile_width_px as u32) as u16;
    let raw_ty = (screen_y % tile_height_px as u32) as u16;
    let tm_entry = fetch_tilemap(
        vram,
        sc.base as usize,
        tile_col,
        tile_row,
        sc.h_wide,
        sc.v_wide,
    );
    let (char_no, palette, priority, hflip, vflip) = decode_tile_entry(tm_entry);

    let (sub_tx, sub_ty, pixel_x, pixel_y) = {
        let stx_unflipped = raw_tx / 8;
        let sty_unflipped = raw_ty / 8;
        let hparts = tile_width_px / 8;
        let vparts = tile_height_px / 8;
        let stx = if hflip {
            hparts - 1 - stx_unflipped
        } else {
            stx_unflipped
        };
        let sty = if vflip {
            vparts - 1 - sty_unflipped
        } else {
            sty_unflipped
        };
        let px = if hflip { 7 - (raw_tx & 7) } else { raw_tx & 7 };
        let py = if vflip { 7 - (raw_ty & 7) } else { raw_ty & 7 };
        (stx, sty, px, py)
    };
    let final_char = ((char_no as u32 + sub_tx as u32 + sub_ty as u32 * 16) & 0x3ff) as u16;
    let bytes_per_tile = bpp as usize * 8;
    let tile_base = (tile_data_base_bytes + final_char as usize * bytes_per_tile) & 0xffff;
    let color_idx = match bpp {
        2 => read_2bpp(vram, tile_base, pixel_x, pixel_y),
        4 => read_4bpp(vram, tile_base, pixel_x, pixel_y),
        8 => read_8bpp(vram, tile_base, pixel_x, pixel_y),
        _ => 0,
    };
    let palette_colors = 1u16 << bpp;
    let cgram_idx = cgram_palette_base as u16 + palette as u16 * palette_colors + color_idx as u16;
    BgPixel {
        color_idx,
        cgram_idx: cgram_idx as u8,
        palette_group: palette,
        priority,
    }
}

/// Render one BG layer for 256 pixels into `out`.
///
/// - `tile_data_base_bytes`: byte address of tile data region in VRAM
/// - `bpp`: 2 or 4
/// - `cgram_palette_base`: CGRAM offset for palette 0 of this layer
/// - `tile_size_px`: 8 or 16
/// - `line`: 0-based screen y (i.e. scanline - 1)
#[allow(clippy::too_many_arguments)]
pub fn render_bg_line(
    vram: &[u8; 0x10000],
    out: &mut [BgPixel; 256],
    sc: BgSc,
    scroll: BgScroll,
    tile_data_base_bytes: usize,
    bpp: u8,
    cgram_palette_base: u8,
    tile_size_px: u16,
    line: u16,
) {
    let screen_y = ((line as u32).wrapping_add(scroll.vofs as u32)) & 0x1FF;

    for out_x in 0..256u16 {
        let screen_x = ((out_x as u32).wrapping_add(scroll.hofs as u32)) & 0x1FF;
        out[out_x as usize] = render_bg_pixel(
            vram,
            sc,
            tile_data_base_bytes,
            bpp,
            cgram_palette_base,
            tile_size_px,
            screen_x,
            screen_y,
        );
    }
}
