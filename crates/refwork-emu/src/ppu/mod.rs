//! Scanline PPU: backgrounds + sprites + mid-frame raster effects (M2).
//!
//! OWNER (implementation): PPU agent.
//!
//! M1 scope (retained): BG modes 0 and 1, sprites (OAM, OBSEL sizes,
//! priorities), whole-frame scrolling, CGRAM palettes, brightness, force blank.
//!
//! M2 additions:
//! - Color math: CGWSEL ($2130) / CGADSUB ($2131) / COLDATA ($2132) — fixed-color
//!   and subscreen operands, add/subtract, half-math, per-layer enable, backdrop.
//!   Integer-only: 5-bit-per-channel adds with clamp (deny gate enforces no floats).
//! - Windows: $2123-$212B (W12SEL/W34SEL/WOBJSEL, WH0-WH3, WBGLOG/WOBJLOG) plus
//!   TMW/TSW ($212E/$212F): two windows, per-layer enable/invert, four combination
//!   ops (OR/AND/XOR/XNOR), main/sub masking. Per-line 256-entry masks in fixed
//!   arrays (D8: no per-frame allocations).
//! - OPHCT real H dot counter: latch time derives from mclk_frame via `latch_hv`.
//! - Mosaic ($2106): size + per-BG enable, line-group quantization.
//! - BG mode 3 (8bpp palette): 8bpp tile color indices into CGRAM directly.
//! - BG mode 7 (affine): 8.8 fixed-point signed matrix math in i32, no floats.
//!   EXTBG (SETINI bit 6) and hires/interlace remain faulting (D9).
//!
//! Still faulting (D9): modes 2, 4, 5, 6; SETINI interlace/overscan/hires;
//! mode-7 EXTBG; indirect-color mode.
//!
//! Sprite range/time overflow limits are deliberately not modeled (documented
//! simplification; determinism is unaffected).

use crate::fault::Fault;
use crate::timing::FB_BYTES;

mod bg;
mod palette;
mod regs;
mod sprite;

use bg::{render_bg_line, BgPixel};
use palette::{bgr555_to_xrgb8888, read_cgram_color};
use regs::{BgNba, BgSc, BgScroll, Obsel, Vmain};
use sprite::{render_sprite_line, SpritePixel};

// ────────────────────────────────────────────────────────────────
// Window mask computation
// ────────────────────────────────────────────────────────────────

/// Compute a single window's 256-pixel active mask.
/// Returns a bitmask array where `mask[x] = 1` means the window covers pixel x.
/// Window is active for pixels in the range [left, right] (inclusive, wrapping u8).
#[inline]
fn window_range_mask(left: u8, right: u8, out: &mut [u8; 256]) {
    for x in 0u8..=255 {
        out[x as usize] = if left <= right {
            if x >= left && x <= right {
                1
            } else {
                0
            }
        } else {
            // Wrapped range: active outside the gap
            if x >= left || x <= right {
                1
            } else {
                0
            }
        };
    }
}

/// Combine two window masks with the documented 2-bit combination operator.
/// 0=OR, 1=AND, 2=XOR, 3=XNOR (same as XNOR = !(w1 XOR w2)).
#[inline]
fn window_combine(w1: u8, w2: u8, op: u8) -> u8 {
    match op & 0x03 {
        0 => w1 | w2,
        1 => w1 & w2,
        2 => w1 ^ w2,
        _ => (w1 ^ w2) ^ 1, // XNOR
    }
}

// ────────────────────────────────────────────────────────────────
// Color math helpers (integer only; deny gate enforces)
// ────────────────────────────────────────────────────────────────

/// Read a BGR555 color from CGRAM at index `idx` (0-255), returning the
/// 5-bit per channel tuple (r5, g5, b5).
#[inline]
fn cgram_color_components(cgram: &[u8; 512], idx: usize) -> (u8, u8, u8) {
    let color = read_cgram_color(cgram, idx);
    let r5 = (color & 0x1F) as u8;
    let g5 = ((color >> 5) & 0x1F) as u8;
    let b5 = ((color >> 10) & 0x1F) as u8;
    (r5, g5, b5)
}

/// Decompose a BGR555 fixed-color into (r5, g5, b5) components.
#[inline]
fn cgram_color_components_fixed(color: u16) -> (u8, u8, u8) {
    let r5 = (color & 0x1F) as u8;
    let g5 = ((color >> 5) & 0x1F) as u8;
    let b5 = ((color >> 10) & 0x1F) as u8;
    (r5, g5, b5)
}

/// Apply color math: add or subtract two BGR555 colors, optional half,
/// with per-channel 5-bit clamp. Returns a BGR555 result.
/// `add`: true=add, false=subtract. `half`: true=divide result by 2.
///
/// Documented operation order: the raw sum/difference is halved FIRST and
/// clamped after — a 5-bit add tops out at 62, halving to 31, so half-math
/// must not be capped at 15 by a premature clamp.
#[inline]
#[allow(clippy::too_many_arguments)] // two unpacked RGB triples + op flags
fn color_math_op(
    main_r: u8,
    main_g: u8,
    main_b: u8,
    sub_r: u8,
    sub_g: u8,
    sub_b: u8,
    add: bool,
    half: bool,
) -> u16 {
    #[inline]
    fn channel(main: u8, sub: u8, add: bool, half: bool) -> u8 {
        let mut v: i16 = if add {
            main as i16 + sub as i16
        } else {
            main as i16 - sub as i16
        };
        if half {
            // Arithmetic shift: negative differences stay negative until the
            // final floor clamp below.
            v >>= 1;
        }
        v.clamp(0, 31) as u8
    }
    let r5 = channel(main_r, sub_r, add, half);
    let g5 = channel(main_g, sub_g, add, half);
    let b5 = channel(main_b, sub_b, add, half);
    (r5 as u16) | ((g5 as u16) << 5) | ((b5 as u16) << 10)
}

// ────────────────────────────────────────────────────────────────
// PPU struct
// ────────────────────────────────────────────────────────────────

/// PPU state: VRAM (the published-region buffer when enabled), CGRAM, OAM,
/// registers, write latches, and the back buffer.
pub struct Ppu {
    /// 64 KiB video RAM. Externally owned so it can be published (D7).
    pub vram: &'static mut [u8; 0x10000],
    /// 512-byte palette RAM (256 × BGR555).
    pub cgram: [u8; 512],
    /// 544-byte object attribute memory (512 + 32 high table).
    pub oam: [u8; 544],
    /// Internal XRGB8888 back buffer, 256×224, stride 1024 (row-major).
    /// `render_scanline` writes here; the core blits at frame end.
    pub back: Box<[u8; FB_BYTES]>,

    // ── $2100 INIDISP ───────────────────────────────────────────
    /// Force-blank flag (bit 7 of INIDISP).  Power-on = true.
    force_blank: bool,
    /// Master brightness 0-15 (bits [3:0] of INIDISP).
    brightness: u8,

    // ── $2101 OBSEL ─────────────────────────────────────────────
    obsel: Obsel,

    // ── $2102/$2103 OAMADD ──────────────────────────────────────
    /// OAMADD base value (10-bit, latched at v-blank start).
    oam_base_addr: u16,
    /// Priority-rotation bit (bit 7 of $2103).
    oam_priority_rotation: bool,
    /// Internal OAM address (incremented on reads/writes).
    oam_addr: u16,
    /// Low-table write latch: holds the low byte while waiting for the high byte.
    oam_write_latch: u8,
    /// True when the next write to $2104 is the low byte (even address).
    oam_latch_valid: bool,

    // ── $2105 BGMODE ────────────────────────────────────────────
    /// Current BG mode (0-7; only 0 and 1 implemented).
    bg_mode: u8,
    /// BG3 priority bit (bit 3 of $2105, mode 1 only).
    bg3_priority: bool,
    /// Per-BG tile size: true = 16×16, false = 8×8.  Indices 0-3 = BG1-BG4.
    bg_tile_size: [bool; 4],

    // ── $2107–$210A BGnSC ───────────────────────────────────────
    bg_sc: [BgSc; 4],

    // ── $210B/$210C BG12NBA/BG34NBA ─────────────────────────────
    bg_nba: BgNba,

    // ── $210D–$2114 BGnHOFS/VOFS ────────────────────────────────
    /// BG scroll registers (BG1–BG4).
    bg_scroll: [BgScroll; 4],
    /// Shared write-twice offset latch (prev byte written to any H/VOFS reg).
    offset_latch: u8,

    // ── $2115 VMAIN ─────────────────────────────────────────────
    vmain: Vmain,

    // ── $2116/$2117 VMADD ───────────────────────────────────────
    vmadd: u16,

    // ── VRAM prefetch buffer ($2139/$213A) ──────────────────────
    /// Prefetch buffer: loaded on VMADD write and after read.
    vram_prefetch: u16,

    // ── $2121 CGADD / $2122 CGDATA ──────────────────────────────
    cgram_addr: u8,
    /// Write-twice latch for CGDATA: holds low byte on first write.
    cgdata_latch: u8,
    /// True when the next $2122 write is the high byte.
    cgdata_high: bool,

    // ── $2123–$212B window registers ────────────────────────────
    w12sel: u8,
    w34sel: u8,
    wobjsel: u8,
    wh0: u8,
    wh1: u8,
    wh2: u8,
    wh3: u8,
    wbglog: u8,
    wobjlog: u8,

    // ── $212C/$212D TM/TS ───────────────────────────────────────
    /// Main-screen layer enable bits (honored).
    tm: u8,
    /// Sub-screen layer enable bits (stored, ignored in M1 — no subscreen).
    ts: u8,

    // ── $212E/$212F TMW/TSW ─────────────────────────────────────
    tmw: u8,
    tsw: u8,

    // ── $2130/$2131 CGWSEL/CGADSUB ──────────────────────────────
    cgwsel: u8,
    cgadsub: u8,

    // ── $2132 COLDATA ───────────────────────────────────────────
    /// Fixed color as BGR555 (each component independently settable via $2132).
    /// Bit pattern: R5=bits[4:0], G5=bits[9:5], B5=bits[14:10].
    coldata_color: u16,

    // ── $2133 SETINI ────────────────────────────────────────────
    setini: u8,

    // ── Mode-7 registers ($211A–$2120) ──────────────────────────
    /// Mode-7 registers stored silently (mode 7 itself faults via BGMODE).
    m7sel: u8,
    m7a: i16, // signed 16-bit
    m7b: i16, // only low byte is the multiplier for $2134-$2136
    m7c: i16,
    m7d: i16,
    m7x: i16,
    m7y: i16,
    /// Write-twice latch for mode-7 matrix/center params.
    m7_latch: u8,

    // ── $213C/$213D OPHCT/OPVCT ─────────────────────────────────
    /// Latched H counter (dot position; M1 simplification: always 0).
    ophct: u16,
    /// Latched V counter (current line).
    opvct: u16,
    /// Read-toggle for OPHCT (bit 0 of reads toggles between low/high byte).
    ophct_read_high: bool,
    /// Read-toggle for OPVCT.
    opvct_read_high: bool,
    /// STAT78 counter latch flag (set by $2137 read, cleared by $213F read).
    counter_latched: bool,

    // ── $213B RDCGRAM ───────────────────────────────────────────
    /// Read-twice latch for RDCGRAM.
    cgram_read_high: bool,

    // ── $2138 RDOAM ─────────────────────────────────────────────
    // (uses oam_addr which is post-incremented on each read)

    // ── Current scanline (for $2137 SLHV latch) ─────────────────
    cur_line: u16,

    // ── M2: Mosaic ($2106) ──────────────────────────────────────
    /// Mosaic size (0-15; displayed size = size+1 pixels per block).
    mosaic_size: u8,
    /// Per-BG mosaic enable bits[3:0]: BG1-BG4.
    mosaic_bg_enable: u8,
    /// Mosaic "start of current block" line for each BG (updated each frame).
    /// Mosaic groups restart at the start of the frame (line 1) and repeat
    /// every (mosaic_size+1) lines.
    mosaic_start_line: u16,

    // ── M2: Subscreen line buffer (D8: fixed, allocated in new) ─
    /// Per-pixel subscreen CGRAM index for the current line (0 = backdrop/transparent).
    sub_pixels: Box<[u8; 256]>,

    // ── M2: Window masks (D8: fixed-size arrays) ─────────────────
    /// Per-pixel window-1 raw mask (1 = inside).
    win1_mask: Box<[u8; 256]>,
    /// Per-pixel window-2 raw mask.
    win2_mask: Box<[u8; 256]>,
    /// Combined window mask per main-screen layer (BG1..BG4, OBJ=index 4).
    main_win_mask: Box<[[u8; 256]; 5]>,
    /// Combined window mask per sub-screen layer.
    sub_win_mask: Box<[[u8; 256]; 5]>,
    /// Combined color-math window mask (for CGWSEL bits [7:6]).
    math_win_mask: Box<[u8; 256]>,
}

impl Ppu {
    /// Power-on state: VRAM/CGRAM/OAM zero-filled (documented constant,
    /// D3), registers at documented reset values, force-blank on.
    pub fn new(vram: &'static mut [u8; 0x10000]) -> Ppu {
        Ppu {
            vram,
            cgram: [0u8; 512],
            oam: [0u8; 544],
            back: Box::new([0u8; FB_BYTES]),

            force_blank: true, // power-on: force blank active
            brightness: 0,

            obsel: Obsel::default(),

            oam_base_addr: 0,
            oam_priority_rotation: false,
            oam_addr: 0,
            oam_write_latch: 0,
            oam_latch_valid: false,

            bg_mode: 0,
            bg3_priority: false,
            bg_tile_size: [false; 4],

            bg_sc: [BgSc::default(); 4],
            bg_nba: BgNba::default(),
            bg_scroll: [BgScroll::default(); 4],
            offset_latch: 0,

            vmain: Vmain::default(),
            vmadd: 0,
            vram_prefetch: 0,

            cgram_addr: 0,
            cgdata_latch: 0,
            cgdata_high: false,

            w12sel: 0,
            w34sel: 0,
            wobjsel: 0,
            wh0: 0,
            wh1: 0,
            wh2: 0,
            wh3: 0,
            wbglog: 0,
            wobjlog: 0,

            tm: 0,
            ts: 0,
            tmw: 0,
            tsw: 0,

            cgwsel: 0,
            cgadsub: 0,
            coldata_color: 0,
            setini: 0,

            m7sel: 0,
            m7a: 0,
            m7b: 0,
            m7c: 0,
            m7d: 0,
            m7x: 0,
            m7y: 0,
            m7_latch: 0,

            ophct: 0,
            opvct: 0,
            ophct_read_high: false,
            opvct_read_high: false,
            counter_latched: false,
            cgram_read_high: false,

            cur_line: 0,

            mosaic_size: 0,
            mosaic_bg_enable: 0,
            mosaic_start_line: 1,

            sub_pixels: Box::new([0u8; 256]),
            win1_mask: Box::new([0u8; 256]),
            win2_mask: Box::new([0u8; 256]),
            main_win_mask: Box::new([[0u8; 256]; 5]),
            sub_win_mask: Box::new([[0u8; 256]; 5]),
            math_win_mask: Box::new([0u8; 256]),
        }
    }

