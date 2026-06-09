//! Sprite (OAM) rendering for M1.
//!
//! Sprite sizes from OBSEL:
//!
//! Size pair index (bits [7:5] of $2101) selects two sizes (small, large):
//!   0: 8×8  / 16×16
//!   1: 8×8  / 32×32
//!   2: 8×8  / 64×64
//!   3: 16×16 / 32×32
//!   4: 16×16 / 64×64
//!   5: 32×32 / 64×64
//!   6: 16×32 / 32×64
//!   7: 16×32 / 32×32
//!
//! OAM low table (512 bytes, entries 0-127, 4 bytes each):
//!   byte 0: X[7:0]
//!   byte 1: Y[7:0]
//!   byte 2: tile[7:0]
//!   byte 3: vhoopppt  v=vflip h=hflip oo=priority pp=palette t=tile[8]
//!
//! OAM high table (32 bytes, 4 sprites per byte):
//!   bits [1:0] of sprite N = { size_bit, x_bit8 }
//!   sprite N → byte N/4, bits [(N%4)*2 + 1 : (N%4)*2]
//!   bit0 of pair = X sign (bit 8)
//!   bit1 of pair = size toggle (0=small, 1=large)

use super::regs::Obsel;

/// Sprite sizes: (width, height) in pixels.
const SPRITE_SIZES: [(u16, u16); 8] = [
    (8, 8),   // index 0 small
    (8, 8),   // index 1 small
    (8, 8),   // index 2 small
    (16, 16), // index 3 small
    (16, 16), // index 4 small
    (32, 32), // index 5 small
    (16, 32), // index 6 small
    (16, 32), // index 7 small
];

const SPRITE_SIZES_LARGE: [(u16, u16); 8] = [
    (16, 16), // index 0 large
    (32, 32), // index 1 large
    (64, 64), // index 2 large
    (32, 32), // index 3 large
    (64, 64), // index 4 large
    (64, 64), // index 5 large
    (32, 64), // index 6 large
    (32, 32), // index 7 large
];

/// Decoded sprite attributes.
#[derive(Clone, Copy, Default)]
pub struct Sprite {
    pub x: i16,       // 9-bit signed X position
    pub y: u8,        // Y position
    pub tile: u16,    // tile number (0-511)
    pub palette: u8,  // palette index (0-7, → CGRAM 128 + palette*16)
    pub priority: u8, // 0-3
    pub hflip: bool,
    pub vflip: bool,
    pub width: u16,
    pub height: u16,
}

/// Parse all 128 sprite entries from OAM.
pub fn parse_oam(oam: &[u8; 544], obsel: Obsel) -> [Sprite; 128] {
    let mut sprites = [Sprite {
        x: 0,
        y: 0,
        tile: 0,
        palette: 0,
        priority: 0,
        hflip: false,
        vflip: false,
        width: 8,
        height: 8,
    }; 128];

    let size_pair = obsel.size & 7;

    for i in 0..128usize {
        let base = i * 4;
        let lo_x = oam[base] as i16;
        let lo_y = oam[base + 1];
        let tile_lo = oam[base + 2] as u16;
        let attr = oam[base + 3];

        let tile_hi = ((attr & 1) as u16) << 8;
        let palette = (attr >> 1) & 0x07;
        let priority = (attr >> 4) & 0x03;
        let hflip = (attr >> 6) & 1 != 0;
        let vflip = (attr >> 7) != 0;

        // High table: 32 bytes after OAM low table (offset 512)
        let hi_byte = oam[512 + i / 4];
        let hi_shift = (i % 4) * 2;
        let hi_bits = (hi_byte >> hi_shift) & 0x03;
        let x_sign = hi_bits & 1; // bit 0 = X bit 8
        let size_bit = (hi_bits >> 1) & 1; // bit 1 = large size toggle

        // 9-bit signed X: if x_sign=1 and lo_x < 256, x = lo_x - 256
        let x = if x_sign != 0 { lo_x - 256 } else { lo_x };

        let (width, height) = if size_bit == 0 {
            SPRITE_SIZES[size_pair as usize]
        } else {
            SPRITE_SIZES_LARGE[size_pair as usize]
        };

        sprites[i] = Sprite {
            x,
            y: lo_y,
            tile: tile_lo | tile_hi,
            palette,
            priority,
            hflip,
            vflip,
            width,
            height,
        };
    }

    sprites
}

/// A single sprite pixel contribution.
#[derive(Clone, Copy)]
pub struct SpritePixel {
    pub cgram_idx: u8,
    pub color_idx: u8,
    pub priority: u8,
}

