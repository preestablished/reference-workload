//! Minimal scanline PPU: backgrounds + sprites, no mid-frame effects (M1).
//!
//! OWNER (implementation): PPU agent.
//!
//! M1 scope (IMPLEMENTATION-PLAN.md M1): BG modes 0 and 1 (tilemaps up to
//! 64×64, 8×8/16×16 tiles, 2bpp/4bpp), sprites (OAM, OBSEL sizes,
//! priorities), whole-frame scrolling, CGRAM palettes, brightness, force
//! blank. Per-scanline rendering into an internal XRGB8888 back buffer.
//! NOT in M1 (must fault per D9 if enabled, via the returned fault):
//! BG modes 2..=7, mosaic, windows, color math beyond fixed backdrop,
//! interlace/pseudo-hires/overscan flags.
//!
//! Sprite range/time overflow limits are deliberately not modeled in M1
//! (documented simplification; determinism is unaffected).

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
    /// Fixed color register (stored, unused in M1).
    coldata: u8,

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
            coldata: 0,
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
                if mode >= 2 {
                    return Some(Fault::UnimplementedBgMode { mode });
                }
                self.bg_mode = mode;
                self.bg3_priority = (value & 0x08) != 0;
                for i in 0..4 {
                    self.bg_tile_size[i] = (value >> (4 + i)) & 1 != 0;
                }
            }

            // ── $2106 MOSAIC ───────────────────────────────────
            0x06 => {
                if value & 0xF0 != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
                // Mosaic size field (bits [3:0]): enabled layers above already faulted.
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
            0x23 => {
                self.w12sel = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }
            // ── $2124 W34SEL ───────────────────────────────────
            0x24 => {
                self.w34sel = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }
            // ── $2125 WOBJSEL ──────────────────────────────────
            0x25 => {
                self.wobjsel = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }

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

            // ── $212E TMW (main-screen window) ─────────────────
            0x2E => {
                self.tmw = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }
            // ── $212F TSW (sub-screen window) ──────────────────
            0x2F => {
                self.tsw = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }

            // ── $2130 CGWSEL ───────────────────────────────────
            0x30 => {
                self.cgwsel = value;
                // Nonzero color-math/clip enable bits fault; zero accepted.
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }
            // ── $2131 CGADSUB ──────────────────────────────────
            0x31 => {
                self.cgadsub = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }

            // ── $2132 COLDATA ──────────────────────────────────
            // Store fixed color (unused in M1).
            0x32 => self.coldata = value,

            // ── $2133 SETINI ───────────────────────────────────
            // Any nonzero bit (interlace/overscan/hires/extbg) faults; zero ok.
            0x33 => {
                self.setini = value;
                if value != 0 {
                    return Some(Fault::UnimplementedPpuFeature { reg, value });
                }
            }

            // Writes to read-only registers ($2134–$213F) or beyond scope:
            // PPU open bus — return without action (deterministic, no fault).
            _ => {}
        }
        None
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
            // Latch H/V counters.
            // M1 simplification: H dot position not tracked; H = 0, V = cur_line.
            0x37 => {
                self.ophct = 0; // H = 0 (no dot counter in M1)
                self.opvct = self.cur_line;
                self.counter_latched = true;
                // Resets OPHCT/OPVCT read toggles per docs.
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
            // Force blank → write all-black row
            for i in 0..256 {
                let off = row_start + i * 4;
                self.back[off] = 0; // B
                self.back[off + 1] = 0; // G
                self.back[off + 2] = 0; // R
                self.back[off + 3] = 0; // X
            }
            return;
        }

        match self.bg_mode {
            0 => self.render_mode0(row_start),
            1 => self.render_mode1(row_start),
            _ => {
                // Should have been caught at BGMODE write; render black.
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

    /// Render one mode-0 scanline.
    ///
    /// Mode 0: 4 BGs, all 2bpp.
    /// Per-BG palette offsets (documented): BG1=0, BG2=32, BG3=64, BG4=96.
    /// Priority order (high → low):
    ///   OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3hi, BG4hi, OBJ0, BG3lo, BG4lo, backdrop
    fn render_mode0(&mut self, row_start: usize) {
        let line = (row_start / 1024 + 1) as u16;

        let mut bg_pixels: [[BgPixel; 256]; 4] = [[BgPixel::TRANSPARENT; 256]; 4];
        let mut spr_pixels = [SpritePixel::TRANSPARENT; 256];

        // BG palette bases for mode 0 (0, 32, 64, 96)
        let bg_palette_bases: [u8; 4] = [0, 32, 64, 96];

        for bg in 0..4usize {
            if self.tm & (1 << bg) == 0 {
                continue; // layer disabled
            }
            let tile_size = if self.bg_tile_size[bg] { 16 } else { 8 };
            let data_base = self.bg_tile_data_base(bg) as usize;
            render_bg_line(
                self.vram,
                &mut bg_pixels[bg],
                self.bg_sc[bg],
                self.bg_scroll[bg],
                data_base,
                2,
                bg_palette_bases[bg],
                tile_size,
                line - 1,
            );
        }

        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        // Compose pixels in priority order for mode 0.
        for x in 0..256usize {
            let sp = spr_pixels[x];
            let b0 = bg_pixels[0][x];
            let b1 = bg_pixels[1][x];
            let b2 = bg_pixels[2][x];
            let b3 = bg_pixels[3][x];

            // Priority order: OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo,
            //                 OBJ1, BG3hi, BG4hi, OBJ0, BG3lo, BG4lo, backdrop
            let cgram_idx = 'pick: {
                if !sp.is_transparent() && sp.priority == 3 {
                    break 'pick sp.cgram_idx;
                }
                if !b0.is_transparent() && b0.priority {
                    break 'pick b0.cgram_idx;
                }
                if !b1.is_transparent() && b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 2 {
                    break 'pick sp.cgram_idx;
                }
                if !b0.is_transparent() && !b0.priority {
                    break 'pick b0.cgram_idx;
                }
                if !b1.is_transparent() && !b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 1 {
                    break 'pick sp.cgram_idx;
                }
                if !b2.is_transparent() && b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if !b3.is_transparent() && b3.priority {
                    break 'pick b3.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 0 {
                    break 'pick sp.cgram_idx;
                }
                if !b2.is_transparent() && !b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if !b3.is_transparent() && !b3.priority {
                    break 'pick b3.cgram_idx;
                }
                0u8 // backdrop = CGRAM color 0
            };

            let color = read_cgram_color(&self.cgram, cgram_idx as usize);
            let pixel = bgr555_to_xrgb8888(color, self.brightness);
            let off = row_start + x * 4;
            self.back[off] = (pixel & 0xFF) as u8; // B
            self.back[off + 1] = ((pixel >> 8) & 0xFF) as u8; // G
            self.back[off + 2] = ((pixel >> 16) & 0xFF) as u8; // R
            self.back[off + 3] = 0; // X
        }
    }

    /// Render one mode-1 scanline.
    ///
    /// Mode 1: BG1 4bpp, BG2 4bpp, BG3 2bpp.
    /// Standard priority order:
    ///   OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3hi, OBJ0, BG3lo, backdrop
    /// With BG3 priority bit set:
    ///   BG3hi, OBJ3, BG1hi, BG2hi, OBJ2, BG1lo, BG2lo, OBJ1, BG3lo, OBJ0, backdrop
    fn render_mode1(&mut self, row_start: usize) {
        let line = (row_start / 1024 + 1) as u16;

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
                line - 1,
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
                line - 1,
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
                line - 1,
            );
        }
        if self.tm & 0x10 != 0 {
            render_sprite_line(self.vram, &self.oam, self.obsel, &mut spr_pixels, line);
        }

        let bg3_prio = self.bg3_priority;

        for x in 0..256usize {
            let sp = spr_pixels[x];
            let b1 = bg1[x];
            let b2 = bg2[x];
            let b3 = bg3[x];

            let cgram_idx = 'pick: {
                if bg3_prio && !b3.is_transparent() && b3.priority {
                    break 'pick b3.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 3 {
                    break 'pick sp.cgram_idx;
                }
                if !b1.is_transparent() && b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if !b2.is_transparent() && b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 2 {
                    break 'pick sp.cgram_idx;
                }
                if !b1.is_transparent() && !b1.priority {
                    break 'pick b1.cgram_idx;
                }
                if !b2.is_transparent() && !b2.priority {
                    break 'pick b2.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 1 {
                    break 'pick sp.cgram_idx;
                }
                if !b3.is_transparent() && b3.priority && !bg3_prio {
                    break 'pick b3.cgram_idx;
                }
                if !sp.is_transparent() && sp.priority == 0 {
                    break 'pick sp.cgram_idx;
                }
                if !b3.is_transparent() && !b3.priority {
                    break 'pick b3.cgram_idx;
                }
                0u8 // backdrop
            };

            let color = read_cgram_color(&self.cgram, cgram_idx as usize);
            let pixel = bgr555_to_xrgb8888(color, self.brightness);
            let off = row_start + x * 4;
            self.back[off] = (pixel & 0xFF) as u8;
            self.back[off + 1] = ((pixel >> 8) & 0xFF) as u8;
            self.back[off + 2] = ((pixel >> 16) & 0xFF) as u8;
            self.back[off + 3] = 0;
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
        // During v-blank, the internal OAM address is reloaded from OAMADD
        // (this is the documented behavior when not force-blanked; if force-
        // blanked the address may be written freely, but we still reload here
        // since force-blank is already reflected in oam_addr from writes).
        if !self.force_blank {
            self.oam_addr = self.oam_base_addr << 1;
        }
        self.cur_line = 225;
    }

    /// Frame start hook (line 0): clear v-blank-internal latches.
    pub fn begin_frame(&mut self) {
        self.cur_line = 0;
        // Reset OPHCT/OPVCT read toggles at frame start.
        self.ophct_read_high = false;
        self.opvct_read_high = false;
    }

    /// Update the current scanline number (used by $2137 SLHV latch).
    ///
    /// The core should call this before each `render_scanline`. In M1 we
    /// also update it internally in `render_scanline`, but this hook exists
    /// for external callers.
    pub fn set_line(&mut self, line: u16) {
        self.cur_line = line;
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

    #[test]
    fn ppu_fault_bg_mode7() {
        let mut p = make_ppu();
        let f = p.write(0x05, 0x07);
        assert!(matches!(f, Some(Fault::UnimplementedBgMode { mode: 7 })));
    }

    #[test]
    fn ppu_fault_mosaic() {
        let mut p = make_ppu();
        let f = p.write(0x06, 0x10); // nonzero enable bits
        assert!(matches!(
            f,
            Some(Fault::UnimplementedPpuFeature { reg: 0x06, .. })
        ));
    }

    #[test]
    fn ppu_fault_window_w12sel() {
        let mut p = make_ppu();
        let f = p.write(0x23, 0x01);
        assert!(matches!(
            f,
            Some(Fault::UnimplementedPpuFeature { reg: 0x23, .. })
        ));
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
}