    // ────────────────────────────────────────────────────────────
    // Internal helpers
    // ────────────────────────────────────────────────────────────

    /// Apply VMAIN address-translation remap to `addr`.
    ///
    /// Remap modes (bits [3:2] of $2115):
    ///   0: no remap
    ///   1: rotate 8 bits (aaa aaaa aaaa aaaa → aaa aaaa aaaa aaaa with bits [10:3]←[7:0] and [2:0] kept)
    ///      Documented: remap for 2bpp tiles: addr = (addr & 0xFF00) | ((addr & 0x00E0) >> 5) | ((addr & 0x001F) << 3)
    ///   2: rotate 9 bits (for 4bpp)
    ///   3: rotate 10 bits (for 8bpp)
    fn apply_vram_remap(addr: u16, remap: u8) -> u16 {
        match remap {
            0 => addr,
            1 => {
                // 2-bit rotation (for 2bpp): bits [10:3] come from [7:0], bits [2:0] from addr[10:8]
                // Documented: swap lower 3 bits with upper portion of low byte
                // addr = aaa aaaa bbb ccccc → remap: row-based for 8px-wide 2bpp tiles
                // (addr & ~0xFF) | ((addr & 0x1F) << 3) | ((addr & 0xE0) >> 5)
                (addr & 0xFF00) | ((addr & 0x001F) << 3) | ((addr & 0x00E0) >> 5)
            }
            2 => {
                // 4bpp remap: 9-bit rotation
                (addr & 0xFE00) | ((addr & 0x003F) << 3) | ((addr & 0x01C0) >> 6)
            }
            _ => {
                // 8bpp remap: 10-bit rotation
                (addr & 0xFC00) | ((addr & 0x007F) << 3) | ((addr & 0x0380) >> 7)
            }
        }
    }

    /// Compute the VRAM word-address after applying remap.
    fn vram_remapped_addr(&self) -> u16 {
        Self::apply_vram_remap(self.vmadd, self.vmain.remap)
    }

    /// Load the VRAM prefetch buffer from the current (remapped) address.
    fn reload_prefetch(&mut self) {
        let addr = self.vram_remapped_addr() as usize;
        let lo = self.vram[(addr * 2) & 0xFFFF] as u16;
        let hi = self.vram[(addr * 2 + 1) & 0xFFFF] as u16;
        self.vram_prefetch = lo | (hi << 8);
    }

    /// Increment VMADD by the configured step.
    fn increment_vmadd(&mut self) {
        self.vmadd = self.vmadd.wrapping_add(self.vmain.step);
    }

    /// Write a word to VRAM at the remapped address.
    fn vram_write_lo(&mut self, value: u8) {
        let addr = self.vram_remapped_addr() as usize;
        self.vram[(addr * 2) & 0xFFFF] = value;
        if !self.vmain.inc_on_high {
            self.increment_vmadd();
            self.reload_prefetch();
        }
    }

    fn vram_write_hi(&mut self, value: u8) {
        let addr = self.vram_remapped_addr() as usize;
        self.vram[(addr * 2 + 1) & 0xFFFF] = value;
        if self.vmain.inc_on_high {
            self.increment_vmadd();
            self.reload_prefetch();
        }
    }

    // ────────────────────────────────────────────────────────────
    // Register write implementation
    // ────────────────────────────────────────────────────────────

