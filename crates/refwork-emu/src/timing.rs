//! Frame/scanline timing constants and bus-region access costs.
//!
//! M1 timing model (deterministic, documented; refined in M2 if accuracy
//! debugging needs it):
//! - NTSC-class frame: 262 scanlines (0..=261), 1364 master clocks each.
//!   Fixed frame length [`MCLK_PER_FRAME`]; the short-line/long-line
//!   interlace variations are deliberately not modeled in M1.
//! - Visible scanlines are 1..=224 (rendered to framebuffer rows 0..=223).
//! - V-blank begins at the start of scanline 225 (NMI flag set there);
//!   it ends at the start of scanline 0.
//! - Auto-joypad read occupies scanlines 225..=227.
//! - One CPU internal cycle = 6 master clocks. Memory access costs follow
//!   the documented region speed table ([`mem_speed`]).

/// Scanlines per frame (0..=261).
pub const LINES_PER_FRAME: u16 = 262;
/// Master clocks per scanline.
pub const MCLK_PER_LINE: u64 = 1364;
/// Master clocks per frame (fixed; see module docs).
pub const MCLK_PER_FRAME: u64 = LINES_PER_FRAME as u64 * MCLK_PER_LINE;
/// First visible scanline (inclusive).
pub const FIRST_VISIBLE_LINE: u16 = 1;
/// Last visible scanline (inclusive).
pub const LAST_VISIBLE_LINE: u16 = 224;
/// Scanline at whose start v-blank begins and the NMI flag is raised.
pub const VBLANK_START_LINE: u16 = 225;
/// Master clocks per CPU internal cycle.
pub const MCLK_PER_INTERNAL_CYCLE: u64 = 6;
/// H-blank starts at this dot within a scanline (1 dot = 4 master clocks).
pub const HBLANK_START_DOT: u16 = 274;

/// Published framebuffer geometry (ARCHITECTURE.md §1 D7): XRGB8888,
/// 256×224, row-major, stride 1024 bytes.
pub const FB_WIDTH: usize = 256;
/// Framebuffer height in pixels.
pub const FB_HEIGHT: usize = 224;
/// Framebuffer row stride in bytes.
pub const FB_STRIDE: usize = 1024;
/// Total published framebuffer size in bytes.
pub const FB_BYTES: usize = FB_STRIDE * FB_HEIGHT; // 229_376

/// Master clocks consumed by one bus access at `addr` (24-bit bus address).
///
/// Standard region speed table:
/// - banks $00-$3F: $0000-$1FFF → 8, $2000-$3FFF → 6, $4000-$41FF → 12,
///   $4200-$5FFF → 6, $6000-$FFFF → 8
/// - banks $40-$7F: 8
/// - banks $80-$BF: as $00-$3F, except $8000-$FFFF → 6 if `fast_rom` else 8
/// - banks $C0-$FF: 6 if `fast_rom` else 8
pub fn mem_speed(addr: u32, fast_rom: bool) -> u64 {
    let bank = (addr >> 16) as u8;
    let off = (addr & 0xFFFF) as u16;
    match bank {
        0x00..=0x3F => match off {
            0x0000..=0x1FFF => 8,
            0x2000..=0x3FFF => 6,
            0x4000..=0x41FF => 12,
            0x4200..=0x5FFF => 6,
            _ => 8,
        },
        0x40..=0x7F => 8,
        0x80..=0xBF => match off {
            0x0000..=0x1FFF => 8,
            0x2000..=0x3FFF => 6,
            0x4000..=0x41FF => 12,
            0x4200..=0x5FFF => 6,
            0x6000..=0x7FFF => 8,
            _ => {
                if fast_rom {
                    6
                } else {
                    8
                }
            }
        },
        _ => {
            if fast_rom {
                6
            } else {
                8
            }
        }
    }
}