impl SpritePixel {
    pub const TRANSPARENT: SpritePixel = SpritePixel {
        cgram_idx: 0,
        color_idx: 0,
        priority: 0,
    };

    #[inline]
    pub fn is_transparent(self) -> bool {
        self.color_idx == 0
    }
}

/// Render sprites for one scanline into `out`.
///
/// Priority: lower OAM index wins (drawn last = higher priority in our
/// painter approach, but we actually keep the first non-transparent hit
/// per pixel). Actually we collect and pick lowest index wins at each pixel.
///
/// M1 simplification: no range/time limits (all 128 sprites evaluated).
/// Priority rotation is not applied in M1 (comment per spec).
///
/// `name_base_bytes`: OBJ tile data base in VRAM bytes.
/// `name_gap_words`: additional word offset between first and second sprite page.
pub fn render_sprite_line(
    vram: &[u8; 0x10000],
    oam: &[u8; 544],
    obsel: Obsel,
    out: &mut [SpritePixel; 256],
    line: u16,
) {
    // Initialize all pixels transparent.
    for p in out.iter_mut() {
        *p = SpritePixel::TRANSPARENT;
    }

    // Track which pixel is already filled by a lower OAM index sprite.
    // OAM index 0 = highest sprite priority; we draw in reverse order
    // so lower index overwrites higher index.
    let sprites = parse_oam(oam, obsel);

    // Draw sprites in reverse index order so index 0 wins (overwrites).
    for i in (0..128usize).rev() {
        let s = sprites[i];
        let screen_y = line as i32 - 1; // 0-based

        // Check if this sprite's Y range covers the current scanline.
        // Y is the top of the sprite; comparison wraps at 256.
        let sprite_y = s.y as i32;
        let dy = (screen_y - sprite_y).rem_euclid(256);
        if dy >= s.height as i32 {
            continue;
        }
        let tile_row_abs = dy as u16;

        // Compute tile pixel Y within the 8×8 sub-tile.
        let tile_row_flipped = if s.vflip {
            s.height - 1 - tile_row_abs
        } else {
            tile_row_abs
        };
        let sub_tile_y = tile_row_flipped / 8;
        let pixel_y = tile_row_flipped & 7;

        for col in 0..s.width {
            let screen_x = s.x + col as i16;
            if !(0..256).contains(&screen_x) {
                continue;
            }

            let col_flipped = if s.hflip { s.width - 1 - col } else { col };
            let sub_tile_x = col_flipped / 8;
            let pixel_x = col_flipped & 7;

            // Sub-tile number within the sprite matrix.
            // Sprites with width > 8 have multiple sub-tile columns.
            let sub_tile_col = sub_tile_x;
            let sub_tile_row = sub_tile_y;

            // Tile number in the OBJ page.
            // Low 4 bits of tile# are the column within a row of 16 tiles.
            // Adding sub_tile_col wraps within the low 4 bits for width>8.
            // Sub-tile rows add 16 to the tile number.
            let base_tile = s.tile;
            let tile_no = base_tile
                .wrapping_add(sub_tile_row * 16)
                .wrapping_add(sub_tile_col)
                & 0x1FF;

            // OBJ tile data base: name_base_bytes for tiles 0-255,
            // + name_select gap for tiles 256-511.
            let name_base_bytes = (obsel.name_base as usize) << 14; // words→bytes: *2, then *8192 for 4KiW units
            let tile_addr_bytes = if tile_no < 0x100 {
                (name_base_bytes + tile_no as usize * 32) & 0xFFFF
            } else {
                let page2_base =
                    (name_base_bytes + (obsel.name_select as usize + 1) * 0x2000) & 0xFFFF;
                (page2_base + (tile_no as usize & 0xFF) * 32) & 0xFFFF
            };

            // 4bpp tile read
            let row_addr = (tile_addr_bytes + pixel_y as usize * 2) & 0xFFFF;
            let bit = 7 - pixel_x;
            let p0 = (vram[row_addr] >> bit) & 1;
            let p1 = (vram[(row_addr + 1) & 0xFFFF] >> bit) & 1;
            let p2 = (vram[(row_addr + 16) & 0xFFFF] >> bit) & 1;
            let p3 = (vram[(row_addr + 17) & 0xFFFF] >> bit) & 1;
            let color_idx = p0 | (p1 << 1) | (p2 << 2) | (p3 << 3);

            if color_idx == 0 {
                continue; // transparent
            }

            // Sprite palette starts at CGRAM 128.
            let cgram_idx = 128u8.wrapping_add(s.palette * 16).wrapping_add(color_idx);

            out[screen_x as usize] = SpritePixel {
                cgram_idx,
                color_idx,
                priority: s.priority,
            };
        }
    }
}