    /// Write PPU register `$2100 + reg` (reg in 0x00..=0x33).
    /// Returns a fault if the write enables an M1-unimplemented feature.
    pub fn write(&mut self, reg: u8, value: u8) -> Option<Fault> {
        match reg {
            // ── $2100 INIDISP ──────────────────────────────────
            0x00 => {
                self.force_blank = (value & 0x80) != 0;
                self.brightness = value & 0x0F;
            }

            // ── $2101 OBSEL ────────────────────────────────────
            0x01 => {
                self.obsel.size = (value >> 5) & 0x07;
                self.obsel.name_base = ((value >> 3) & 0x03) as u16;
                self.obsel.name_select = ((value >> 1) & 0x03) as u16;
            }

            // ── $2102/$2103 OAMADD ─────────────────────────────
            0x02 => {
                self.oam_base_addr = (self.oam_base_addr & 0x100) | (value as u16);
                self.oam_addr = self.oam_base_addr << 1;
                self.oam_latch_valid = false;
            }
            0x03 => {
                self.oam_base_addr = (self.oam_base_addr & 0x0FF) | (((value & 1) as u16) << 8);
                self.oam_priority_rotation = (value & 0x80) != 0;
                self.oam_addr = self.oam_base_addr << 1;
                self.oam_latch_valid = false;
            }

            // ── $2104 OAMDATA ──────────────────────────────────
            0x04 => {
                let addr = self.oam_addr & 0x3FF;
                if addr < 512 {
                    // Low table: documented write-pair latch behavior.
                    // First write (even addr): latch the byte.
                    // Second write (odd addr): commit both bytes.
                    if addr & 1 == 0 {
                        self.oam_write_latch = value;
                        self.oam_latch_valid = true;
                    } else if self.oam_latch_valid {
                        let base = (addr & !1) as usize;
                        self.oam[base] = self.oam_write_latch;
                        self.oam[base + 1] = value;
                        self.oam_latch_valid = false;
                    }
                } else {
                    // High table: 32 bytes written directly.
                    let hi_idx = (addr - 512) as usize;
                    if hi_idx < 32 {
                        self.oam[512 + hi_idx] = value;
                    }
                }
                self.oam_addr = (self.oam_addr + 1) & 0x3FF;
            }

            // ── $2105 BGMODE ───────────────────────────────────
            0x05 => {
                let mode = value & 0x07;
                // Modes 0, 1, 3, 7 are implemented; others fault (D9).
                match mode {
                    0 | 1 | 3 | 7 => {}
                    _ => return Some(Fault::UnimplementedBgMode { mode }),
                }
                self.bg_mode = mode;
                self.bg3_priority = (value & 0x08) != 0;
                for i in 0..4 {
                    self.bg_tile_size[i] = (value >> (4 + i)) & 1 != 0;
                }
            }

            // ── $2106 MOSAIC ───────────────────────────────────
            // bits [7:4] = mosaic size (0-15), bits [3:0] = per-BG enable.
            0x06 => {
                self.mosaic_size = (value >> 4) & 0x0F;
                self.mosaic_bg_enable = value & 0x0F;
                // Reset the mosaic start line to the current scanline so that
                // a mid-frame write takes effect from the next complete block.
                // Documented: new block starts at current line (approximate;
                // cycle-exact behavior not modeled).
                self.mosaic_start_line = if self.cur_line >= 1 { self.cur_line } else { 1 };
            }

            // ── $2107–$210A BGnSC ──────────────────────────────
            0x07..=0x0A => {
                let idx = (reg - 0x07) as usize;
                // Base address: bits [7:2] → 2KiB-word granularity → word addr = bits[7:2] << 10
                // In bytes: bits[7:2] << 11
                self.bg_sc[idx].base = ((value & 0xFC) as u16) << 9;
                self.bg_sc[idx].h_wide = (value & 0x01) != 0;
                self.bg_sc[idx].v_wide = (value & 0x02) != 0;
            }

            // ── $210B BG12NBA ──────────────────────────────────
            0x0B => {
                // Each nibble: 4 KiW unit → byte addr = nibble << 13
                self.bg_nba.bg1_base = ((value & 0x0F) as u16) << 13;
                self.bg_nba.bg2_base = (((value >> 4) & 0x0F) as u16) << 13;
            }

            // ── $210C BG34NBA ──────────────────────────────────
            0x0C => {
                self.bg_nba.bg3_base = ((value & 0x0F) as u16) << 13;
                self.bg_nba.bg4_base = (((value >> 4) & 0x0F) as u16) << 13;
            }

            // ── $210D–$2114 BGnHOFS/VOFS ───────────────────────
            // Shared write-twice latch (prev byte latch):
            // HOFS: new[7:0] | old_latch → value & (prev<<8) but only low 10 bits
            // VOFS: full 10-bit from (value << 8 | old_latch) - but documented behavior:
            //   HOFS: {value[7:0], old_latch[7:3]} → 13 bits, masked to 10
            //   VOFS: {value[7:0], old_latch[7:0]} → 16 bits, masked to 10
            // Actually the standard documented behavior:
            //   Write 1 (HOFS): latch = value → effective_h = (value << 8 | prev_latch) but 10-bit
            //     actually: hofs = ((value & 3) << 8) | prev_latch ... let's follow the correct doc:
            //   HOFS write: h = (value << 3) | (prev_latch & 7); prev_latch = value
            //   VOFS write: v = (value << 8 | prev_latch) & 0x3FF; prev_latch = value
            // Correct documented: Mode 0-6 standard BG scroll:
            //   BGnHOFS (low-byte reg written first):
            //     temp = (value << 8) | (prev & ~7) | ((BGnHOFS >> 8) & 7)
            //     BGnHOFS = temp & 0x3FF   (or full 13 bits for offset-per-tile modes)
            //   BGnVOFS: BGnVOFS = ((value << 8) | prev) & 0x3FF
            //   prev_latch updated after both writes
            // The simplest widely-documented rule that works for M1 (whole-frame):
            //   HOFS: store (value | (prev << 8)) & 0x1FFF (13-bit M7 or 10-bit BG)
            //   VOFS: store ((value << 8) | prev) & 0x01FF (9-bit)
            // We use the standard 16-bit console documented rule:
            0x0D => self.write_bg_hofs(0, value),
            0x0E => self.write_bg_vofs(0, value),
            0x0F => self.write_bg_hofs(1, value),
            0x10 => self.write_bg_vofs(1, value),
            0x11 => self.write_bg_hofs(2, value),
            0x12 => self.write_bg_vofs(2, value),
            0x13 => self.write_bg_hofs(3, value),
            0x14 => self.write_bg_vofs(3, value),

            // ── $2115 VMAIN ────────────────────────────────────
            0x15 => {
                self.vmain.inc_on_high = (value & 0x80) != 0;
                self.vmain.step = match value & 0x03 {
                    0 => 1,
                    1 => 32,
                    // Steps 0b10 and 0b11 both select 128 per the documented
                    // register table (the collapse is intentional).
                    _ => 128,
                };
                self.vmain.remap = (value >> 2) & 0x03;
            }

            // ── $2116/$2117 VMADD ──────────────────────────────
            0x16 => {
                self.vmadd = (self.vmadd & 0xFF00) | (value as u16);
                self.reload_prefetch();
            }
            0x17 => {
                self.vmadd = (self.vmadd & 0x00FF) | ((value as u16) << 8);
                self.reload_prefetch();
            }

            // ── $2118/$2119 VMDATA L/H ─────────────────────────
            0x18 => self.vram_write_lo(value),
            0x19 => self.vram_write_hi(value),

            // ── $211A M7SEL ────────────────────────────────────
            0x1A => self.m7sel = value,

            // ── $211B–$2120 mode-7 params (store silently) ─────
            0x1B => {
                self.m7a = ((value as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }
            0x1C => {
                self.m7b = ((value as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }
            0x1D => {
                self.m7c = ((value as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }
            0x1E => {
                self.m7d = ((value as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }
            0x1F => {
                self.m7x = (((value & 0x1F) as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }
            0x20 => {
                self.m7y = (((value & 0x1F) as i16) << 8) | (self.m7_latch as i16);
                self.m7_latch = value;
            }

            // ── $2121 CGADD ────────────────────────────────────
            0x21 => {
                self.cgram_addr = value;
                self.cgdata_high = false;
            }

            // ── $2122 CGDATA ───────────────────────────────────
            0x22 => {
                if !self.cgdata_high {
                    self.cgdata_latch = value;
                    self.cgdata_high = true;
                } else {
                    let idx = (self.cgram_addr as usize) * 2;
                    self.cgram[idx] = self.cgdata_latch;
                    self.cgram[idx + 1] = value & 0x7F; // only 15 bits (top bit ignored)
                    self.cgdata_high = false;
                    self.cgram_addr = self.cgram_addr.wrapping_add(1);
                }
            }

            // ── $2123 W12SEL ───────────────────────────────────
            // Bits per layer: [7:6]=W2 BG2 enable/invert, [5:4]=W1 BG2, [3:2]=W2 BG1, [1:0]=W1 BG1
            0x23 => self.w12sel = value,
            // ── $2124 W34SEL ───────────────────────────────────
            0x24 => self.w34sel = value,
            // ── $2125 WOBJSEL ──────────────────────────────────
            // [7:6]=W2 OBJ, [5:4]=W1 OBJ, [3:2]=W2 COL, [1:0]=W1 COL
            0x25 => self.wobjsel = value,

            // ── $2126–$2129 WH0-WH3 (window position) ─────────
            0x26 => self.wh0 = value,
            0x27 => self.wh1 = value,
            0x28 => self.wh2 = value,
            0x29 => self.wh3 = value,

            // ── $212A WBGLOG ───────────────────────────────────
            0x2A => self.wbglog = value,
            // ── $212B WOBJLOG ──────────────────────────────────
            0x2B => self.wobjlog = value,

            // ── $212C TM (main-screen layer enables) ───────────
            0x2C => self.tm = value & 0x1F,

            // ── $212D TS (sub-screen layer enables) ────────────
            // Stored but ignored in M1 — no subscreen blending implemented.
            0x2D => self.ts = value & 0x1F,

            // ── $212E TMW (main-screen window mask enable) ─────
            0x2E => self.tmw = value,
            // ── $212F TSW (sub-screen window mask enable) ───────
            0x2F => self.tsw = value,

            // ── $2130 CGWSEL ───────────────────────────────────
            // Bits [7:6]: color-math window clip region (0=never,1=in-win,2=out-win,3=always)
            // Bits [5:4]: color-math enabled region (same encoding)
            // Bit 1: sub-screen black = 1, direct color mode (indirect not implemented → fault)
            // Bit 0: direct color mode — fault (not implemented; indirect-color corner)
            0x30 => {
                self.cgwsel = value;
                // Direct-color mode (bit 0): fault (D9 — not implemented).
                if value & 0x01 != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
                // Sub-screen black (bit 1) is handled in the compositor.
            }
            // ── $2131 CGADSUB ──────────────────────────────────
            // Bit 7: add (0) / subtract (1); bit 6: half; bits [5:0]: layer enables
            0x31 => self.cgadsub = value,

            // ── $2132 COLDATA ──────────────────────────────────
            // Fixed color: bit 7=B, bit 6=G, bit 5=R selects plane; bits[4:0]=value.
            // Writes can set/update each component independently.
            // We pack all three 5-bit components into `coldata` extended to u16.
            // To keep the register file simple we use a 16-bit shadow in coldata
            // reinterpreted as (B5<<10|G5<<5|R5<<0) = BGR555 fixed-color.
            0x32 => {
                let plane_val = (value & 0x1F) as u16;
                // bits[7:5] select which component(s) to update; bits[4:0] = 5-bit value.
                self.set_coldata_component(value >> 5, plane_val);
            }

            // ── $2133 SETINI ───────────────────────────────────
            // Bit 6: EXTBG — fault (not implemented; affects mode-7 extra BG).
            // Bits 3/0: overscan/interlace — fault (not implemented).
            // Bit 2: pseudo-hires — fault (not implemented).
            // Bit 1: BG2 area (mode-7 only) — fault when mode-7 EXTBG.
            0x33 => {
                self.setini = value;
                if value & 0b0100_1111 != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
                // Bit 6 (EXTBG) only faults when mode 7 is active. Store it
                // and fault at render time when relevant.
            }

            // Writes to read-only registers ($2134–$213F) or beyond scope:
            // PPU open bus — return without action (deterministic, no fault).
            _ => {}
        }
        None
    }

    /// Update the fixed-color component selected by `plane_bits` (bit2=B,bit1=G,bit0=R).
    /// `val5` is the 5-bit component value.
    fn set_coldata_component(&mut self, plane_bits: u8, val5: u16) {
        if plane_bits & 0x01 != 0 {
            // Red
            self.coldata_color = (self.coldata_color & !0x001F) | (val5 & 0x1F);
        }
        if plane_bits & 0x02 != 0 {
            // Green
            self.coldata_color = (self.coldata_color & !0x03E0) | ((val5 & 0x1F) << 5);
        }
        if plane_bits & 0x04 != 0 {
            // Blue
            self.coldata_color = (self.coldata_color & !0x7C00) | ((val5 & 0x1F) << 10);
        }
    }

    /// Compute per-pixel window masks for the current line and store into the
    /// pre-allocated arrays. Called once per rendered line before compositing.
    ///
    /// Window SEL register layout (W12SEL = $2123):
    ///   bits [1:0] = W1 BG1: bit0=enable, bit1=invert
    ///   bits [3:2] = W2 BG1: bit2=enable, bit3=invert
    ///   bits [5:4] = W1 BG2: bit4=enable, bit5=invert
    ///   bits [7:6] = W2 BG2: bit6=enable, bit7=invert
    /// W34SEL ($2124) same layout for BG3/BG4.
    /// WOBJSEL ($2125):
    ///   bits [1:0] = W1 OBJ: bit0=enable, bit1=invert
    ///   bits [3:2] = W2 OBJ: bit2=enable, bit3=invert
    ///   bits [5:4] = W1 COL: bit4=enable, bit5=invert  (color-math window)
    ///   bits [7:6] = W2 COL: bit6=enable, bit7=invert
    ///
    /// WBGLOG ($212A): BG1/2/3/4 combination ops; WOBJLOG ($212B): OBJ/COL ops.
    /// TMW ($212E): main-screen window mask enable per layer.
    /// TSW ($212F): sub-screen window mask enable per layer.
    fn compute_window_masks(&mut self) {
        // Build the two raw window pixel masks once.
        window_range_mask(self.wh0, self.wh1, &mut self.win1_mask);
        window_range_mask(self.wh2, self.wh3, &mut self.win2_mask);

        // Helper: build combined mask for one layer given its W1/W2 enable/invert bits
        // and the 2-bit combination op from a log register.
        // sel_bits: [bit0]=W1_enable, [bit1]=W1_invert, [bit2]=W2_enable, [bit3]=W2_invert
        // combination op: 2-bit value (0=OR,1=AND,2=XOR,3=XNOR)
        // Returns a 256-entry mask where 1 = pixel is inside the combined window.
        let build_layer_mask = |win1: &[u8; 256], win2: &[u8; 256], sel: u8, op: u8| -> [u8; 256] {
            let w1_en = (sel & 0x01) != 0;
            let w1_inv = (sel & 0x02) != 0;
            let w2_en = (sel & 0x04) != 0;
            let w2_inv = (sel & 0x08) != 0;
            let mut out = [0u8; 256];
            for x in 0..256usize {
                let m1 = if w1_en {
                    if w1_inv {
                        1 - win1[x]
                    } else {
                        win1[x]
                    }
                } else {
                    0
                };
                let m2 = if w2_en {
                    if w2_inv {
                        1 - win2[x]
                    } else {
                        win2[x]
                    }
                } else {
                    0
                };
                out[x] = if w1_en && w2_en {
                    window_combine(m1, m2, op)
                } else if w1_en {
                    m1
                } else if w2_en {
                    m2
                } else {
                    0 // neither window enabled → no masking
                };
            }
            out
        };

        // WBGLOG bits: [1:0]=BG1 op, [3:2]=BG2 op, [5:4]=BG3 op, [7:6]=BG4 op
        // WOBJLOG bits: [1:0]=OBJ op, [3:2]=COL op

        // BG1 (layer index 0): W12SEL bits[3:0]
        let bg1_sel = self.w12sel & 0x0F;
        let bg1_op = self.wbglog & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, bg1_sel, bg1_op);
        self.main_win_mask[0].copy_from_slice(&m);
        self.sub_win_mask[0].copy_from_slice(&m);

        // BG2 (layer index 1): W12SEL bits[7:4]
        let bg2_sel = (self.w12sel >> 4) & 0x0F;
        let bg2_op = (self.wbglog >> 2) & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, bg2_sel, bg2_op);
        self.main_win_mask[1].copy_from_slice(&m);
        self.sub_win_mask[1].copy_from_slice(&m);

        // BG3 (layer index 2): W34SEL bits[3:0]
        let bg3_sel = self.w34sel & 0x0F;
        let bg3_op = (self.wbglog >> 4) & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, bg3_sel, bg3_op);
        self.main_win_mask[2].copy_from_slice(&m);
        self.sub_win_mask[2].copy_from_slice(&m);

        // BG4 (layer index 3): W34SEL bits[7:4]
        let bg4_sel = (self.w34sel >> 4) & 0x0F;
        let bg4_op = (self.wbglog >> 6) & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, bg4_sel, bg4_op);
        self.main_win_mask[3].copy_from_slice(&m);
        self.sub_win_mask[3].copy_from_slice(&m);

        // OBJ (layer index 4): WOBJSEL bits[3:0]
        let obj_sel = self.wobjsel & 0x0F;
        let obj_op = self.wobjlog & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, obj_sel, obj_op);
        self.main_win_mask[4].copy_from_slice(&m);
        self.sub_win_mask[4].copy_from_slice(&m);

        // Color-math window (for CGWSEL): WOBJSEL bits[7:4]
        let col_sel = (self.wobjsel >> 4) & 0x0F;
        let col_op = (self.wobjlog >> 2) & 0x03;
        let m = build_layer_mask(&self.win1_mask, &self.win2_mask, col_sel, col_op);
        self.math_win_mask.copy_from_slice(&m);
    }

    /// Write BGnHOFS using the documented shared write-twice offset latch.
    ///
    /// BGnHOFS documented behavior:
    ///   hofs = (value << 8) | (prev_latch & ~7) | (hofs_old >> 8) & 7
    ///   → simplified for M1 whole-frame: hofs = ((value << 3) | (prev_latch >> 5)) but this varies.
    ///   Standard: HOFS writes use prev_latch for bits[7:3], value for bits[10:8].
    ///   Effective: hofs[7:0] = prev_latch, hofs[9:8] = value[1:0] (for modes 0-6)
    ///              actually: hofs = { value[2:0], prev_latch } & 0x3FF
    fn write_bg_hofs(&mut self, bg: usize, value: u8) {
        // Documented rule for BGnHOFS (applies to modes 0-6 standard BG):
        // hofs = (value & 3) << 8 | prev_latch
        // hofs[9:8] = value[1:0], hofs[7:0] = prev_latch
        self.bg_scroll[bg].hofs = ((value as u16 & 0x03) << 8) | self.offset_latch as u16;
        self.offset_latch = value;
    }

    /// Write BGnVOFS using the documented shared write-twice offset latch.
    fn write_bg_vofs(&mut self, bg: usize, value: u8) {
        // vofs[9:8] = value[1:0], vofs[7:0] = prev_latch
        self.bg_scroll[bg].vofs = ((value as u16 & 0x03) << 8) | self.offset_latch as u16;
        self.offset_latch = value;
    }

    // ────────────────────────────────────────────────────────────
    // Register read implementation
    // ────────────────────────────────────────────────────────────

    /// Read PPU register `$2100 + reg` (the $2134..=$213F read block plus
    /// the readable ports). `mdr` is the open-bus value for undriven bits.
    pub fn read(&mut self, reg: u8, mdr: u8) -> u8 {
        match reg {
            // ── $2134–$2136 MPYL/M/H ───────────────────────────
            // Signed M7A × signed M7B(low 8 bits) product.
            0x34 => {
                let prod = (self.m7a as i32) * ((self.m7b as i8) as i32);
                (prod & 0xFF) as u8
            }
            0x35 => {
                let prod = (self.m7a as i32) * ((self.m7b as i8) as i32);
                ((prod >> 8) & 0xFF) as u8
            }
            0x36 => {
                let prod = (self.m7a as i32) * ((self.m7b as i8) as i32);
                ((prod >> 16) & 0xFF) as u8
            }

            // ── $2137 SLHV ─────────────────────────────────────
            // Latch H/V counters. H dot position is provided by the bus via
            // `latch_hv_counters`; reading $2137 triggers the latch.
            0x37 => {
                // The bus calls latch_hv_counters with the current dot before
                // dispatching this read; cur_line / ophct are already set.
                // Nothing additional to do here except set counter_latched.
                self.counter_latched = true;
                self.ophct_read_high = false;
                self.opvct_read_high = false;
                mdr // $2137 reads return open bus
            }

            // ── $2138 RDOAM ────────────────────────────────────
            0x38 => {
                let addr = (self.oam_addr & 0x3FF) as usize;
                let val = if addr < 544 { self.oam[addr] } else { mdr };
                self.oam_addr = (self.oam_addr + 1) & 0x3FF;
                val
            }

            // ── $2139 RDVRAM L ─────────────────────────────────
            // Returns low byte of prefetch buffer; increments address if
            // inc_on_high=false.
            0x39 => {
                let val = (self.vram_prefetch & 0xFF) as u8;
                if !self.vmain.inc_on_high {
                    self.increment_vmadd();
                    self.reload_prefetch();
                }
                val
            }

            // ── $213A RDVRAM H ─────────────────────────────────
            0x3A => {
                let val = ((self.vram_prefetch >> 8) & 0xFF) as u8;
                if self.vmain.inc_on_high {
                    self.increment_vmadd();
                    self.reload_prefetch();
                }
                val
            }

            // ── $213B RDCGRAM ──────────────────────────────────
            // Read-twice latch: first read returns low 8 bits, second returns high 7 bits
            // with high bit from mdr (open bus).
            0x3B => {
                let idx = self.cgram_addr as usize * 2;

                if !self.cgram_read_high {
                    self.cgram_read_high = true;
                    self.cgram[idx]
                } else {
                    self.cgram_read_high = false;
                    let hi = self.cgram[idx + 1] & 0x7F;
                    let result = (mdr & 0x80) | hi;
                    self.cgram_addr = self.cgram_addr.wrapping_add(1);
                    result
                }
            }

            // ── $213C OPHCT ────────────────────────────────────
            // Double-read: first = bits[7:0], second = bits[8] in bit0, high from mdr.
            0x3C => {
                if !self.ophct_read_high {
                    self.ophct_read_high = true;
                    (self.ophct & 0xFF) as u8
                } else {
                    self.ophct_read_high = false;
                    (mdr & 0xFE) | ((self.ophct >> 8) & 1) as u8
                }
            }

            // ── $213D OPVCT ────────────────────────────────────
            0x3D => {
                if !self.opvct_read_high {
                    self.opvct_read_high = true;
                    (self.opvct & 0xFF) as u8
                } else {
                    self.opvct_read_high = false;
                    (mdr & 0xFE) | ((self.opvct >> 8) & 1) as u8
                }
            }

            // ── $213E STAT77 ───────────────────────────────────
            // Bits [7:5] = open bus (mdr), bit 4 = sprite overflow (0), bit 3 = sprite 0 hit (0),
            // bits [3:0] ... bit [0] = version. Version = 1, flags = 0.
            // Documented: bits [3:0] = PPU1 version (1), bit 5 = range overflow, bit 6 = time overflow.
            // Return: (mdr & 0x10) | 0x01  but more precisely per docs:
            // STAT77: v oooo 001  → bit7-5 = open bus, bit4 = range-over, bit3 = time-over (both 0), bits[2:0] = version=1
            0x3E => (mdr & 0x20) | 0x01,

            // ── $213F STAT78 ───────────────────────────────────
            // Bit 7 = interlace field (0), bit 6 = counter latch flag, bit 5 = open bus,
            // bit 4 = NTSC=0/PAL=1 (NTSC → 0), bits [3:0] = PPU2 version = 1.
            // Reading clears the counter latch flag and resets $213C/$213D read latches.
            0x3F => {
                let latch_bit = if self.counter_latched { 0x40 } else { 0 };
                self.counter_latched = false;
                self.ophct_read_high = false;
                self.opvct_read_high = false;
                (mdr & 0x20) | latch_bit | 0x01
            }

            // Reads of write-only registers or unknown: return mdr (PPU open bus).
            _ => mdr,
        }
    }

    // ────────────────────────────────────────────────────────────
    // Scanline rendering
    // ────────────────────────────────────────────────────────────

    /// Render visible scanline `line` (1..=224) into back-buffer row
    /// `line - 1`, honoring force blank and brightness.
    pub fn render_scanline(&mut self, line: u16) {
        self.cur_line = line;

        let row = (line - 1) as usize;
        let row_start = row * 1024;

        if self.force_blank {
            for i in 0..256 {
                let off = row_start + i * 4;
                self.back[off] = 0;
                self.back[off + 1] = 0;
                self.back[off + 2] = 0;
                self.back[off + 3] = 0;
            }
            return;
        }

        // Compute window masks for this line (used by compositor).
        self.compute_window_masks();

        match self.bg_mode {
            0 => self.render_mode0(row_start, line),
            1 => self.render_mode1(row_start, line),
            3 => self.render_mode3(row_start, line),
            7 => self.render_mode7(row_start, line),
            _ => {
                // Should have been caught at BGMODE write; render black (D9).
                for i in 0..256 {
                    let off = row_start + i * 4;
                    self.back[off] = 0;
                    self.back[off + 1] = 0;
                    self.back[off + 2] = 0;
                    self.back[off + 3] = 0;
                }
            }
        }
    }

    /// Apply mosaic to `line` for BG `bg_idx` (0-indexed):
    /// returns the effective "source" line for tile fetching.
    /// Mosaic snaps the fetch line to the start of the current NxN block.
    #[inline]
    fn mosaic_line(&self, line: u16, bg_idx: usize) -> u16 {
        if self.mosaic_bg_enable & (1 << bg_idx) == 0 {
            return line; // mosaic not enabled for this BG
        }
        let size = (self.mosaic_size as u16) + 1;
        let start = self.mosaic_start_line;
        // Offset from start of frame's mosaic group.
        let offset = line.saturating_sub(start);
        let block_start = offset / size * size;
        start + block_start
    }

    /// Apply mosaic to horizontal pixel coordinate x for BG `bg_idx`.
    #[inline]
    fn mosaic_x(&self, x: u16, bg_idx: usize) -> u16 {
        if self.mosaic_bg_enable & (1 << bg_idx) == 0 {
            return x;
        }
        let size = (self.mosaic_size as u16) + 1;
        (x / size) * size
    }

    /// Layer visibility on the main screen at `x`: opaque pixel, not masked
    /// away by its window (TMW bit; layer 0..=3 = BG1..BG4, 4 = OBJ).
    #[inline]
    fn main_visible(&self, transparent: bool, layer: usize, x: usize) -> bool {
        !(transparent || (self.tmw & (1 << layer) != 0 && self.main_win_mask[layer][x] != 0))
    }

    /// Layer visibility on the sub screen at `x` (TSW bit, sub window mask).
    #[inline]
    fn sub_visible(&self, transparent: bool, layer: usize, x: usize) -> bool {
        !(transparent || (self.tsw & (1 << layer) != 0 && self.sub_win_mask[layer][x] != 0))
    }

    /// Composite a main + sub pixel with color math and write to the back buffer.
    ///
    /// `main_cgram` and `sub_cgram` are CGRAM indices (0 = backdrop for sub).
    /// `main_is_backdrop` indicates the main pixel is the backdrop (color 0).
    /// `x` is the horizontal pixel position.
    /// `math_enabled` indicates this layer participates in color math.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn composite_pixel(
        &mut self,
        row_start: usize,
        x: usize,
        main_cgram: usize,
        sub_cgram: usize,
        main_is_backdrop: bool,
        math_enabled: bool,
    ) {
        // CGWSEL bits [7:6]: clip color to black (main screen clip region).
        //   0=never clip, 1=clip inside windows, 2=clip outside windows, 3=always
        // CGWSEL bits [5:4]: color math enable region.
        //   0=always, 1=inside windows, 2=outside windows, 3=never
        // CGWSEL bit 1: sub-screen black (force sub to black when disabled)

        let cgwsel = self.cgwsel;
        let cgadsub = self.cgadsub;

        // Determine if color-math applies to this pixel position.
        let math_region = (cgwsel >> 4) & 0x03;
        let in_math_window = self.math_win_mask[x];
        let do_math = math_enabled
            && match math_region {
                0 => true,                // always
                1 => in_math_window != 0, // inside window
                2 => in_math_window == 0, // outside window
                _ => false,               // never
            };

        // Main screen clip-to-black region.
        let clip_region = (cgwsel >> 6) & 0x03;
        let in_clip_window = self.math_win_mask[x];
        let clip_main = match clip_region {
            0 => false,
            1 => in_clip_window != 0,
            2 => in_clip_window == 0,
            3 => true,
            _ => false,
        };

        // Get main-screen color components.
        let (main_r, main_g, main_b) = if clip_main {
            (0u8, 0u8, 0u8) // clipped to black
        } else {
            cgram_color_components(&self.cgram, main_cgram)
        };

        let final_color = if do_math && !main_is_backdrop {
            // Determine sub-screen color operand.
            // CGWSEL bit 1: sub-screen black → use fixed black (0,0,0) as operand.
            let sub_black = (cgwsel & 0x02) != 0;
            let (sub_r, sub_g, sub_b) = if sub_black || sub_cgram == 0 {
                // Use fixed-color when sub is backdrop or sub-screen black.
                // CGADSUB bit 7: 0=add(use fixed or subscreen), 1=subtract.
                // When sub_black=0 and sub_cgram=0: use fixed color register.
                cgram_color_components_fixed(self.coldata_color)
            } else {
                cgram_color_components(&self.cgram, sub_cgram)
            };

            let add = (cgadsub & 0x80) == 0; // bit7=0 → add, bit7=1 → sub
            let half = (cgadsub & 0x40) != 0;
            color_math_op(main_r, main_g, main_b, sub_r, sub_g, sub_b, add, half)
        } else {
            // No math: use raw main color
            (main_r as u16) | ((main_g as u16) << 5) | ((main_b as u16) << 10)
        };

        let pixel = bgr555_to_xrgb8888(final_color, self.brightness);
        let off = row_start + x * 4;
        self.back[off] = (pixel & 0xFF) as u8;
        self.back[off + 1] = ((pixel >> 8) & 0xFF) as u8;
        self.back[off + 2] = ((pixel >> 16) & 0xFF) as u8;
        self.back[off + 3] = 0;
    }

    /// Render one mode-0 scanline.
    ///
    /// Mode 0: 4 BGs, all 2bpp.
    /// Per-BG palette offsets (documented): BG1=0, BG2=32, BG3=64, BG4=96.
    /// Priority order (high → low):
    ///   OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3hi, BG4hi, OBJ0, BG3lo, BG4lo, backdrop
    fn render_mode0(&mut self, row_start: usize, line: u16) {
        let mut bg_pixels: [[BgPixel; 256]; 4] = [[BgPixel::TRANSPARENT; 256]; 4];
        let mut spr_pixels = [SpritePixel::TRANSPARENT; 256];

        // BG palette bases for mode 0 (0, 32, 64, 96)
        let bg_palette_bases: [u8; 4] = [0, 32, 64, 96];

        for bg in 0..4usize {
            if self.tm & (1 << bg) == 0 {
                continue;
            }
            let tile_size = if self.bg_tile_size[bg] { 16 } else { 8 };
            let data_base = self.bg_tile_data_base(bg) as usize;
            let fetch_line = self.mosaic_line(line - 1, bg);
            render_bg_line(
                self.vram,
                &mut bg_pixels[bg],
                self.bg_sc[bg],
                self.bg_scroll[bg],
                data_base,
                2,
                bg_palette_bases[bg],
                tile_size,
                fetch_line,
            );
        }
        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        // Build sub-screen pixels for color math.
        self.build_sub_pixels_mode0(&bg_pixels, &spr_pixels);

        // CGADSUB: per-layer math enable bits [5:0]: BG1,BG2,BG3,BG4,OBJ,backdrop
        let cgadsub = self.cgadsub;

        #[allow(clippy::needless_range_loop)] // x indexes several parallel line buffers
        for x in 0..256usize {
            let sp = spr_pixels[x];
            // Per-BG mosaic quantization (matches the mode-1/3 paths).
            let b0 = bg_pixels[0][self.mosaic_x(x as u16, 0) as usize];
            let b1 = bg_pixels[1][self.mosaic_x(x as u16, 1) as usize];
            let b2 = bg_pixels[2][self.mosaic_x(x as u16, 2) as usize];
            let b3 = bg_pixels[3][self.mosaic_x(x as u16, 3) as usize];

            // Window masking: layer is hidden if its main-win-mask pixel = 1 and TMW bit is set.
            let b0_vis = self.main_visible(b0.is_transparent(), 0, x);
            let b1_vis = self.main_visible(b1.is_transparent(), 1, x);
            let b2_vis = self.main_visible(b2.is_transparent(), 2, x);
            let b3_vis = self.main_visible(b3.is_transparent(), 3, x);
            let sp_vis = self.main_visible(sp.is_transparent(), 4, x);

            let (main_cgram, main_is_backdrop, math_en) = 'pick: {
                if sp_vis && sp.priority == 3 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b0_vis && b0.priority {
                    break 'pick (b0.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b1_vis && b1.priority {
                    break 'pick (b1.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 2 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b0_vis && !b0.priority {
                    break 'pick (b0.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b1_vis && !b1.priority {
                    break 'pick (b1.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 1 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b2_vis && b2.priority {
                    break 'pick (b2.cgram_idx as usize, false, cgadsub & 0x04 != 0);
                }
                if b3_vis && b3.priority {
                    break 'pick (b3.cgram_idx as usize, false, cgadsub & 0x08 != 0);
                }
                if sp_vis && sp.priority == 0 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b2_vis && !b2.priority {
                    break 'pick (b2.cgram_idx as usize, false, cgadsub & 0x04 != 0);
                }
                if b3_vis && !b3.priority {
                    break 'pick (b3.cgram_idx as usize, false, cgadsub & 0x08 != 0);
                }
                (0usize, true, cgadsub & 0x20 != 0) // backdrop
            };

            let sub_cgram = self.sub_pixels[x] as usize;
            self.composite_pixel(
                row_start,
                x,
                main_cgram,
                sub_cgram,
                main_is_backdrop,
                math_en,
            );
        }
    }

    /// Build sub-screen pixel array for mode 0 (used by color math).
    fn build_sub_pixels_mode0(
        &mut self,
        bg_pixels: &[[BgPixel; 256]; 4],
        spr_pixels: &[SpritePixel; 256],
    ) {
        let bg_palette_bases: [u8; 4] = [0, 32, 64, 96];
        for x in 0..256usize {
            let sp = spr_pixels[x];
            let b0 = bg_pixels[0][x];
            let b1 = bg_pixels[1][x];
            let b2 = bg_pixels[2][x];
            let b3 = bg_pixels[3][x];

            let sp_vis = self.sub_visible(sp.is_transparent(), 4, x);
            let b0_vis = (self.ts & 0x01 != 0) && self.sub_visible(b0.is_transparent(), 0, x);
            let b1_vis = (self.ts & 0x02 != 0) && self.sub_visible(b1.is_transparent(), 1, x);
            let b2_vis = (self.ts & 0x04 != 0) && self.sub_visible(b2.is_transparent(), 2, x);
            let b3_vis = (self.ts & 0x08 != 0) && self.sub_visible(b3.is_transparent(), 3, x);

            let _ = bg_palette_bases;
            self.sub_pixels[x] = 'pick: {
                if sp_vis && sp.priority == 3 {
                    break 'pick sp.cgram_idx;
                }
                if b0_vis && b0.priority {
                    break 'pick b0.cgram_idx;
                }
                if b1_vis && b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if sp_vis && sp.priority == 2 {
                    break 'pick sp.cgram_idx;
                }
                if b0_vis && !b0.priority {
                    break 'pick b0.cgram_idx;
                }
                if b1_vis && !b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if sp_vis && sp.priority == 1 {
                    break 'pick sp.cgram_idx;
                }
                if b2_vis && b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if b3_vis && b3.priority {
                    break 'pick b3.cgram_idx;
                }
                if sp_vis && sp.priority == 0 {
                    break 'pick sp.cgram_idx;
                }
                if b2_vis && !b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if b3_vis && !b3.priority {
                    break 'pick b3.cgram_idx;
                }
                0u8 // backdrop
            };
        }
    }

    /// Render one mode-1 scanline.
    ///
    /// Mode 1: BG1 4bpp, BG2 4bpp, BG3 2bpp.
    /// Standard priority order:
    ///   OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3hi, OBJ0, BG3lo, backdrop
    /// With BG3 priority bit set:
    ///   BG3hi, OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3lo, OBJ0, backdrop
    fn render_mode1(&mut self, row_start: usize, line: u16) {
        let mut bg1 = [BgPixel::TRANSPARENT; 256];
        let mut bg2 = [BgPixel::TRANSPARENT; 256];
        let mut bg3 = [BgPixel::TRANSPARENT; 256];
        let mut spr_pixels = [SpritePixel::TRANSPARENT; 256];

        if self.tm & 0x01 != 0 {
            let ts = if self.bg_tile_size[0] { 16 } else { 8 };
            render_bg_line(
                self.vram,
                &mut bg1,
                self.bg_sc[0],
                self.bg_scroll[0],
                self.bg_tile_data_base(0) as usize,
                4,
                0,
                ts,
                self.mosaic_line(line - 1, 0),
            );
        }
        if self.tm & 0x02 != 0 {
            let ts = if self.bg_tile_size[1] { 16 } else { 8 };
            render_bg_line(
                self.vram,
                &mut bg2,
                self.bg_sc[1],
                self.bg_scroll[1],
                self.bg_tile_data_base(1) as usize,
                4,
                0,
                ts,
                self.mosaic_line(line - 1, 1),
            );
        }
        if self.tm & 0x04 != 0 {
            let ts = if self.bg_tile_size[2] { 16 } else { 8 };
            render_bg_line(
                self.vram,
                &mut bg3,
                self.bg_sc[2],
                self.bg_scroll[2],
                self.bg_tile_data_base(2) as usize,
                2,
                0,
                ts,
                self.mosaic_line(line - 1, 2),
            );
        }
        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        // Build sub-screen pixel buffer for color math.
        self.build_sub_pixels_mode1(&bg1, &bg2, &bg3, &spr_pixels);

        let bg3_prio = self.bg3_priority;
        let cgadsub = self.cgadsub;

        for x in 0..256usize {
            let sp = spr_pixels[x];
            let bx1 = bg1[self.mosaic_x(x as u16, 0) as usize];
            let bx2 = bg2[self.mosaic_x(x as u16, 1) as usize];
            let bx3 = bg3[self.mosaic_x(x as u16, 2) as usize];

            let b1_vis = self.main_visible(bx1.is_transparent(), 0, x);
            let b2_vis = self.main_visible(bx2.is_transparent(), 1, x);
            let b3_vis = self.main_visible(bx3.is_transparent(), 2, x);
            let sp_vis = self.main_visible(sp.is_transparent(), 4, x);

            let (main_cgram, main_is_backdrop, math_en) = 'pick: {
                if bg3_prio && b3_vis && bx3.priority {
                    break 'pick (bx3.cgram_idx as usize, false, cgadsub & 0x04 != 0);
                }
                if sp_vis && sp.priority == 3 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b1_vis && bx1.priority {
                    break 'pick (bx1.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b2_vis && bx2.priority {
                    break 'pick (bx2.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 2 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b1_vis && !bx1.priority {
                    break 'pick (bx1.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b2_vis && !bx2.priority {
                    break 'pick (bx2.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 1 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if !bg3_prio && b3_vis && bx3.priority {
                    break 'pick (bx3.cgram_idx as usize, false, cgadsub & 0x04 != 0);
                }
                if sp_vis && sp.priority == 0 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b3_vis && !bx3.priority {
                    break 'pick (bx3.cgram_idx as usize, false, cgadsub & 0x04 != 0);
                }
                (0usize, true, cgadsub & 0x20 != 0) // backdrop
            };

            let sub_cgram = self.sub_pixels[x] as usize;
            self.composite_pixel(
                row_start,
                x,
                main_cgram,
                sub_cgram,
                main_is_backdrop,
                math_en,
            );
        }
    }

    /// Render one mode-3 scanline.
    ///
    /// Mode 3: BG1 8bpp (256-color direct palette), BG2 4bpp.
    /// Priority order (high → low):
    ///   OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, OBJ0, backdrop.
    fn render_mode3(&mut self, row_start: usize, line: u16) {
        let mut bg1 = [BgPixel::TRANSPARENT; 256];
        let mut bg2 = [BgPixel::TRANSPARENT; 256];
        let mut spr_pixels = [SpritePixel::TRANSPARENT; 256];

        if self.tm & 0x01 != 0 {
            let ts = if self.bg_tile_size[0] { 16 } else { 8 };
            self.render_bg_line_8bpp(&mut bg1, 0, ts, self.mosaic_line(line - 1, 0));
        }
        if self.tm & 0x02 != 0 {
            let ts = if self.bg_tile_size[1] { 16 } else { 8 };
            render_bg_line(
                self.vram,
                &mut bg2,
                self.bg_sc[1],
                self.bg_scroll[1],
                self.bg_tile_data_base(1) as usize,
                4,
                0,
                ts,
                self.mosaic_line(line - 1, 1),
            );
        }
        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        // Sub-screen: fill with backdrop (0) for now — no TS layer in M2 mode3.
        for v in self.sub_pixels.iter_mut() {
            *v = 0;
        }

        let cgadsub = self.cgadsub;

        for x in 0..256usize {
            let sp = spr_pixels[x];
            let b1 = bg1[self.mosaic_x(x as u16, 0) as usize];
            let b2 = bg2[self.mosaic_x(x as u16, 1) as usize];

            let b1_vis = self.main_visible(b1.is_transparent(), 0, x);
            let b2_vis = self.main_visible(b2.is_transparent(), 1, x);
            let sp_vis = self.main_visible(sp.is_transparent(), 4, x);

            let (main_cgram, main_is_backdrop, math_en) = 'pick: {
                if sp_vis && sp.priority == 3 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b1_vis && b1.priority {
                    break 'pick (b1.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b2_vis && b2.priority {
                    break 'pick (b2.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 2 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b1_vis && !b1.priority {
                    break 'pick (b1.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if b2_vis && !b2.priority {
                    break 'pick (b2.cgram_idx as usize, false, cgadsub & 0x02 != 0);
                }
                if sp_vis && sp.priority == 1 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if sp_vis && sp.priority == 0 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                (0usize, true, cgadsub & 0x20 != 0)
            };

            let sub_cgram = self.sub_pixels[x] as usize;
            self.composite_pixel(
                row_start,
                x,
                main_cgram,
                sub_cgram,
                main_is_backdrop,
                math_en,
            );
        }
    }

    /// Render one BG1 line in 8bpp mode (used by mode 3).
    ///
    /// 8bpp tile format: 64 bytes per 8×8 tile, packed as 8 bit-planes ×
    /// 8 rows. Each row is 8 bytes of consecutive VRAM; byte at
    /// `tile_base + row * 8 + col` directly encodes the 8-bit CGRAM index.
    ///
    /// VRAM word layout: sequential byte-pairs (lo, hi). Our `vram[]` is a
    /// byte array addressed directly (not word-addressed), so:
    ///   byte_addr = tile_no * 64 + row * 8 + col
    /// But VRAM data for BG1 in mode 3 uses the standard bitplane encoding:
    ///   4 pairs of bitplanes, each pair occupying 16 bytes per row-group.
    ///   Bitplanes 0,1: bytes 0-15 of each 64-byte tile.
    ///   Bitplanes 2,3: bytes 16-31.
    ///   Bitplanes 4,5: bytes 32-47.
    ///   Bitplanes 6,7: bytes 48-63.
    ///
    ///   For row r (0-7):
    ///     bp01_lo = vram[tile_base + r * 2]        (bitplane 0, row r)
    ///     bp01_hi = vram[tile_base + r * 2 + 1]    (bitplane 1, row r)
    ///     bp23_lo = vram[tile_base + 16 + r * 2]   (bitplane 2)
    ///     bp23_hi = vram[tile_base + 16 + r * 2 + 1]
    ///     bp45_lo = vram[tile_base + 32 + r * 2]   (bitplane 4)
    ///     bp45_hi = vram[tile_base + 32 + r * 2 + 1]
    ///     bp67_lo = vram[tile_base + 48 + r * 2]   (bitplane 6)
    ///     bp67_hi = vram[tile_base + 48 + r * 2 + 1]
    ///
    ///   Pixel color at column c (0-7, bit 7=leftmost):
    ///     bit0 = (bp01_lo >> (7-c)) & 1
    ///     bit1 = (bp01_hi >> (7-c)) & 1
    ///     bit2 = (bp23_lo >> (7-c)) & 1
    ///     bit3 = (bp23_hi >> (7-c)) & 1
    ///     bit4 = (bp45_lo >> (7-c)) & 1
    ///     bit5 = (bp45_hi >> (7-c)) & 1
    ///     bit6 = (bp67_lo >> (7-c)) & 1
    ///     bit7 = (bp67_hi >> (7-c)) & 1
    ///     color_idx = bit0 | (bit1<<1) | ... | (bit7<<7)
    fn render_bg_line_8bpp(
        &mut self,
        out: &mut [BgPixel; 256],
        bg: usize,
        tile_size: u16,
        line: u16,
    ) {
        let sc = self.bg_sc[bg];
        let scroll = self.bg_scroll[bg];
        let data_base = self.bg_tile_data_base(bg) as usize;
        // 8bpp 8×8 tile: 8 bitplanes × 8 rows × 2 bytes = 128 bytes total.
        // Two bitplanes per group of 16 bytes; 4 groups = 64 bytes for one tile
        // when stored as bitplane pairs. Each group is 2 bytes * 8 rows = 16 bytes.
        // Total per tile: 4 groups × 16 bytes = 64 bytes. Wait — 8bpp needs 8 bitplanes.
        // 8 planes / 2 per group = 4 groups, each 16 bytes, total 64 bytes per tile. Correct.
        let bytes_per_tile: usize = 64;

        let screen_y = ((line as u32).wrapping_add(scroll.vofs as u32)) & 0x1FF;

        for out_x in 0..256u32 {
            let screen_x = ((out_x).wrapping_add(scroll.hofs as u32)) & 0x1FF;

            let tile_col = screen_x / tile_size as u32;
            let tile_row = screen_y / tile_size as u32;
            let raw_tx = (screen_x % tile_size as u32) as u16;
            let raw_ty = (screen_y % tile_size as u32) as u16;

            // Fetch tilemap entry (same format as other modes).
            let tm_base = sc.base as usize;
            let h_wide = sc.h_wide;
            let v_wide = sc.v_wide;
            let h_screen = if h_wide { (tile_col >> 5) & 1 } else { 0 } as usize;
            let v_screen = if v_wide { (tile_row >> 5) & 1 } else { 0 } as usize;
            let screen_off: usize = match (h_screen, v_screen) {
                (0, 0) => 0x000,
                (1, 0) => 0x800,
                (0, 1) => {
                    if h_wide {
                        0x1000
                    } else {
                        0x800
                    }
                }
                _ => 0x1800,
            };
            let local_tx = (tile_col & 31) as usize;
            let local_ty = (tile_row & 31) as usize;
            let entry_off = (local_ty * 32 + local_tx) * 2;
            let addr = (tm_base + screen_off + entry_off) & 0xFFFF;
            let lo = self.vram[addr] as u16;
            let hi = self.vram[(addr + 1) & 0xFFFF] as u16;
            let entry = lo | (hi << 8);

            let char_no = (entry & 0x3FF) as usize;
            let priority = (entry >> 13) & 1 != 0;
            let hflip = (entry >> 14) & 1 != 0;
            let vflip = (entry >> 15) & 1 != 0;

            // Compute pixel within the (possibly 16×16) tile, handling flip.
            let (final_char, px, py) = if tile_size == 16 {
                let stx = if hflip { 1 - raw_tx / 8 } else { raw_tx / 8 };
                let sty = if vflip { 1 - raw_ty / 8 } else { raw_ty / 8 };
                let fpx = if hflip { 7 - (raw_tx & 7) } else { raw_tx & 7 };
                let fpy = if vflip { 7 - (raw_ty & 7) } else { raw_ty & 7 };
                (
                    (char_no + stx as usize + sty as usize * 16) & 0x3FF,
                    fpx,
                    fpy,
                )
            } else {
                let fpx = if hflip { 7 - raw_tx } else { raw_tx };
                let fpy = if vflip { 7 - raw_ty } else { raw_ty };
                (char_no, fpx, fpy)
            };

            let tile_base = (data_base + final_char * bytes_per_tile) & 0xFFFF;
            // Read 4 pairs of bitplanes for row `py`.
            let r = py as usize;
            let c = px as usize;
            let shift = 7 - c;

            let bp01_lo = self.vram[(tile_base + r * 2) & 0xFFFF];
            let bp01_hi = self.vram[(tile_base + r * 2 + 1) & 0xFFFF];
            let bp23_lo = self.vram[(tile_base + 16 + r * 2) & 0xFFFF];
            let bp23_hi = self.vram[(tile_base + 16 + r * 2 + 1) & 0xFFFF];
            let bp45_lo = self.vram[(tile_base + 32 + r * 2) & 0xFFFF];
            let bp45_hi = self.vram[(tile_base + 32 + r * 2 + 1) & 0xFFFF];
            let bp67_lo = self.vram[(tile_base + 48 + r * 2) & 0xFFFF];
            let bp67_hi = self.vram[(tile_base + 48 + r * 2 + 1) & 0xFFFF];

            let color_idx: u8 = ((bp01_lo >> shift) & 1)
                | (((bp01_hi >> shift) & 1) << 1)
                | (((bp23_lo >> shift) & 1) << 2)
                | (((bp23_hi >> shift) & 1) << 3)
                | (((bp45_lo >> shift) & 1) << 4)
                | (((bp45_hi >> shift) & 1) << 5)
                | (((bp67_lo >> shift) & 1) << 6)
                | (((bp67_hi >> shift) & 1) << 7);

            // Index 0 = transparent.
            out[out_x as usize] = BgPixel {
                color_idx,
                cgram_idx: color_idx,
                priority,
            };
        }
    }

    /// Render one mode-7 scanline.
    ///
    /// Mode 7: single affine-transformed BG, 8bpp, 8.8 fixed-point matrix
    /// in i32 (deny gate enforces no floats).
    /// Registers: $211A M7SEL, $211B-$211E M7A/B/C/D, $211F/$2120 M7X/M7Y.
    /// Priority: OBJ3/2/1/0 interleaved with single BG1 layer (no priority bit
    /// in mode 7 — BG1 always below priority ≥ 2 sprites).
    /// EXTBG (SETINI bit 6): faults at write time; not rendered here.
    fn render_mode7(&mut self, row_start: usize, line: u16) {
        let mut spr_pixels = [SpritePixel::TRANSPARENT; 256];
        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        // M7SEL: bit 1 = vflip, bit 0 = hflip.
        // bit 7: fill-outside mode (0 = use transparent/color0, 1 = tile 0 repeats).
        let m7sel = self.m7sel;
        let flip_h = (m7sel & 0x01) != 0;
        let flip_v = (m7sel & 0x02) != 0;
        let fill_tile0 = (m7sel & 0x80) != 0;

        // Matrix components in 8.8 fixed point (i16 stored as signed, cast to i32).
        let a = self.m7a as i32; // 8.8 fp; multiply by screen delta, shift right 8
        let b = self.m7b as i32;
        let c = self.m7c as i32;
        let d = self.m7d as i32;

        // Center (M7X/M7Y) are 13-bit signed (bits[12:0]).
        // Sign-extend from bit 12: shift left 19 bits to reach bit 31, then arithmetic right 19.
        let cx = ((self.m7x as i32) << 19) >> 19;
        let cy = ((self.m7y as i32) << 19) >> 19;

        // Screen Y for this scanline (1-indexed → 0-indexed).
        let screen_y = if flip_v {
            255 - (line as i32 - 1)
        } else {
            line as i32 - 1
        };

        let cgadsub = self.cgadsub;
        let bg1_enabled = self.tm & 0x01 != 0;

        #[allow(clippy::needless_range_loop)] // x indexes several parallel line buffers
        for x in 0..256usize {
            let screen_x = if flip_h { 255 - x as i32 } else { x as i32 };

            let dx = screen_x - cx;
            let dy = screen_y - cy;

            // 8.8 fixed-point affine transform.  A, B, C, D are already in 8.8 fp
            // (i.e. value/256 gives the rational coefficient).  After multiplying by
            // the integer screen-space deltas the products are in 8.8 fp, so we
            // shift right by 8 to recover integer VRAM-space coordinates.
            let vram_x = ((a * dx + b * dy) >> 8) + cx;
            let vram_y = ((c * dx + d * dy) >> 8) + cy;

            // Valid range is 0..=1023 (10-bit).
            let in_range = (0..1024).contains(&vram_x) && (0..1024).contains(&vram_y);

            let color_idx = if in_range || fill_tile0 {
                let px = if in_range { vram_x as usize } else { 0 };
                let py = if in_range { vram_y as usize } else { 0 };

                // Mode-7 VRAM interleaved layout:
                //   Word N (byte-pair at 2N, 2N+1):
                //     Even byte (2N)   = tilemap entry (tile number) for tile (N/64 % 128, …)
                //     Odd byte  (2N+1) = pixel data byte for some tile's pixel
                //
                // Tilemap: 128×128 tile grid.  Entry at (col, row) is the even byte of word
                //   word_addr = row * 128 + col  → byte_addr = (row*128 + col) * 2
                //
                // Tile pixel data: tile T has 8×8 pixels.  Pixel at (px_in_tile, py_in_tile)
                // is the odd byte of word:
                //   word_addr = T * 64 + py_in_tile * 8 + px_in_tile
                //   → byte_addr = word_addr * 2 + 1

                let tile_col = px / 8;
                let tile_row = py / 8;
                let tile_entry_addr = (tile_row * 128 + tile_col) * 2;
                let tile_no = self.vram[tile_entry_addr & 0xFFFF] as usize;

                let px_in_tile = px & 7;
                let py_in_tile = py & 7;
                let pixel_word = tile_no * 64 + py_in_tile * 8 + px_in_tile;
                let pixel_byte_addr = (pixel_word * 2 + 1) & 0xFFFF;
                self.vram[pixel_byte_addr]
            } else {
                0 // out-of-range and not fill_tile0 → color 0 (transparent/backdrop)
            };

            // Mode-7 BG1: no priority bit (always "low" priority in hardware terms).
            // Priority relative to sprites:
            //   Sprites prio 3/2 always win over BG1; prio 1/0 lose to BG1.
            let bg_pix = BgPixel {
                color_idx,
                cgram_idx: color_idx,
                priority: false,
            };

            let sp = spr_pixels[x];
            let sp_vis = self.main_visible(sp.is_transparent(), 4, x);
            let b1_vis = bg1_enabled && self.main_visible(bg_pix.is_transparent(), 0, x);

            let (main_cgram, main_is_backdrop, math_en) = 'pick: {
                if sp_vis && sp.priority >= 2 {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                if b1_vis {
                    break 'pick (bg_pix.cgram_idx as usize, false, cgadsub & 0x01 != 0);
                }
                if sp_vis {
                    break 'pick (sp.cgram_idx as usize, false, cgadsub & 0x10 != 0);
                }
                (0usize, true, cgadsub & 0x20 != 0)
            };

            let sub_cgram = 0usize; // mode-7 has no sub-screen in this impl
            self.composite_pixel(
                row_start,
                x,
                main_cgram,
                sub_cgram,
                main_is_backdrop,
                math_en,
            );
        }
    }

    /// Build sub-screen pixel array for mode 1.
    fn build_sub_pixels_mode1(
        &mut self,
        bg1: &[BgPixel; 256],
        bg2: &[BgPixel; 256],
        bg3: &[BgPixel; 256],
        spr_pixels: &[SpritePixel; 256],
    ) {
        let bg3_prio = self.bg3_priority;
        for x in 0..256usize {
            let sp = spr_pixels[x];
            let b1 = bg1[x];
            let b2 = bg2[x];
            let b3 = bg3[x];

            let sp_vis = self.sub_visible(sp.is_transparent(), 4, x);
            let b1_vis = (self.ts & 0x01 != 0) && self.sub_visible(b1.is_transparent(), 0, x);
            let b2_vis = (self.ts & 0x02 != 0) && self.sub_visible(b2.is_transparent(), 1, x);
            let b3_vis = (self.ts & 0x04 != 0) && self.sub_visible(b3.is_transparent(), 2, x);

            self.sub_pixels[x] = 'pick: {
                if bg3_prio && b3_vis && b3.priority {
                    break 'pick b3.cgram_idx;
                }
                if sp_vis && sp.priority == 3 {
                    break 'pick sp.cgram_idx;
                }
                if b1_vis && b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if b2_vis && b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if sp_vis && sp.priority == 2 {
                    break 'pick sp.cgram_idx;
                }
                if b1_vis && !b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if b2_vis && !b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if sp_vis && sp.priority == 1 {
                    break 'pick sp.cgram_idx;
                }
                if !bg3_prio && b3_vis && b3.priority {
                    break 'pick b3.cgram_idx;
                }
                if sp_vis && sp.priority == 0 {
                    break 'pick sp.cgram_idx;
                }
                if b3_vis && !b3.priority {
                    break 'pick b3.cgram_idx;
                }
                0u8
            };
        }
    }

    /// Get tile data base address in bytes for BG `bg` (0-indexed).
    #[inline]
    fn bg_tile_data_base(&self, bg: usize) -> u16 {
        match bg {
            0 => self.bg_nba.bg1_base,
            1 => self.bg_nba.bg2_base,
            2 => self.bg_nba.bg3_base,
            _ => self.bg_nba.bg4_base,
        }
    }

    // ────────────────────────────────────────────────────────────
    // V-blank and frame hooks
    // ────────────────────────────────────────────────────────────

    /// V-blank entry hook (start of line 225): internal OAM address reload
    /// per documented behavior (reload from OAMADD base unless force-blanked).
    pub fn begin_vblank(&mut self) {
        if !self.force_blank {
            self.oam_addr = self.oam_base_addr << 1;
        }
        self.cur_line = 225;
    }

    /// Frame start hook (line 0): clear v-blank-internal latches, reset
    /// mosaic start-line to line 1 (documented: mosaic block restarts at
    /// the top of each frame).
    pub fn begin_frame(&mut self) {
        self.cur_line = 0;
        self.mosaic_start_line = 1;
        // Reset OPHCT/OPVCT read toggles at frame start.
        self.ophct_read_high = false;
        self.opvct_read_high = false;
    }

    /// Update the current scanline number (used by $2137 SLHV latch).
    pub fn set_line(&mut self, line: u16) {
        self.cur_line = line;
    }

    /// Latch H/V counters when SLHV ($2137) is read.
    ///
    /// `dot` is the current H dot position (0-339) derived from the bus's
    /// `mclk_frame`: dot = (mclk_frame % MCLK_PER_LINE) / 4.
    /// This provides real dot-position tracking for OPHCT.
    pub fn latch_hv_counters(&mut self, dot: u16) {
        self.ophct = dot;
        self.opvct = self.cur_line;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod ppu_tests {
    use super::*;

    // ── Helper ───────────────────────────────────────────────────────────────

    fn make_ppu() -> Ppu {
        let vram: &'static mut [u8; 0x10000] = Box::leak(Box::new([0u8; 0x10000]));
        Ppu::new(vram)
    }

    // ── VMAIN / VMDATA / VMADD ───────────────────────────────────────────────

    /// VMAIN step = 1 (default), inc-on-high = false, remap = 0.
    /// Writing $2118 (VMDATA L) should advance VMADD by 1.
    #[test]
    fn ppu_vmain_step1_inc_on_low() {
        let mut p = make_ppu();
        // VMAIN: inc-on-high=0, step=1, remap=0
        assert!(p.write(0x15, 0x00).is_none());
        // Set VMADD to 0x0000
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        // Write low byte
        assert!(p.write(0x18, 0xAB).is_none());
        // VMADD should have advanced to 1
        assert!(p.write(0x17, 0x00).is_none()); // reset high (harmless)
        assert!(p.write(0x16, 0x00).is_none()); // reset VMADD to 0

        // Verify by reading back: load prefetch at addr 0, read L
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        let lo = p.read(0x39, 0xFF);
        assert_eq!(lo, 0xAB, "VRAM[0] low byte should be 0xAB");
    }

    /// VMAIN inc-on-high = true: VMADD advances after $2119 (VMDATA H) write.
    #[test]
    fn ppu_vmain_inc_on_high() {
        let mut p = make_ppu();
        // VMAIN: inc-on-high=1, step=1, remap=0
        assert!(p.write(0x15, 0x80).is_none());
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        // Write both bytes at address 0
        assert!(p.write(0x18, 0x11).is_none()); // low — should NOT increment
        assert!(p.write(0x19, 0x22).is_none()); // high — should increment
                                                // Now VMADD should be 1
                                                // Write at address 1
        assert!(p.write(0x18, 0x33).is_none());
        assert!(p.write(0x19, 0x44).is_none());
        // Read back address 0
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        let lo = p.read(0x39, 0);
        let hi = p.read(0x3A, 0);
        assert_eq!(lo, 0x11);
        assert_eq!(hi, 0x22);
        // Read address 1 (prefetch was reloaded by reading lo above with inc-on-high=1,
        // so we need to reload manually by writing VMADD again)
        assert!(p.write(0x16, 0x01).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        let lo2 = p.read(0x39, 0);
        let hi2 = p.read(0x3A, 0);
        assert_eq!(lo2, 0x33);
        assert_eq!(hi2, 0x44);
    }

    /// VMAIN step=32.
    #[test]
    fn ppu_vmain_step32() {
        let mut p = make_ppu();
        // VMAIN: inc-on-high=0, step=32, remap=0
        assert!(p.write(0x15, 0x01).is_none());
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        // Write low byte at addr 0
        assert!(p.write(0x18, 0xBB).is_none());
        // VMADD should be 32
        assert!(p.write(0x18, 0xCC).is_none());
        // VMADD should be 64

        // Read addr 0
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        assert_eq!(p.read(0x39, 0), 0xBB);

        // Read addr 32
        assert!(p.write(0x16, 32u8).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        assert_eq!(p.read(0x39, 0), 0xCC);
    }

    /// VMAIN step=128.
    #[test]
    fn ppu_vmain_step128() {
        let mut p = make_ppu();
        // bits [1:0] = 0b10 = 2 → step = 128
        assert!(p.write(0x15, 0x02).is_none());
        // Write 0xDD at word-address 0
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        assert!(p.write(0x18, 0xDD).is_none()); // writes addr 0, then VMADD → 128
                                                // Now write 0xEE at word-address 128
        assert!(p.write(0x18, 0xEE).is_none()); // writes addr 128, then VMADD → 256

        // Read back addr 0
        assert!(p.write(0x16, 0x00).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        assert_eq!(p.read(0x39, 0), 0xDD, "word 0 low byte should be 0xDD");

        // Read back addr 128
        assert!(p.write(0x16, 128u8).is_none());
        assert!(p.write(0x17, 0x00).is_none());
        assert_eq!(p.read(0x39, 0), 0xEE, "word 128 low byte should be 0xEE");
    }

    /// VMAIN remap mode 1: 2bpp rotation.
    /// The documented remap swaps bits: addr_out = (addr & 0xFF00) | ((addr & 0x1F) << 3) | ((addr & 0xE0) >> 5)
    #[test]
    fn ppu_vmain_remap_mode1() {
        let mut p = make_ppu();
        // VMAIN remap=1, inc-on-low, step=1
        assert!(p.write(0x15, 0x04).is_none()); // bits [3:2] = 01
                                                // Write to logical address 0x0020 (32 decimal)
        let addr: u16 = 0x0020;
        assert!(p.write(0x16, (addr & 0xFF) as u8).is_none());
        assert!(p.write(0x17, (addr >> 8) as u8).is_none());
        assert!(p.write(0x18, 0x55).is_none());

        // Physical address that addr=0x0020 maps to with remap=1:
        // remap(0x0020) = (0x0020 & 0xFF00) | ((0x0020 & 0x1F) << 3) | ((0x0020 & 0xE0) >> 5)
        //               = 0x0000 | (0x00 << 3) | (0x20 >> 5)
        //               = 0 | 0 | 1 = 0x0001
        let phys: usize = 2;
        assert_eq!(
            p.vram[phys], 0x55,
            "remap mode 1: byte at physical address should be 0x55"
        );
    }

    /// VMAIN remap mode 2: 4bpp rotation.
    #[test]
    fn ppu_vmain_remap_mode2() {
        let mut p = make_ppu();
        // VMAIN remap=2, inc-on-low, step=1
        assert!(p.write(0x15, 0x08).is_none()); // bits [3:2] = 10
        let addr: u16 = 0x0040;
        assert!(p.write(0x16, (addr & 0xFF) as u8).is_none());
        assert!(p.write(0x17, (addr >> 8) as u8).is_none());
        assert!(p.write(0x18, 0x77).is_none());
        // remap(0x0040) = (0x0040 & 0xFE00) | ((0x0040 & 0x3F) << 3) | ((0x0040 & 0x01C0) >> 6)
        //               = 0x0000 | (0x00 << 3) | (0x40 >> 6)
        //               = 0 | 0 | 1 = 0x0001
        let phys: usize = 2;
        assert_eq!(p.vram[phys], 0x77);
    }

    /// VMAIN remap mode 3: 8bpp rotation.
    #[test]
    fn ppu_vmain_remap_mode3() {
        let mut p = make_ppu();
        assert!(p.write(0x15, 0x0C).is_none()); // bits [3:2] = 11
        let addr: u16 = 0x0080;
        assert!(p.write(0x16, (addr & 0xFF) as u8).is_none());
        assert!(p.write(0x17, (addr >> 8) as u8).is_none());
        assert!(p.write(0x18, 0x88).is_none());
        // remap(0x0080) = (0x0080 & 0xFC00) | ((0x0080 & 0x7F) << 3) | ((0x0080 & 0x0380) >> 7)
        //               = 0x0000 | (0x00 << 3) | (0x80 >> 7)
        //               = 0 | 0 | 1 = 0x0001
        let phys: usize = 2;
        assert_eq!(p.vram[phys], 0x88);
    }

    /// VRAM prefetch: after writing VMADD the prefetch buffer is loaded and
    /// RDVRAM returns it without advancing the address.
    #[test]
    fn ppu_vram_prefetch_read() {
        let mut p = make_ppu();
        assert!(p.write(0x15, 0x80).is_none()); // inc-on-high
                                                // Put a known word at VRAM[5]
        assert!(p.write(0x16, 5).is_none());
        assert!(p.write(0x17, 0).is_none());
        assert!(p.write(0x18, 0xAA).is_none()); // lo
        assert!(p.write(0x19, 0xBB).is_none()); // hi → increments to addr 6

        // Set VMADD back to 5: this loads the prefetch
        assert!(p.write(0x16, 5).is_none());
        assert!(p.write(0x17, 0).is_none());
        // Read lo — should return 0xAA (from prefetch); does NOT increment since inc-on-high
        let lo = p.read(0x39, 0xFF);
        // Read hi — returns 0xBB, then increments
        let hi = p.read(0x3A, 0xFF);
        assert_eq!(lo, 0xAA, "prefetch low should be 0xAA");
        assert_eq!(hi, 0xBB, "prefetch high should be 0xBB");
    }

    // ── CGRAM write/read latches ─────────────────────────────────────────────

    /// CGRAM write-twice latch: write CGADD then two bytes, read back.
    #[test]
    fn ppu_cgram_write_read_latch() {
        let mut p = make_ppu();
        // Write color 1 (index 1 = CGRAM bytes 2,3) = BGR555 0x1234
        assert!(p.write(0x21, 1).is_none()); // CGADD = 1
        assert!(p.write(0x22, 0x34).is_none()); // low byte (first write)
        assert!(p.write(0x22, 0x12).is_none()); // high byte (second write, commits, addr→2)

        // Read back via RDCGRAM
        // First reset CGADD
        assert!(p.write(0x21, 1).is_none());
        let lo = p.read(0x3B, 0xFF); // first read = low byte
        let hi = p.read(0x3B, 0xFF); // second read = high 7 bits | mdr[7]
        assert_eq!(lo, 0x34);
        assert_eq!(hi & 0x7F, 0x12 & 0x7F);
    }

    /// CGRAM: a single first write should not commit.
    #[test]
    fn ppu_cgram_single_write_no_commit() {
        let mut p = make_ppu();
        // Pre-fill CGRAM[0] with known values
        p.cgram[0] = 0xAA;
        p.cgram[1] = 0x55;
        // Write CGADD=0 and do only one write
        assert!(p.write(0x21, 0).is_none());
        assert!(p.write(0x22, 0xFF).is_none()); // first write — latched, not committed
                                                // CGRAM[0] should still be 0xAA
        assert_eq!(p.cgram[0], 0xAA, "single write should not commit low byte");
        assert_eq!(p.cgram[1], 0x55, "single write should not touch high byte");
    }

    // ── OAM write latch + high table ─────────────────────────────────────────

    /// OAM write-pair latch: two writes to the low table commit both bytes.
    #[test]
    fn ppu_oam_write_pair_latch() {
        let mut p = make_ppu();
        // OAMADD = sprite 0 (byte address 0)
        assert!(p.write(0x02, 0x00).is_none());
        assert!(p.write(0x03, 0x00).is_none());
        // Write pair: bytes 0 and 1 of sprite 0
        assert!(p.write(0x04, 0x10).is_none()); // X lo (byte 0) — latched
        assert!(p.write(0x04, 0x20).is_none()); // Y (byte 1) — commits
        assert_eq!(p.oam[0], 0x10, "OAM[0] should be 0x10 after commit");
        assert_eq!(p.oam[1], 0x20, "OAM[1] should be 0x20 after commit");
    }

    /// OAM: first write to even addr should not yet update OAM.
    #[test]
    fn ppu_oam_first_write_latch_not_committed() {
        let mut p = make_ppu();
        p.oam[0] = 0xAA;
        p.oam[1] = 0xBB;
        assert!(p.write(0x02, 0x00).is_none());
        assert!(p.write(0x03, 0x00).is_none());
        assert!(p.write(0x04, 0xFF).is_none()); // even addr → latched
        assert_eq!(p.oam[0], 0xAA, "first write should not commit");
    }

    /// OAM high table (bytes 512–543) written directly.
    #[test]
    fn ppu_oam_high_table_direct_write() {
        let mut p = make_ppu();
        // Address 256 in word-address terms = byte addr 512 in the low-table
        // space. We need to get oam_addr to 512.
        // OAMADD bit 8 set → start at word 256 = byte 512
        assert!(p.write(0x02, 0x00).is_none()); // low 8 bits = 0
        assert!(p.write(0x03, 0x01).is_none()); // bit 8 set → oam_base = 0x100 → addr = 0x200 = 512
                                                // Now write a byte to the high table (addr 512 = high table offset 0)
        assert!(p.write(0x04, 0xF0).is_none());
        assert_eq!(p.oam[512], 0xF0, "high table byte 0 should be 0xF0");
    }

    // ── Fault generation ─────────────────────────────────────────────────────

    #[test]
    fn ppu_fault_bg_mode2() {
        let mut p = make_ppu();
        let f = p.write(0x05, 0x02); // mode 2
        assert!(matches!(f, Some(Fault::UnimplementedBgMode { mode: 2 })));
    }

    /// Mode 7 is now accepted (M2 implements it); writing BGMODE=7 must NOT fault.
    #[test]
    fn ppu_no_fault_bg_mode7() {
        let mut p = make_ppu();
        let f = p.write(0x05, 0x07);
        assert!(f.is_none(), "mode 7 must be accepted without fault in M2");
    }

    /// Mode 3 is now accepted (M2 implements it); writing BGMODE=3 must NOT fault.
    #[test]
    fn ppu_no_fault_bg_mode3() {
        let mut p = make_ppu();
        let f = p.write(0x05, 0x03);
        assert!(f.is_none(), "mode 3 must be accepted without fault in M2");
    }

    /// Mosaic is now accepted (M2 implements it); writing $2106 must NOT fault.
    #[test]
    fn ppu_no_fault_mosaic() {
        let mut p = make_ppu();
        let f = p.write(0x06, 0x10); // nonzero enable bits
        assert!(f.is_none(), "mosaic must be accepted without fault in M2");
    }

    /// Window W12SEL register ($2123) is now accepted; must NOT fault.
    #[test]
    fn ppu_no_fault_window_w12sel() {
        let mut p = make_ppu();
        let f = p.write(0x23, 0x01);
        assert!(f.is_none(), "W12SEL must be accepted without fault in M2");
    }

    /// CGADSUB ($2131) accepted without fault in M2.
    #[test]
    fn ppu_no_fault_cgadsub() {
        let mut p = make_ppu();
        assert!(p.write(0x31, 0xC1).is_none(), "CGADSUB must not fault");
    }

    /// Mosaic: size register is stored correctly.
    #[test]
    fn ppu_mosaic_stores_size_and_enable() {
        let mut p = make_ppu();
        assert!(p.write(0x06, 0xF3).is_none()); // size=15, bg_enable=0b0011
        assert_eq!(p.mosaic_size, 15);
        assert_eq!(p.mosaic_bg_enable, 0b0011);
    }

    /// mosaic_line: with mosaic disabled for a BG, line passes through unchanged.
    #[test]
    fn ppu_mosaic_line_disabled_passthrough() {
        let mut p = make_ppu();
        // mosaic_bg_enable = 0 → all BGs passthrough
        assert!(p.write(0x06, 0x00).is_none());
        p.mosaic_start_line = 1;
        assert_eq!(p.mosaic_line(10, 0), 10);
    }

    /// mosaic_line: 4×4 block snaps to block start.
    #[test]
    fn ppu_mosaic_line_4x4_quantization() {
        let mut p = make_ppu();
        // mosaic size = 3 (block = 4 pixels), BG1 enabled (bit 0)
        assert!(p.write(0x06, 0x31).is_none()); // size=3, bg1 enabled
        p.mosaic_start_line = 1;
        // Lines 1,2,3,4 should all map to line 1; lines 5,6,7,8 → line 5.
        assert_eq!(p.mosaic_line(1, 0), 1);
        assert_eq!(p.mosaic_line(2, 0), 1);
        assert_eq!(p.mosaic_line(3, 0), 1);
        assert_eq!(p.mosaic_line(4, 0), 1);
        assert_eq!(p.mosaic_line(5, 0), 5);
        assert_eq!(p.mosaic_line(8, 0), 5);
    }

    #[test]
    fn ppu_fault_setini_nonzero() {
        let mut p = make_ppu();
        let f = p.write(0x33, 0x01);
        assert!(matches!(
            f,
            Some(Fault::UnimplementedPpuFeature { reg: 0x33, .. })
        ));
    }

    #[test]
    fn ppu_no_fault_setini_zero() {
        let mut p = make_ppu();
        assert!(p.write(0x33, 0x00).is_none());
    }

    // ── Force blank ──────────────────────────────────────────────────────────

    #[test]
    fn ppu_force_blank_renders_black() {
        let mut p = make_ppu();
        // Force blank is on by default; render line 1
        p.render_scanline(1);
        // All pixels should be black (XRGB8888 = 0x00000000)
        for i in 0..256 {
            let off = i * 4;
            assert_eq!(p.back[off], 0, "B must be 0 (force blank)");
            assert_eq!(p.back[off + 1], 0, "G must be 0 (force blank)");
            assert_eq!(p.back[off + 2], 0, "R must be 0 (force blank)");
        }
    }

    // ── Render smoke test 1: mode-0 BG1 single tile with palette ─────────────

    /// Set up one 2bpp tile in BG1, assign a simple palette, enable the
    /// layer, and verify that the expected pixel values appear after rendering.
    #[test]
    fn ppu_render_mode0_bg1_single_tile() {
        let mut p = make_ppu();

        // Turn off force blank at full brightness.
        assert!(p.write(0x00, 0x0F).is_none()); // INIDISP: force_blank=0, brightness=15

        // Select mode 0.
        assert!(p.write(0x05, 0x00).is_none()); // BGMODE: mode 0, 8×8 tiles

        // BG1 tilemap at VRAM 0x0000, no scroll extension.
        assert!(p.write(0x07, 0x00).is_none()); // BG1SC: base=0, 32×32

        // BG1 tile data at VRAM 0x0000 (tile 0 lives at byte 0).
        assert!(p.write(0x0B, 0x00).is_none()); // BG12NBA: bg1 nibble 0 → base 0

        // Zero scroll (BG1 at top-left).
        assert!(p.write(0x0D, 0x00).is_none()); // BG1HOFS write 1
        assert!(p.write(0x0D, 0x00).is_none()); // BG1HOFS write 2
        assert!(p.write(0x0E, 0x00).is_none()); // BG1VOFS write 1
        assert!(p.write(0x0E, 0x00).is_none()); // BG1VOFS write 2

        // Enable BG1 on main screen.
        assert!(p.write(0x2C, 0x01).is_none()); // TM: BG1 only

        // Set tilemap entry 0 (tile 0, palette 0, priority 0, no flip).
        // entry word = 0x0000 (all zeros → tile 0, palette 0)
        // VRAM: tilemap is at 0x0000; entries are 2 bytes each.
        p.vram[0] = 0x00; // tile lo
        p.vram[1] = 0x00; // tile hi + attr

        // Write 2bpp tile 0 data at VRAM byte offset 0 (same location as tilemap
        // but tile data base is also 0 — this is fine for a smoke test since we just
        // need consistent data).
        // 2bpp tile: 8 rows × 2 bytes = 16 bytes total, starting at byte 0.
        // But the tilemap entry is also at byte 0. Let's set tile data base above
        // the tilemap.  Tilemap = 0x0000, tile data = 0x2000 (base nibble = 1).
        //
        // Re-set: BG1 tile data base at 0x2000 bytes (nibble 1 = 1 << 13 = 0x2000).
        assert!(p.write(0x0B, 0x01).is_none()); // BG12NBA: bg1 nibble 1 → base 0x2000

        // 2bpp tile 0 at VRAM 0x2000: fill row 0 with color 3 (both bits set).
        // Row 0 plane 0: 0xFF (all pixels = bit 1 set), plane 1: 0xFF
        p.vram[0x2000] = 0xFF; // plane 0 row 0
        p.vram[0x2001] = 0xFF; // plane 1 row 0 → color idx = 3 for all 8 pixels

        // CGRAM: set mode-0 BG1 palette 0 colors.
        // Color 0 (index 0): transparent (not rendered)
        // Color 3 (index 3, palette 0): BGR555 = R=31 G=0 B=0 = 0x001F
        assert!(p.write(0x21, 3).is_none()); // CGADD = 3
        assert!(p.write(0x22, 0x1F).is_none()); // lo byte: R=31 (bits[4:0])
        assert!(p.write(0x22, 0x00).is_none()); // hi byte: G=0, B=0

        // Render scanline 1 (→ back-buffer row 0).
        p.render_scanline(1);

        // Pixel 0..8 on row 0 should all be (R=255, G=0, B=0) with brightness=15.
        // R5=31, brightness=15: R8 = (31<<3)|(31>>2) = 248|7 = 255; scaled = 255*(15+1)/16 = 255
        for x in 0..8usize {
            let off = x * 4;
            let b = p.back[off];
            let g = p.back[off + 1];
            let r = p.back[off + 2];
            assert_eq!(b, 0, "pixel {x}: blue should be 0");
            assert_eq!(g, 0, "pixel {x}: green should be 0");
            assert_eq!(r, 255, "pixel {x}: red should be 255");
        }
    }

    /// Scroll offset test: BG1 scrolled right by 8 pixels should show the
    /// second tile's pixels starting at x=0.
    #[test]
    fn ppu_render_mode0_bg1_hscroll() {
        let mut p = make_ppu();
        assert!(p.write(0x00, 0x0F).is_none()); // force_blank=0, brightness=15
        assert!(p.write(0x05, 0x00).is_none()); // mode 0
        assert!(p.write(0x07, 0x00).is_none()); // BG1SC base=0
        assert!(p.write(0x0B, 0x01).is_none()); // tile data at 0x2000

        // Scroll BG1 right by 8 → pixel column 0 shows tile column 1
        // First write: prev_latch = 8; hofs = (8 & 3) << 8 | 0 = 0
        // Second write: prev_latch = 0; hofs = (0 & 3) << 8 | 8 = 8
        assert!(p.write(0x0D, 8).is_none()); // set latch = 8
        assert!(p.write(0x0D, 0).is_none()); // hofs = (0<<8) | 8 = 8

        // Enable BG1.
        assert!(p.write(0x2C, 0x01).is_none());

        // Tile 0 (map entry 0x0000) at tile column 0: all transparent (tile data = 0).
        // Tile 1 (map entry 0x0001 at VRAM offset 2) at tile column 1: color 1 = green.
        p.vram[2] = 0x01; // tilemap entry for tile column 1: tile#=1
        p.vram[3] = 0x00;
        // Tile 1 data at VRAM 0x2010 (tile 0 = 16 bytes, tile 1 starts at +16):
        p.vram[0x2010] = 0xFF; // plane 0 row 0: all pixels bit0 set
        p.vram[0x2011] = 0x00; // plane 1 row 0: bit1 clear → color_idx = 1
                               // CGRAM palette 0, color 1 (idx 1) = green: BGR555 G=31 → 0x03E0
        assert!(p.write(0x21, 1).is_none());
        assert!(p.write(0x22, 0xE0).is_none()); // lo: bits[7:5]=G lo
        assert!(p.write(0x22, 0x03).is_none()); // hi: bits[1:0]=G hi → G5=31

        p.render_scanline(1);

        // With hofs=8, screen column 0 maps to tile column 1, tile pixel 0.
        let off = 0; // pixel 0, byte offset 0
        let b = p.back[off];
        let g = p.back[off + 1];
        let r = p.back[off + 2];
        assert_eq!(r, 0, "hscroll: red should be 0 for green pixel");
        assert_eq!(b, 0, "hscroll: blue should be 0 for green pixel");
        assert_eq!(g, 255, "hscroll: green should be 255");
    }

    // ── Render smoke test 2: sprite over BG with priority ────────────────────

    /// Set up a BG1 tile of color A, and a sprite covering the same area with
    /// color B at a higher priority.  Verify that sprite pixels appear on top
    /// of the BG, and that transparent sprite pixels reveal the BG.
    #[test]
    fn ppu_render_sprite_over_bg_priority() {
        let mut p = make_ppu();
        assert!(p.write(0x00, 0x0F).is_none()); // force_blank=0

        // Mode 1: BG1 4bpp.
        assert!(p.write(0x05, 0x01).is_none()); // BGMODE: mode 1
        assert!(p.write(0x07, 0x00).is_none()); // BG1SC at 0x0000
        assert!(p.write(0x0B, 0x01).is_none()); // BG1 tile data at 0x2000

        // Enable BG1 and sprites on main screen.
        assert!(p.write(0x2C, 0x11).is_none()); // TM: BG1 + OBJ

        // BG1 tile 0 (mode 1, 4bpp): all pixels = color_idx 1 (red: R=31).
        // Tile data at 0x2000. 4bpp: 32 bytes per tile.
        // Row 0, planes 0+1: plane0 row0 = 0xFF, plane1 row0 = 0x00
        //   → bits [1:0] = 01 → color_idx = 1
        p.vram[0x2000] = 0xFF; // plane 0 row 0
        p.vram[0x2001] = 0x00; // plane 1 row 0
                               // planes 2+3 at byte 16:
        p.vram[0x2010] = 0x00; // plane 2 row 0
        p.vram[0x2011] = 0x00; // plane 3 row 0 → color_idx final = 0b0001 = 1

        // CGRAM: BG1 4bpp palette 0 (mode 1 BG1 uses palette 0, colors 0-15 at CGRAM 0-15).
        // Color 1 = red: R5=31 G=0 B=0 → BGR555 = 0x001F
        assert!(p.write(0x21, 1).is_none());
        assert!(p.write(0x22, 0x1F).is_none()); // lo
        assert!(p.write(0x22, 0x00).is_none()); // hi

        // OBSEL: size pair 0 (8×8 small, 16×16 large), name_base=0, name_select=0.
        // Tile data will be placed in VRAM starting at OBJ name base.
        // OBJ name base field (bits[4:3] of $2101): 0 → byte addr 0.
        // But OBJ tile data shares the VRAM space with BG tiles. We use a
        // separate area: let OBSEL name_base=1 → byte addr = 1 << 14 = 0x4000.
        assert!(p.write(0x01, 0x08).is_none()); // OBSEL: size=0, name_base=1, name_select=0
                                                //   $2101 = 0b000_01_000 → [7:5]=0(size0), [4:3]=01(name_base=1), [2:1]=00(gap=0)

        // OBJ tile 0 at 0x4000 (4bpp, 32 bytes): pixel row 0 = color_idx 2 (blue: B=31).
        // plane 0 row 0 = 0x00, plane 1 row 0 = 0xFF, plane 2/3 = 0
        //   → bits = 0b0010 = 2
        p.vram[0x4000] = 0x00; // plane 0
        p.vram[0x4001] = 0xFF; // plane 1 → color_idx = 2
                               // planes 2+3 at byte 16
        p.vram[0x4010] = 0x00;
        p.vram[0x4011] = 0x00;
        // Row 0, pixels 1-7: make pixel 0 transparent (overwrite bit 7 of plane1 to 0).
        // Actually: 0xFF means all 8 bits set → all 8 pixels of this row have color_idx=2.
        // We want pixel 0 transparent. Let's make plane0 row0 = 0b0111_1111 = 0x7F
        // and plane1 row0 = 0b1000_0000 = 0x80:
        //   pixel 7 (MSB): bit7 of plane0=0, bit7 of plane1=1 → color_idx=2
        //   pixel 0 (LSB): bit0 of plane0=1, bit0 of plane1=0 → color_idx=1 (not 0)
        // Simpler: sprite pixel 3 transparent, all others color_idx=2.
        // plane0 = 0b1111_0111 = 0xF7 (bit 3 from right = 0), plane1 = 0b1111_0111 = 0xF7
        //   pixel 3 (from right, i.e. screen_x = sprite_x + 4): bit=(7-4)=3
        //   p0 bit3=1, p1 bit3=1 → color_idx=3. Hmm, let me just use:
        // plane0 = 0xFF, plane1 = 0x00 → color_idx = 1 (not transparent, not 2)
        // plane0 = 0x00, plane1 = 0xFF → color_idx = 2 ← use this
        // Already set above: plane0=0x00, plane1=0xFF → color_idx=2 for all pixels.
        // Pixel 3 (sprite column 3): screen_x = sprite.x + 3.
        // Let sprite.x = 0, sprite.y = 0 (renders on line 1 of screen = scanline 1).
        // To make pixel 3 transparent: we need color_idx=0 there.
        // bit position for screen_x=3: px in tile = 3, bit = 7-3 = 4.
        // plane0 bit4 = 0, plane1 bit4 = 0 → color_idx = 0 (transparent).
        p.vram[0x4000] = 0b1110_1111u8; // plane0: bit4=0 for pixel 3
        p.vram[0x4001] = 0b1110_1111u8; // plane1: bit4=0 for pixel 3
                                        // → pixels 0-2, 4-7: color_idx = 0b11 = 3 (both planes set)
                                        //   pixel 3: bit4 of plane0=0, bit4 of plane1=0 → color_idx = 0 (transparent)

        // CGRAM sprite palette 0 = colors 128-143 (CGRAM 128*2 = byte 256).
        // Color 3 in sprite palette 0 = CGRAM index 128 + 3 = 131.
        // Set it to blue: B5=31 G=0 R=0 → BGR555 = 0x7C00
        assert!(p.write(0x21, 131).is_none()); // CGADD = 131
        assert!(p.write(0x22, 0x00).is_none()); // lo: R=0, G lo=0
        assert!(p.write(0x22, 0x7C).is_none()); // hi: B=31 → 0x7C = 0b0111_1100

        // Place sprite 0 at X=0, Y=0, tile=0, priority=3, palette=0.
        // OAM bytes 0-3: X_lo=0, Y=0, tile=0, attr=0b11_0_000_00=0x30 (priority=3, palette=0)
        // attr byte format: vhpp ppp t (v=vflip, h=hflip, pp=priority, ppp=palette, t=tile hi)
        //   bits[7]=vflip, [6]=hflip, [5:4]=priority, [3:1]=palette, [0]=tile_hi
        p.oam[0] = 0; // X lo
        p.oam[1] = 0; // Y
        p.oam[2] = 0; // tile lo
        p.oam[3] = 0b0011_0000; // priority=3 (bits [5:4]=11), palette=0, no flip
                                // OAM high table byte 0 = 4 sprites. Sprite 0: bits[1:0] = size_bit(0)=small(8×8), x_sign=0
        p.oam[512] = 0b0000_0000; // sprite 0: size=0 (small=8x8), x_sign=0

        // Render scanline 1.
        p.render_scanline(1);

        // Pixel 0 (sprite pixel 0): sprite covers BG. Sprite color_idx = 3 (not transparent).
        // Priority 3 wins over everything. Should be blue.
        // Blue: B5=31 → B8=255, full brightness → B=255, G=0, R=0.
        {
            let off = 0; // pixel 0, byte offset 0
            let b = p.back[off];
            let g = p.back[off + 1];
            let r = p.back[off + 2];
            assert_eq!(r, 0, "pixel 0: sprite should be blue (R=0)");
            assert_eq!(g, 0, "pixel 0: sprite should be blue (G=0)");
            assert_eq!(b, 255, "pixel 0: sprite should be blue (B=255)");
        }

        // Pixel 3: sprite transparent → BG1 shows through (red).
        // BG1 color_idx = 1, CGRAM[1] = red (R5=31 → R8=255).
        {
            let off = 3 * 4;
            let b = p.back[off];
            let g = p.back[off + 1];
            let r = p.back[off + 2];
            assert_eq!(b, 0, "pixel 3: transparent sprite → BG shows (B=0)");
            assert_eq!(g, 0, "pixel 3: transparent sprite → BG shows (G=0)");
            assert_eq!(r, 255, "pixel 3: transparent sprite → BG shows (R=255)");
        }

        // Pixel 8 (outside 8×8 sprite): BG1 only.
        {
            let off = 8 * 4;
            let b = p.back[off];
            let g = p.back[off + 1];
            let r = p.back[off + 2];
            assert_eq!(b, 0, "pixel 8: no sprite, BG1 red");
            assert_eq!(g, 0, "pixel 8: no sprite, BG1 red");
            assert_eq!(r, 255, "pixel 8: no sprite, BG1 red");
        }
    }

    // ── M2: Window mask truth-table ──────────────────────────────────────────

    /// Window OR: result is 1 if either w1 or w2 is 1.
    #[test]
    fn window_combine_or() {
        assert_eq!(window_combine(0, 0, 0), 0);
        assert_eq!(window_combine(1, 0, 0), 1);
        assert_eq!(window_combine(0, 1, 0), 1);
        assert_eq!(window_combine(1, 1, 0), 1);
    }

    /// Window AND: result is 1 only when both are 1.
    #[test]
    fn window_combine_and() {
        assert_eq!(window_combine(0, 0, 1), 0);
        assert_eq!(window_combine(1, 0, 1), 0);
        assert_eq!(window_combine(0, 1, 1), 0);
        assert_eq!(window_combine(1, 1, 1), 1);
    }

    /// Window XOR: result is 1 when exactly one is 1.
    #[test]
    fn window_combine_xor() {
        assert_eq!(window_combine(0, 0, 2), 0);
        assert_eq!(window_combine(1, 0, 2), 1);
        assert_eq!(window_combine(0, 1, 2), 1);
        assert_eq!(window_combine(1, 1, 2), 0);
    }

    /// Window XNOR: result is 1 when both are the same.
    #[test]
    fn window_combine_xnor() {
        assert_eq!(window_combine(0, 0, 3), 1);
        assert_eq!(window_combine(1, 0, 3), 0);
        assert_eq!(window_combine(0, 1, 3), 0);
        assert_eq!(window_combine(1, 1, 3), 1);
    }

    /// window_range_mask: normal range (left ≤ right).
    #[test]
    fn window_range_mask_normal() {
        let mut mask = [0u8; 256];
        window_range_mask(10, 20, &mut mask);
        for (x, &m) in mask.iter().enumerate() {
            let expected = if (10..=20).contains(&x) { 1 } else { 0 };
            assert_eq!(m, expected, "pixel {x}");
        }
    }

    /// window_range_mask: wrapped range (left > right → outside the gap is active).
    #[test]
    fn window_range_mask_wrapped() {
        let mut mask = [0u8; 256];
        window_range_mask(250, 5, &mut mask);
        for (x, &m) in mask.iter().enumerate() {
            let expected = if x >= 250 || x <= 5 { 1 } else { 0 };
            assert_eq!(m, expected, "pixel {x}");
        }
    }

    // ── M2: Color math add/subtract/half ─────────────────────────────────────

    /// color_math_op add: 5-bit clamp at 31.
    #[test]
    fn color_math_add_clamps() {
        // R=20 + sub_R=20 = 40 → clamped to 31.
        let result = color_math_op(20, 0, 0, 20, 0, 0, true, false);
        assert_eq!(result & 0x1F, 31, "R should clamp to 31");
    }

    /// color_math_op subtract: clamp at 0.
    #[test]
    fn color_math_sub_clamps() {
        // R=5 - sub_R=10 = -5 → clamped to 0.
        let result = color_math_op(5, 0, 0, 10, 0, 0, false, false);
        assert_eq!(result & 0x1F, 0, "R should clamp to 0");
    }

    /// color_math_op half: add then divide by 2.
    #[test]
    fn color_math_add_half() {
        // R=10 + sub_R=10 = 20; half → 10.
        let result = color_math_op(10, 0, 0, 10, 0, 0, true, true);
        assert_eq!(result & 0x1F, 10, "R add+half should be 10");
        // Halving happens BEFORE the 5-bit clamp: 31 + 31 = 62, half → 31
        // (a clamp-then-halve implementation would wrongly produce 15).
        let result = color_math_op(31, 0, 0, 31, 0, 0, true, true);
        assert_eq!(result & 0x1F, 31, "half-math must not be capped at 15");
        // Subtract+half floors at 0: 10 - 31 = -21, half → -10, clamp → 0.
        let result = color_math_op(10, 0, 0, 31, 0, 0, false, true);
        assert_eq!(result & 0x1F, 0, "sub+half floors at 0");
    }

    /// COLDATA component write: each plane independently settable.
    #[test]
    fn ppu_coldata_component_write() {
        let mut p = make_ppu();
        // Write R=31 (plane R = bit5 set, value = 31)
        assert!(p.write(0x32, (1 << 5) | 31).is_none()); // R plane, value=31
        assert_eq!(p.coldata_color & 0x001F, 31, "R component");
        // Write G=15 (plane G = bit6 set, value = 15)
        assert!(p.write(0x32, (2 << 5) | 15).is_none()); // G plane, value=15
        assert_eq!((p.coldata_color >> 5) & 0x1F, 15, "G component");
        // Write B=7 (plane B = bit7 set, value = 7)
        assert!(p.write(0x32, (4 << 5) | 7).is_none()); // B plane, value=7
        assert_eq!((p.coldata_color >> 10) & 0x1F, 7, "B component");
        // R and G should be unchanged
        assert_eq!(p.coldata_color & 0x001F, 31, "R after B write");
        assert_eq!((p.coldata_color >> 5) & 0x1F, 15, "G after B write");
    }

    // ── M2: OPHCT real dot latch ──────────────────────────────────────────────

    /// latch_hv_counters stores dot in ophct and cur_line in opvct.
    #[test]
    fn ppu_latch_hv_counters_stores_dot() {
        let mut p = make_ppu();
        p.set_line(42);
        p.latch_hv_counters(137);
        // Read OPHCT (two-read protocol): first read = low byte, second = high bit.
        let lo = p.read(0x3C, 0);
        let hi = p.read(0x3C, 0);
        let ophct = lo as u16 | (((hi & 1) as u16) << 8);
        assert_eq!(ophct, 137, "OPHCT should be 137");
        // Read OPVCT
        let vlo = p.read(0x3D, 0);
        let vhi = p.read(0x3D, 0);
        let opvct = vlo as u16 | (((vhi & 1) as u16) << 8);
        assert_eq!(opvct, 42, "OPVCT should be 42");
    }

    /// SLHV read ($2137) sets counter_latched; STAT78 ($213F) returns latch bit then clears.
    #[test]
    fn ppu_slhv_sets_counter_latched() {
        let mut p = make_ppu();
        p.set_line(10);
        p.latch_hv_counters(50); // simulates bus pre-call
        let _ = p.read(0x37, 0); // SLHV read: sets counter_latched
                                 // STAT78 should return bit 6 set
        let stat78 = p.read(0x3F, 0);
        assert_eq!(
            stat78 & 0x40,
            0x40,
            "counter_latched bit should be set after $2137 read"
        );
        // Second STAT78 read: bit clears
        let stat78b = p.read(0x3F, 0);
        assert_eq!(
            stat78b & 0x40,
            0,
            "counter_latched should clear after $213F read"
        );
    }
}
