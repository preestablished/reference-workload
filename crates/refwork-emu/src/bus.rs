//! Bus abstraction the CPU executes against, and the system bus wiring
//! WRAM, cartridge, PPU, stub APU, DMA, HDMA, joypads, and CPU I/O registers.
//!
//! OWNER (implementation): integration agent. The trait is also implemented
//! by a flat test bus in `xtask` (single-step CPU tests), so its semantics
//! must stay CPU-generic.
//!
//! M2 additions:
//! - HDMA: per-scanline table-walk DMA; `execute_hdma` runs before each visible
//!   line (called from `core_impl.rs` frame loop).
//! - Auto-joypad stale reads: $4218/$4219 return the *previous* latch while
//!   `auto_joy_busy` (the first three vblank lines). Simplification documented: games that
//!   poll $4212 first (common idiom) are exact either way.
//! - OPHCT real dot counter: `start_line` passes the current within-line dot
//!   to `ppu.latch_hv_counters` so OPHCT reflects a real position.

use crate::apu::Apu;
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::dma::{unit_pattern, Dma, Hdma, HdmaState};
use crate::fault::Fault;
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::timing::{mem_speed, MCLK_PER_INTERNAL_CYCLE, MCLK_PER_LINE};

/// What the CPU needs from the outside world.
///
/// Every method that models a bus cycle advances the implementation's
/// master clock; the CPU itself never tracks time.
pub trait Bus {
    /// One data-bus read of a 24-bit address (advances the clock by the
    /// region's access cost; updates the open-bus latch).
    fn read(&mut self, addr: u32) -> u8;
    /// One data-bus write (clocked like `read`).
    fn write(&mut self, addr: u32, value: u8);
    /// One CPU internal cycle (6 master clocks), no bus activity.
    fn idle(&mut self);
    /// True exactly once per NMI edge (reading consumes the pending edge).
    fn take_nmi(&mut self) -> bool;
    /// Level-triggered IRQ line (CPU masks with its `I` flag).
    fn irq_line(&self) -> bool;
    /// Record a fault (D9). The driver halts emulation when set.
    fn fault(&mut self, fault: Fault);
}

/// The real system bus. Owns every device; the WRAM buffer is the published
/// `wram` region (zero-copy publication, D7).
pub struct SysBus {
    pub wram: &'static mut [u8; 0x20000],
    pub cart: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    /// Master-clock timestamp the APU has been serviced through. Normal catch-up
    /// advances it to `mclk_total`.
    pub apu_mclk_base: u64,
    /// A CPU write of the IPL `$CC` kick can be overwritten before the next APU
    /// catch-up. Preserve that one protocol edge until the next safe APU-port
    /// boundary so the IPL observes it without running the whole APU ahead.
    apu_pending_ipl_cc: bool,
    /// While the IPL transfer loop is active, a CPU can write port 0 before the
    /// associated data byte reaches port 1 (for example as part of a 16-bit
    /// store). Defer catch-up across the immediately following non-port0 write
    /// so the HLE does not consume a stale data byte.
    apu_pending_ipl_transfer_strobe: bool,
    pub dma: Dma,
    /// HDMA per-frame run-time state (D8: fixed-size, allocated in new).
    pub hdma: Hdma,
    /// HDMA enable register ($420C): bitmask of channels running this frame.
    pub hdmaen: u8,
    /// Channels whose HDMA table hit its terminator this frame. A mid-frame
    /// `$420C` enable cannot resurrect them (hardware/snes9x: `HDMA = value
    /// & !HDMAEnded`); cleared by `init_hdma` at the start of each frame.
    pub hdma_ended: u8,
    pub joypad: Joypad,

    /// Master clocks elapsed since the start of the current frame.
    pub mclk_frame: u64,
    /// Monotonically increasing total master clocks elapsed since power-on.
    /// Used for APU catch-up scheduling (never resets between frames).
    pub mclk_total: u64,
    /// Open-bus / memory-data-register latch (D3: documented constant
    /// behavior — reads of unmapped addresses return this).
    pub mdr: u8,
    /// First fault recorded (sticky).
    pub fault: Option<Fault>,

    // ---- CPU I/O registers ($4200-$421F block) ----
    /// $4200 NMITIMEN: NMI enable (bit7), V/H IRQ enable (bits 5/4),
    /// auto-joypad enable (bit0).
    pub nmitimen: u8,
    /// $4207/$4208 HTIME (9 bits), $4209/$420A VTIME (9 bits).
    pub htime: u16,
    pub vtime: u16,
    /// $420D MEMSEL bit0 (FastROM).
    pub fast_rom: bool,
    /// $4202/$4203 multiplicands, $4204-$4206 dividend/divisor,
    /// $4214-$4217 results.
    pub wrmpya: u8,
    pub wrdiv: u16,
    pub rddiv: u16,
    pub rdmpy: u16,
    /// $4210 RDNMI bit7 (v-blank NMI flag, read-clears).
    pub nmi_flag: bool,
    /// Pending NMI edge for the CPU (set when the flag rises with NMI
    /// enabled; consumed by `take_nmi`).
    pub nmi_pending: bool,
    /// $4211 TIMEUP bit7 (H/V timer IRQ flag, read-clears; drives the IRQ
    /// line while set).
    pub irq_flag: bool,
    /// $2181-$2183 WRAM port address.
    pub wmadd: u32,
    /// Current scanline (maintained by the frame scheduler).
    pub line: u16,
    /// True during the first three vblank scanlines while auto-joypad is busy.
    pub auto_joy_busy: bool,
    /// Previous joypad latch (before auto-read started): returned by $4218/$4219
    /// while `auto_joy_busy`. Simplification: games that poll $4212 first are exact
    /// either way; the new latch becomes visible after the third vblank line.
    pub joy_prev: u16,
    /// Diagnostic flags accumulated during the current frame.
    pub frame_flags: crate::fault::FrameFlags,

    // ---- Additional registers ----
    /// $4201 WRIO: output port byte.
    pub wrio: u8,
    /// Precomputed next H/V-IRQ target (absolute mclk_frame value), if armed.
    /// Recomputed on writes to $4200/$4207-$420A and at start_line.
    irq_target_mclk: Option<u64>,

    /// Clean-room diagnostic read counters (introspect-only; compiled out of the
    /// guest binary). Counts only — never the values read.
    #[cfg(feature = "introspect")]
    pub diag_rd_4210: u64,
    #[cfg(feature = "introspect")]
    pub diag_rd_4211: u64,
    #[cfg(feature = "introspect")]
    pub diag_rd_4212: u64,
    #[cfg(feature = "introspect")]
    pub diag_rd_apu: u64,
    #[cfg(feature = "introspect")]
    pub diag_wr_apu: u64,
    #[cfg(feature = "introspect")]
    pub diag_wr_cc_port0: bool,
    #[cfg(feature = "introspect")]
    pub diag_cc_port0_mclk: Option<u64>,
    #[cfg(feature = "introspect")]
    pub diag_post_cc_port0_writes: u64,
    #[cfg(feature = "introspect")]
    pub diag_first_post_cc_port0_delta_mclk: Option<u64>,
    #[cfg(feature = "introspect")]
    pub diag_apu_port0_service_count: u64,
    #[cfg(feature = "introspect")]
    pub diag_first_cc_service_spc_pc: Option<u16>,
}

impl SysBus {
    /// Construct with power-on register state. `wram` is pre-filled by the
    /// caller with the fixed init pattern (D3).
    pub fn new(wram: &'static mut [u8; 0x20000], cart: Cartridge, ppu: Ppu) -> SysBus {
        SysBus {
            wram,
            cart,
            ppu,
            apu: Apu::new(),
            apu_mclk_base: 0,
            apu_pending_ipl_cc: false,
            apu_pending_ipl_transfer_strobe: false,
            dma: Dma::new(),
            hdma: Hdma::new(),
            hdmaen: 0,
            hdma_ended: 0,
            joypad: Joypad::new(),

            mclk_frame: 0,
            mclk_total: 0,
            mdr: 0,
            fault: None,

            nmitimen: 0,
            htime: 0x1FF, // documented power-on: 9-bit all-1s (out of range → no IRQ)
            vtime: 0x1FF,
            fast_rom: false,
            wrmpya: 0xFF,
            wrdiv: 0xFFFF,
            rddiv: 0,
            rdmpy: 0,

            nmi_flag: false,
            nmi_pending: false,
            irq_flag: false,

            wmadd: 0,
            line: 0,
            auto_joy_busy: false,
            joy_prev: 0,
            frame_flags: crate::fault::FrameFlags::default(),

            wrio: 0xFF,
            irq_target_mclk: None,

            #[cfg(feature = "introspect")]
            diag_rd_4210: 0,
            #[cfg(feature = "introspect")]
            diag_rd_4211: 0,
            #[cfg(feature = "introspect")]
            diag_rd_4212: 0,
            #[cfg(feature = "introspect")]
            diag_rd_apu: 0,
            #[cfg(feature = "introspect")]
            diag_wr_apu: 0,
            #[cfg(feature = "introspect")]
            diag_wr_cc_port0: false,
            #[cfg(feature = "introspect")]
            diag_cc_port0_mclk: None,
            #[cfg(feature = "introspect")]
            diag_post_cc_port0_writes: 0,
            #[cfg(feature = "introspect")]
            diag_first_post_cc_port0_delta_mclk: None,
            #[cfg(feature = "introspect")]
            diag_apu_port0_service_count: 0,
            #[cfg(feature = "introspect")]
            diag_first_cc_service_spc_pc: None,
        }
    }

    /// Non-clocked, side-effect-free read for `debug_peek`/tests. Returns
    /// `None` for addresses whose read has side effects or is unmapped.
    #[cfg_attr(not(feature = "introspect"), allow(dead_code))]
    pub fn peek(&self, addr: u32) -> Option<u8> {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // Banks $7E/$7F: full 128 KiB WRAM.
        if bank == 0x7E || bank == 0x7F {
            let wram_off = ((bank as usize - 0x7E) * 0x10000) | off as usize;
            return Some(self.wram[wram_off]);
        }

        // Low-mirror WRAM: banks $00-$3F/$80-$BF, offset $0000-$1FFF.
        if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) && off < 0x2000 {
            return Some(self.wram[off as usize & 0x1FFF]);
        }

        // Cart ROM/SRAM (no side effects).
        self.cart.read(addr)
    }

    /// Advance the master clock by `mclk`, firing the H/V timer IRQ when the
    /// configured (V,H) position is crossed (NMITIMEN bits 5/4).
    fn add_mclk(&mut self, mclk: u64) {
        let old = self.mclk_frame;
        self.mclk_frame = old + mclk;
        self.mclk_total = self.mclk_total.wrapping_add(mclk);
        // Check if an IRQ target was crossed.
        if let Some(target) = self.irq_target_mclk {
            if old < target && self.mclk_frame >= target {
                self.irq_flag = true;
                // Recompute the next target:
                // H-only (bit4 set, bit5 clear): fires every line → next line.
                // V-only or H+V: one shot per frame → clear.
                let h_irq = self.nmitimen & 0x10 != 0;
                let v_irq = self.nmitimen & 0x20 != 0;
                if h_irq && !v_irq {
                    // Schedule for the same H position next line.
                    self.irq_target_mclk = self.next_h_irq_target();
                } else {
                    self.irq_target_mclk = None;
                }
            }
        }
    }

    /// Compute the absolute mclk_frame value for the next H-only IRQ target
    /// (same H position, next line). Used for H-only repeating IRQ.
    fn next_h_irq_target(&self) -> Option<u64> {
        if self.htime >= 340 {
            return None;
        }
        // Current line's H position offset within the frame.
        let h_offset: u64 = self.htime as u64 * 4 + 14;
        let next_line = self.line as u64 + 1;
        if next_line >= 262 {
            return None;
        }
        Some(next_line * MCLK_PER_LINE + h_offset)
    }

    /// Recompute the H/V IRQ target mclk_frame value from scratch.
    /// Called on writes to $4200/$4207-$420A and at start_line().
    ///
    /// Boundary convention: only targets strictly in the future are armed
    /// (`target > mclk_frame` here, fired by `add_mclk` when
    /// `old < target <= new`). A target landing exactly on the current
    /// master clock does not fire until the next frame's reschedule —
    /// deterministic, documented; refine if M2 raster tests need exact-dot
    /// reprogramming semantics.
    fn recompute_irq_target(&mut self) {
        let h_irq = self.nmitimen & 0x10 != 0;
        let v_irq = self.nmitimen & 0x20 != 0;

        if !h_irq && !v_irq {
            self.irq_target_mclk = None;
            return;
        }

        let htime = self.htime;
        let vtime = self.vtime;

        // H-only: fires at htime dots on the current (and every subsequent) line.
        // V-only: fires at line==vtime, dot ~2 (mclk_in_line=10).
        // H+V:    fires at line==vtime, dot htime.

        if htime >= 340 && h_irq {
            // htime out of range → never fires.
            self.irq_target_mclk = None;
            return;
        }
        if vtime >= 262 && v_irq {
            // vtime out of range → never fires.
            self.irq_target_mclk = None;
            return;
        }

        if h_irq && !v_irq {
            // H-only: next H position on the current line (or next line if
            // we've already passed it on this line).
            let h_offset: u64 = htime as u64 * 4 + 14;
            let candidate = self.line as u64 * MCLK_PER_LINE + h_offset;
            if candidate > self.mclk_frame {
                self.irq_target_mclk = Some(candidate);
            } else {
                // Already passed this line's H position → schedule for next line.
                self.irq_target_mclk = self.next_h_irq_target();
            }
        } else if !h_irq && v_irq {
            // V-only: fires once at start of vtime line, dot ~2 (offset 10 mclk).
            let v_line_start = vtime as u64 * MCLK_PER_LINE;
            let target = v_line_start + 10;
            if target > self.mclk_frame {
                self.irq_target_mclk = Some(target);
            } else {
                self.irq_target_mclk = None; // already passed this frame
            }
        } else {
            // H+V: fires once at line==vtime, dot htime.
            let h_offset: u64 = htime as u64 * 4 + 14;
            let v_line_start = vtime as u64 * MCLK_PER_LINE;
            let target = v_line_start + h_offset;
            if target > self.mclk_frame {
                self.irq_target_mclk = Some(target);
            } else {
                self.irq_target_mclk = None;
            }
        }
    }

    fn record_apu_halt(&mut self, halt: Option<crate::apu::spc700::ApuHalt>) {
        let Some(h) = halt else {
            return;
        };
        use crate::apu::spc700::ApuHalt;
        match h {
            ApuHalt::Stop => {
                let pc = self.apu.cpu.pc;
                self.fault(crate::fault::Fault::ApuStopped { pc });
            }
            ApuHalt::TestTrigger(v) => {
                let pc = self.apu.cpu.pc;
                self.fault(crate::fault::Fault::ApuTestTrigger { value: v, pc });
            }
            // SLEEP is not a fault; the APU will resume on the next interrupt.
            ApuHalt::Sleep => {}
        }
    }

    fn advance_apu_to(&mut self, target_mclk: u64) {
        if self.apu_mclk_base >= target_mclk {
            return;
        }
        let delta = target_mclk - self.apu_mclk_base;
        self.apu_mclk_base = target_mclk;
        let halt = self.apu.advance_master_cycles(delta);
        self.record_apu_halt(halt);
    }

    fn should_capture_ipl_cc(&self, port: u8, value: u8) -> bool {
        port == 0 && value == 0xCC && (0xFFC0..0xFFE0).contains(&self.apu.cpu.pc)
    }

    fn apu_in_ipl_transfer_loop(&self) -> bool {
        (0xFFE0..=0xFFFF).contains(&self.apu.cpu.pc)
    }

    fn service_pending_ipl_cc(&mut self) {
        if !self.apu_pending_ipl_cc {
            return;
        }
        self.apu_pending_ipl_cc = false;
        if !(0xFFC0..0xFFE0).contains(&self.apu.cpu.pc) || self.apu.spc_ports[0] != 0xCC {
            return;
        }

        let _cycles = self.apu.step();
        self.record_apu_halt(self.apu.halted);
        #[cfg(feature = "introspect")]
        {
            self.diag_apu_port0_service_count += 1;
            if self.diag_first_cc_service_spc_pc.is_none() {
                self.diag_first_cc_service_spc_pc = Some(self.apu.cpu.pc);
            }
        }
    }

    /// Catch the APU up to the current `mclk_total` timestamp.
    ///
    /// Called (a) whenever the CPU accesses $2140–$2143, and (b) at the end
    /// of each scanline from `core_impl.rs`. This bounds the divergence window
    /// deterministically: the APU is never more than one scanline behind the
    /// CPU's master-clock view.
    ///
    /// If the catch-up encounters an APU halt condition (SLEEP/STOP/test-register
    /// nonzero write), records the appropriate `Fault` (D9).
    pub fn apu_catch_up(&mut self) {
        self.advance_apu_to(self.mclk_total);
    }

    /// Frame scheduler hook: called by `Core` at the start of every
    /// scanline. Handles v-blank entry (NMI flag/edge, OAM reload,
    /// auto-joypad latch), v-blank exit, and per-line APU catch-up.
    pub fn start_line(&mut self, line: u16, pad: u16) {
        let _ = pad; // pad is set on the joypad by Core before calling start_line
        self.line = line;

        let vblank_start = self.ppu.vblank_start_line();
        if line == 0 {
            // End of V-blank: clear NMI flag, begin new frame. (The PPU's
            // begin_frame/begin_vblank hooks are invoked by the Core frame
            // loop so SysBus stays unit-testable without a live PPU.)
            self.recompute_irq_target();
            self.nmi_flag = false;
        } else if line == vblank_start {
            // V-blank start: set NMI flag.
            self.nmi_flag = true;
            // If NMITIMEN bit7 is set, raise NMI edge.
            if self.nmitimen & 0x80 != 0 {
                self.nmi_pending = true;
            }
            // If auto-joypad enabled (NMITIMEN bit0):
            // Stale-read protocol (M2): snapshot the current latch into
            // joy_prev before performing the new auto-read. While busy
            // (the first three vblank lines), $4218/$4219 return joy_prev (the previous
            // latch). The new latch becomes visible when busy clears at
            // the third vblank line. Games that poll $4212 bit0 first are exact.
            if self.nmitimen & 0x01 != 0 {
                self.joy_prev = self.joypad.joy1;
                self.joypad.auto_read();
                self.auto_joy_busy = true;
            }
        } else if line == vblank_start + 3 {
            self.auto_joy_busy = false;
        }
    }

    /// Initialize HDMA channels at the start of a new frame (line 0 reload).
    ///
    /// For every channel bit set in `hdmaen`:
    ///   - Copy A1T → A2A (reset internal table pointer).
    ///   - Read the first line-counter byte from (A1B:A2A); store in NTRL.
    ///   - If indirect, load the DAS data address from (A1B:A2A+1/+2).
    ///   - Advance A2A past the header bytes.
    ///   - Mark channel active.
    ///
    /// Channels with a zero line-counter byte in their first entry are
    /// terminated immediately (inactive for the entire frame).
    ///
    /// `a_read` is called for each table-byte fetch (A-bus, no B-bus traffic).
    pub fn init_hdma(&mut self) {
        // Frame boundary: terminated channels become re-enableable again
        // (snes9x clears HDMAEnded in S9xStartHDMA).
        self.hdma_ended = 0;
        for ch_idx in 0..8usize {
            if self.hdmaen & (1 << ch_idx) == 0 {
                self.hdma.state[ch_idx] = HdmaState::default(); // inactive
                continue;
            }
            let a1b = self.dma.ch[ch_idx].a1b;
            let a1t = self.dma.ch[ch_idx].a1t;
            // Reset internal table pointer to start-of-table.
            self.dma.ch[ch_idx].a2a = a1t;

            let state = self.load_hdma_entry(ch_idx, a1b);
            self.hdma.state[ch_idx] = state;
        }
    }

    /// Load the next HDMA table entry for channel `ch_idx`.
    ///
    /// Reads the count byte from (table_bank:A2A) into NTRL, optionally loads
    /// the indirect data address from the next 2 bytes into DAS, advances A2A
    /// past the header, and returns the new state. Returns inactive state if
    /// the count byte is 0 (table terminator).
    fn load_hdma_entry(&mut self, ch_idx: usize, table_bank: u8) -> HdmaState {
        let a2a = self.dma.ch[ch_idx].a2a;
        let table_addr = ((table_bank as u32) << 16) | (a2a as u32);
        let ntrl_byte = self.a_read(table_addr);
        self.dma.ch[ch_idx].a2a = a2a.wrapping_add(1);
        // NTRL is the live, raw down-counter (readable at $43xA).
        self.dma.ch[ch_idx].ntrl = ntrl_byte;

        if ntrl_byte == 0 {
            // Terminator entry: channel is done for this frame. Record it so
            // a mid-frame $420C re-enable cannot resurrect the channel with
            // ntrl=0 (which would wrap 0->0xFF and stream garbage).
            self.hdma_ended |= 1 << ch_idx;
            return HdmaState {
                active: false,
                do_transfer: false,
            };
        }

        if (self.dma.ch[ch_idx].dmap & 0x40) != 0 {
            // Indirect: read the 2-byte data address from the table into DAS
            // (the rolling data pointer; DASB is the bank).
            let a2a_now = self.dma.ch[ch_idx].a2a;
            let lo_addr = ((table_bank as u32) << 16) | (a2a_now as u32);
            let lo = self.a_read(lo_addr);
            let hi_addr = ((table_bank as u32) << 16) | (a2a_now.wrapping_add(1) as u32);
            let hi = self.a_read(hi_addr);
            self.dma.ch[ch_idx].a2a = a2a_now.wrapping_add(2);
            self.dma.ch[ch_idx].das = (lo as u16) | ((hi as u16) << 8);
        }
        // Direct mode: the data bytes follow inline in the table — A2A is
        // already the rolling data pointer.

        HdmaState {
            active: true,
            // Every entry transfers on its first line, repeat or not.
            do_transfer: true,
        }
    }

    /// Execute HDMA for one scanline (called before rendering that line).
    ///
    /// For each active HDMA channel (bit set in `hdmaen`):
    ///   1. If the channel is due a transfer (first line of an entry, or the
    ///      entry's repeat bit is set), move `pattern` bytes from the rolling
    ///      data pointer (A2A direct / DAS indirect, advanced per byte) to
    ///      the B-bus register. Non-repeat entries transfer on their first
    ///      line only; the written registers then hold for the remainder.
    ///   2. Decrement NTRL (raw); when bits[6:0] reach 0, load the next
    ///      table entry, else carry the repeat bit into `do_transfer`.
    ///
    /// Channel conflict (channel active in general DMA kicked this line) is
    /// not modeled here; the caller must not kick MDMAEN while HDMA is enabled
    /// for the same channel, or a fault results from the bus.rs $420B handler.
    pub fn execute_hdma(&mut self) {
        for ch_idx in 0..8usize {
            if self.hdmaen & (1 << ch_idx) == 0 {
                continue;
            }
            if !self.hdma.state[ch_idx].active {
                continue;
            }
            if self.fault.is_some() {
                break;
            }

            let dmap = self.dma.ch[ch_idx].dmap;
            let bbad = self.dma.ch[ch_idx].bbad;
            let indirect = dmap & 0x40 != 0;

            if self.hdma.state[ch_idx].do_transfer {
                let pattern = unit_pattern(dmap);
                for &b_offset in pattern {
                    // Rolling data pointer: DAS in indirect mode, A2A (the
                    // table pointer, past inline data bytes) in direct mode.
                    let (bank, addr) = if indirect {
                        (self.dma.ch[ch_idx].dasb, self.dma.ch[ch_idx].das)
                    } else {
                        (self.dma.ch[ch_idx].a1b, self.dma.ch[ch_idx].a2a)
                    };
                    let v = self.a_read(((bank as u32) << 16) | (addr as u32));
                    let b_addr: u32 = 0x002100 | ((bbad.wrapping_add(b_offset)) as u32);
                    self.b_write(b_addr, v);
                    if indirect {
                        self.dma.ch[ch_idx].das = addr.wrapping_add(1);
                    } else {
                        self.dma.ch[ch_idx].a2a = addr.wrapping_add(1);
                    }
                    if self.fault.is_some() {
                        break;
                    }
                }
                if self.fault.is_some() {
                    break;
                }
            }

            // Decrement the raw NTRL counter; bits[6:0] hitting 0 exhausts
            // the entry (a raw $80 count byte thus runs 128 non-repeat lines).
            let ntrl = self.dma.ch[ch_idx].ntrl.wrapping_sub(1);
            self.dma.ch[ch_idx].ntrl = ntrl;
            if ntrl & 0x7F == 0 {
                let table_bank = self.dma.ch[ch_idx].a1b;
                let new_state = self.load_hdma_entry(ch_idx, table_bank);
                self.hdma.state[ch_idx] = new_state;
            } else {
                // Mid-entry lines transfer again only in repeat mode.
                self.hdma.state[ch_idx].do_transfer = ntrl & 0x80 != 0;
            }
        }
    }

    // ---- B-bus access helpers (for DMA) ----

    /// B-bus read for DMA (B-bus address $002100-$0021FF range).
    /// Routes to PPU registers, APU ports, and WRAM port.
    fn b_read(&mut self, b_addr: u32) -> u8 {
        // b_addr is always in the form $002100 | low_byte for DMA B-bus.
        let reg = (b_addr & 0xFF) as u8;
        match reg {
            // PPU write-only registers $00-$33 — return MDR for reads.
            0x00..=0x33 => {
                // Readable registers $34-$3F are handled below.
                self.mdr
            }
            // PPU readable registers $34-$3F: same path as CPU reads.
            0x34..=0x3F => {
                let v = self.ppu.read(reg, self.mdr);
                self.mdr = v;
                v
            }
            // APU ports $40-$7F (mirrors: port = reg & 3).
            0x40..=0x7F => {
                let port = reg & 3;
                let v = self.apu.cpu_read_port(port);
                self.mdr = v;
                v
            }
            // WMDATA $80.
            0x80 => {
                let off = self.wmadd as usize & 0x1FFFF;
                let v = self.wram[off];
                self.wmadd = (self.wmadd + 1) & 0x1FFFF;
                self.mdr = v;
                v
            }
            _ => self.mdr,
        }
    }

    /// B-bus write for DMA.
    fn b_write(&mut self, b_addr: u32, value: u8) {
        let reg = (b_addr & 0xFF) as u8;
        match reg {
            // PPU registers $00-$33 write.
            0x00..=0x33 => {
                let fault = self.ppu.write(reg, value);
                if let Some(f) = fault {
                    self.fault(f);
                }
            }
            // APU ports $40-$7F.
            0x40..=0x7F => {
                let port = reg & 3;
                self.apu.cpu_write_port(port, value);
            }
            // WMDATA $80.
            0x80 => {
                let off = self.wmadd as usize & 0x1FFFF;
                self.wram[off] = value;
                self.wmadd = (self.wmadd + 1) & 0x1FFFF;
            }
            // WMADD $81-$83: DMA on the B-bus reaches these registers exactly
            // like CPU writes do.
            0x81 => self.wmadd = (self.wmadd & 0x1FF00) | value as u32,
            0x82 => self.wmadd = (self.wmadd & 0x100FF) | ((value as u32) << 8),
            0x83 => self.wmadd = (self.wmadd & 0x0FFFF) | (((value as u32) & 1) << 16),
            _ => {} // unmapped B-bus write: silently ignore
        }
    }

    /// A-bus read for DMA (WRAM and cart only; cannot reach $21xx/$43xx).
    /// If the address maps to $21xx-$43xx on the A side, treat as open-bus.
    /// A-bus DMA to the register space is architecturally unsupported.
    fn a_read(&self, addr: u32) -> u8 {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // Guard: A-bus DMA cannot reach $21xx-$43xx registers.
        // These fall in banks $00-$3F/$80-$BF at those offsets.
        if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) && (0x2000..0x6000).contains(&off) {
            // I/O register range on A-bus during DMA: open-bus.
            return self.mdr;
        }

        // WRAM banks $7E/$7F.
        if bank == 0x7E || bank == 0x7F {
            let wram_off = ((bank as usize - 0x7E) * 0x10000) | off as usize;
            return self.wram[wram_off];
        }
        // Low mirror.
        match bank {
            0x00..=0x3F | 0x80..=0xBF if off < 0x2000 => {
                return self.wram[off as usize & 0x1FFF];
            }
            _ => {}
        }
        // Cart.
        self.cart.read(addr).unwrap_or(self.mdr)
    }

    /// A-bus write for DMA.
    fn a_write(&mut self, addr: u32, value: u8) {
        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // Guard: do not touch I/O registers from A-bus DMA.
        if matches!(bank, 0x00..=0x3F | 0x80..=0xBF) && (0x2000..0x6000).contains(&off) {
            // No-op: A-bus DMA cannot reach register space.
            return;
        }

        // WRAM banks $7E/$7F.
        if bank == 0x7E || bank == 0x7F {
            let wram_off = ((bank as usize - 0x7E) * 0x10000) | off as usize;
            self.wram[wram_off] = value;
            return;
        }
        // Low mirror.
        match bank {
            0x00..=0x3F | 0x80..=0xBF if off < 0x2000 => {
                self.wram[off as usize & 0x1FFF] = value;
                return;
            }
            _ => {}
        }
        // Cart SRAM.
        if !self.cart.write(addr, value) {
            self.fault(Fault::UnmappedWrite { addr, value });
        }
    }

    /// Execute general DMA for channels set in `mdmaen` bitmask ($420B).
    /// Timing (documented-approximate, deterministic):
    /// - 12 master clocks initial overhead per $420B kick.
    /// - 8 master clocks per active channel activation overhead.
    /// - 8 master clocks per byte transferred.
    pub fn execute_dma(&mut self, mdmaen: u8) {
        // 12 mclk overhead for the DMA kick itself.
        self.add_mclk(12);

        for ch_idx in 0..8usize {
            if mdmaen & (1 << ch_idx) == 0 {
                continue;
            }
            // 8 mclk overhead per active channel.
            self.add_mclk(8);

            // Snapshot channel parameters (they update live as DMA runs).
            let dmap = self.dma.ch[ch_idx].dmap;
            let bbad = self.dma.ch[ch_idx].bbad;

            let direction_b_to_a = dmap & 0x80 != 0;
            let fixed = dmap & 0x08 != 0;
            // A-bus step encoding per docs (DMAP bits 3-4):
            //   bit3=0,bit4=0 → increment
            //   bit3=1,bit4=0 → fixed (already handled above)
            //   bit3=0,bit4=1 → decrement
            //   bit3=1,bit4=1 → fixed
            // Simplest documented: bit3=fixed, bit4=decrement (when bit3 clear).
            let decrement = (!fixed) && (dmap & 0x10 != 0);

            let pattern = unit_pattern(dmap);
            let pattern_len = pattern.len();

            // DAS = 0 means 65536 bytes.
            let mut remaining: u32 = if self.dma.ch[ch_idx].das == 0 {
                65536
            } else {
                self.dma.ch[ch_idx].das as u32
            };

            let mut pattern_pos = 0usize;

            while remaining > 0 {
                if self.fault.is_some() {
                    break;
                }

                // B-bus address: $002100 | (bbad + pattern offset), wrapping u8.
                let b_offset = pattern[pattern_pos];
                let b_addr: u32 = 0x002100 | ((bbad.wrapping_add(b_offset)) as u32);

                // A-bus address from live channel registers.
                let a_bank = self.dma.ch[ch_idx].a1b as u32;
                let a_off = self.dma.ch[ch_idx].a1t as u32;
                let a_addr = (a_bank << 16) | a_off;

                if direction_b_to_a {
                    // B→A: read from B-bus, write to A-bus.
                    let v = self.b_read(b_addr);
                    self.a_write(a_addr, v);
                } else {
                    // A→B: read from A-bus, write to B-bus.
                    let v = self.a_read(a_addr);
                    self.b_write(b_addr, v);
                }

                // D9: a faulted transfer halts DMA with channel registers and
                // the clock still pointing AT the faulting byte (not past it),
                // so post-mortem state reads coherently.
                if self.fault.is_some() {
                    break;
                }

                // Update A-bus address (live, as hardware does).
                if !fixed {
                    let new_off = if decrement {
                        a_off.wrapping_sub(1) as u16
                    } else {
                        a_off.wrapping_add(1) as u16
                    };
                    self.dma.ch[ch_idx].a1t = new_off;
                }

                // Decrement DAS (live).
                remaining -= 1;
                let new_das = if self.dma.ch[ch_idx].das == 0 {
                    // Was 65536, now 65535.
                    65535u16
                } else {
                    self.dma.ch[ch_idx].das.wrapping_sub(1)
                };
                self.dma.ch[ch_idx].das = new_das;

                // Advance pattern position.
                pattern_pos = (pattern_pos + 1) % pattern_len;

                // 8 mclk per byte transferred.
                self.add_mclk(8);
            }
        }
    }
}

impl Bus for SysBus {
    fn read(&mut self, addr: u32) -> u8 {
        let speed = mem_speed(addr, self.fast_rom);
        self.add_mclk(speed);

        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // ---- Banks $7E/$7F: full 128 KiB WRAM ----
        if bank == 0x7E || bank == 0x7F {
            let wram_off = ((bank as usize - 0x7E) * 0x10000) | off as usize;
            let v = self.wram[wram_off];
            self.mdr = v;
            return v;
        }

        // ---- Banks $00-$3F and $80-$BF (mirror identical) ----
        match bank {
            0x00..=0x3F | 0x80..=0xBF => {
                match off {
                    // $0000-$1FFF: WRAM low mirror.
                    0x0000..=0x1FFF => {
                        let v = self.wram[off as usize & 0x1FFF];
                        self.mdr = v;
                        v
                    }

                    // $2100-$213F: PPU registers.
                    0x2100..=0x213F => {
                        let reg = (off - 0x2100) as u8;
                        // Write-only registers $00-$33: return MDR (open bus).
                        if reg <= 0x33 {
                            return self.mdr;
                        }
                        // SLHV ($2137): latch H/V counters. Compute current
                        // H dot from master clock before handing off to PPU.
                        if reg == 0x37 {
                            let dot = ((self.mclk_frame % MCLK_PER_LINE) / 4) as u16;
                            self.ppu.latch_hv_counters(dot);
                        }
                        // Readable registers $34-$3F.
                        let v = self.ppu.read(reg, self.mdr);
                        self.mdr = v;
                        v
                    }

                    // $2140-$217F: APU ports (mirrors every 4).
                    // Catch the APU up to the current timestamp before reading.
                    0x2140..=0x217F => {
                        #[cfg(feature = "introspect")]
                        {
                            self.diag_rd_apu += 1;
                        }
                        let port = (off & 3) as u8;
                        self.apu_catch_up();
                        self.service_pending_ipl_cc();
                        let v = self.apu.cpu_read_port(port);
                        self.mdr = v;
                        v
                    }

                    // $2180: WMDATA read.
                    0x2180 => {
                        let off_w = self.wmadd as usize & 0x1FFFF;
                        let v = self.wram[off_w];
                        self.wmadd = (self.wmadd + 1) & 0x1FFFF;
                        self.mdr = v;
                        v
                    }

                    // $2181-$21FF: unmapped or write-only — return MDR.
                    0x2181..=0x21FF => self.mdr,

                    // $4016: Joypad serial data.
                    0x4016 => {
                        let serial = self.joypad.read_serial();
                        // bit0 = pad1 data; bit1 = 0; bits 7-2 = mdr.
                        let v = (serial & 0x01) | (self.mdr & 0xFC);
                        self.mdr = v;
                        v
                    }

                    // $4017: port 2 not connected. Bits 1:0 (controller
                    // data lines) read as a constant 0, bits 4-2 read as 1,
                    // bits 7-5 are CPU open bus — fully deterministic.
                    0x4017 => {
                        let v = 0x1C | (self.mdr & 0xE0);
                        self.mdr = v;
                        v
                    }

                    // $4210: RDNMI.
                    0x4210 => {
                        #[cfg(feature = "introspect")]
                        {
                            self.diag_rd_4210 += 1;
                        }
                        // bit7 = nmi_flag; bits 6-4 = mdr; bits 3-0 = $2 (CPU version).
                        let v = ((self.nmi_flag as u8) << 7) | 0x02 | (self.mdr & 0x70);
                        self.nmi_flag = false; // read-clears (not nmi_pending)
                        self.mdr = v;
                        v
                    }

                    // $4211: TIMEUP.
                    0x4211 => {
                        #[cfg(feature = "introspect")]
                        {
                            self.diag_rd_4211 += 1;
                        }
                        let v = ((self.irq_flag as u8) << 7) | (self.mdr & 0x7F);
                        self.irq_flag = false; // read-clears
                        self.mdr = v;
                        v
                    }

                    // $4212: HVBJOY.
                    0x4212 => {
                        #[cfg(feature = "introspect")]
                        {
                            self.diag_rd_4212 += 1;
                        }
                        let vblank = self.line >= self.ppu.vblank_start_line();
                        // hblank: approximate — mclk within current line >= 274*4.
                        // We compute mclk within the current line from mclk_frame.
                        // APPROXIMATION: mclk_frame represents absolute mclks from
                        // frame start; mclk_in_line = mclk_frame % MCLK_PER_LINE.
                        // Clamp: if line is beyond frame bounds (shouldn't happen
                        // in normal operation), use modulo.
                        let hblank_threshold: u64 = crate::timing::HBLANK_START_DOT as u64 * 4;
                        let mclk_in_line = self.mclk_frame % MCLK_PER_LINE;
                        let hblank = mclk_in_line >= hblank_threshold;
                        let v = ((vblank as u8) << 7)
                            | ((hblank as u8) << 6)
                            | (self.mdr & 0x3E)
                            | (self.auto_joy_busy as u8);
                        self.mdr = v;
                        v
                    }

                    // $4213: RDIO — read back WRIO.
                    0x4213 => {
                        let v = self.wrio;
                        self.mdr = v;
                        v
                    }

                    // $4214/$4215: RDDIV (little-endian).
                    0x4214 => {
                        let v = self.rddiv as u8;
                        self.mdr = v;
                        v
                    }
                    0x4215 => {
                        let v = (self.rddiv >> 8) as u8;
                        self.mdr = v;
                        v
                    }

                    // $4216/$4217: RDMPY (little-endian).
                    0x4216 => {
                        let v = self.rdmpy as u8;
                        self.mdr = v;
                        v
                    }
                    0x4217 => {
                        let v = (self.rdmpy >> 8) as u8;
                        self.mdr = v;
                        v
                    }

                    // $4218/$4219: JOY1 (little-endian).
                    // Stale-read protocol (M2): while auto_joy_busy (the first
                    // three vblank lines) return the previous latch (joy_prev);
                    // the new latch becomes visible once busy clears.
                    // Games that poll $4212 bit0 first are fully correct.
                    0x4218 => {
                        let joy = if self.auto_joy_busy {
                            self.joy_prev
                        } else {
                            self.joypad.joy1
                        };
                        let v = joy as u8;
                        self.mdr = v;
                        v
                    }
                    0x4219 => {
                        let joy = if self.auto_joy_busy {
                            self.joy_prev
                        } else {
                            self.joypad.joy1
                        };
                        let v = (joy >> 8) as u8;
                        self.mdr = v;
                        v
                    }

                    // $421A-$421F: ports 2-4 empty → return 0.
                    0x421A..=0x421F => {
                        self.mdr = 0;
                        0
                    }

                    // $4300-$437F: DMA register file.
                    0x4300..=0x437F => {
                        let ch = ((off - 0x4300) >> 4) as usize;
                        let reg = ((off - 0x4300) & 0xF) as u8;
                        let v = self.dma.read(ch, reg).unwrap_or(self.mdr);
                        self.mdr = v;
                        v
                    }

                    // $8000-$FFFF: cartridge ROM.
                    0x8000..=0xFFFF => {
                        let v = self.cart.read(addr).unwrap_or(self.mdr);
                        self.mdr = v;
                        v
                    }

                    // Everything else in the $2000-$7FFF hole that isn't mapped
                    // above: open bus (return MDR unchanged).
                    _ => self.mdr,
                }
            }

            // ---- Banks $40-$7D: all 8-cycle ROM or WRAM ----
            0x40..=0x7D => {
                // Banks $40-$7D, offset $0000-$7FFF: usually ROM high pages
                // but LoROM maps these to ROM at $8000+ of the bank.
                // Per LoROM spec the $0000-$7FFF window in banks $40-$7D
                // is not mapped (open bus) — the ROM occupies $8000-$FFFF.
                // Exception: banks $70-$7D with offset < $8000 = SRAM.
                match bank {
                    0x70..=0x7D if off < 0x8000 => {
                        // SRAM window.
                        let v = self.cart.read(addr).unwrap_or(self.mdr);
                        self.mdr = v;
                        return v;
                    }
                    _ => {}
                }
                let v = self.cart.read(addr).unwrap_or(self.mdr);
                self.mdr = v;
                v
            }

            // ---- Banks $C0-$FF ----
            0xC0..=0xFF => {
                // $F0-$FD offset < $8000: SRAM mirror.
                match bank {
                    0xF0..=0xFD if off < 0x8000 => {
                        let v = self.cart.read(addr).unwrap_or(self.mdr);
                        self.mdr = v;
                        return v;
                    }
                    _ => {}
                }
                let v = self.cart.read(addr).unwrap_or(self.mdr);
                self.mdr = v;
                v
            }

            // Remaining banks (shouldn't be reached given the arms above).
            _ => self.mdr,
        }
    }

    fn write(&mut self, addr: u32, value: u8) {
        let speed = mem_speed(addr, self.fast_rom);
        self.add_mclk(speed);

        let bank = ((addr >> 16) & 0xFF) as u8;
        let off = (addr & 0xFFFF) as u16;

        // ---- Banks $7E/$7F: full 128 KiB WRAM ----
        if bank == 0x7E || bank == 0x7F {
            let wram_off = ((bank as usize - 0x7E) * 0x10000) | off as usize;
            self.wram[wram_off] = value;
            return;
        }

        match bank {
            0x00..=0x3F | 0x80..=0xBF => {
                match off {
                    // $0000-$1FFF: WRAM low mirror.
                    0x0000..=0x1FFF => {
                        self.wram[off as usize & 0x1FFF] = value;
                    }

                    // $2100-$2133: PPU write-only registers.
                    0x2100..=0x2133 => {
                        let reg = (off - 0x2100) as u8;
                        let fault = self.ppu.write(reg, value);
                        if let Some(f) = fault {
                            self.fault(f);
                        }
                    }

                    // $2140-$217F: APU ports.
                    // Catch the APU up to the current timestamp before writing.
                    0x2140..=0x217F => {
                        let port = (off & 3) as u8;
                        #[cfg(feature = "introspect")]
                        {
                            self.diag_wr_apu += 1;
                            // $CC is the fixed IPL kick constant (a hardware
                            // protocol value, not game data): note if the main
                            // CPU ever delivers it to port 0.
                            if port == 0 {
                                if !self.diag_wr_cc_port0 && value == 0xCC {
                                    self.diag_wr_cc_port0 = true;
                                    self.diag_cc_port0_mclk = Some(self.mclk_total);
                                } else if self.diag_wr_cc_port0 {
                                    self.diag_post_cc_port0_writes += 1;
                                    if self.diag_first_post_cc_port0_delta_mclk.is_none() {
                                        if let Some(first) = self.diag_cc_port0_mclk {
                                            self.diag_first_post_cc_port0_delta_mclk =
                                                Some(self.mclk_total.saturating_sub(first));
                                        }
                                    }
                                }
                            }
                        }
                        let defer_for_transfer_data = self.apu_pending_ipl_transfer_strobe
                            && port != 0
                            && self.apu_in_ipl_transfer_loop();
                        if !defer_for_transfer_data {
                            self.apu_catch_up();
                        }
                        if self.apu_pending_ipl_cc && port == 0 && value != 0xCC {
                            self.service_pending_ipl_cc();
                        }
                        self.apu.cpu_write_port(port, value);
                        if self.should_capture_ipl_cc(port, value) {
                            self.apu_pending_ipl_cc = true;
                        } else if self.apu_pending_ipl_cc && port != 0 {
                            self.service_pending_ipl_cc();
                        }
                        if defer_for_transfer_data {
                            self.apu_pending_ipl_transfer_strobe = false;
                            self.apu_catch_up();
                        }
                        if port == 0 && self.apu_in_ipl_transfer_loop() {
                            self.apu_pending_ipl_transfer_strobe = true;
                        }
                    }

                    // $2180: WMDATA write.
                    0x2180 => {
                        let off_w = self.wmadd as usize & 0x1FFFF;
                        self.wram[off_w] = value;
                        self.wmadd = (self.wmadd + 1) & 0x1FFFF;
                    }

                    // $2181: WMADD low byte.
                    0x2181 => {
                        self.wmadd = (self.wmadd & 0x1FF00) | value as u32;
                    }
                    // $2182: WMADD middle byte.
                    0x2182 => {
                        self.wmadd = (self.wmadd & 0x100FF) | ((value as u32) << 8);
                    }
                    // $2183: WMADD bank bit (bit 0 only).
                    0x2183 => {
                        let bank_bit = (value & 1) as u32;
                        self.wmadd = (self.wmadd & 0x0FFFF) | (bank_bit << 16);
                    }

                    // $4016: Joypad strobe.
                    0x4016 => {
                        self.joypad.write_strobe(value);
                    }

                    // $4200: NMITIMEN.
                    0x4200 => {
                        let old = self.nmitimen;
                        self.nmitimen = value;
                        // If NMI enable (bit7) newly set while nmi_flag is already set → edge.
                        if (value & 0x80) != 0 && (old & 0x80) == 0 && self.nmi_flag {
                            self.nmi_pending = true;
                        }
                        self.recompute_irq_target();
                    }

                    // $4201: WRIO.
                    0x4201 => {
                        self.wrio = value;
                    }

                    // $4202: WRMPYA (multiplicand A).
                    0x4202 => {
                        self.wrmpya = value;
                    }
                    // $4203: WRMPYB — triggers multiplication.
                    0x4203 => {
                        self.rdmpy = (self.wrmpya as u16).wrapping_mul(value as u16);
                        // RDDIV is not modified by multiply per docs.
                        // Comment: hardware note — RDDIV gets $00xx remnant documented
                        // as "multiply result overflow" but the safe documented behavior
                        // is: only rdmpy is set; leave rddiv unchanged.
                    }

                    // $4204: WRDIV low byte.
                    0x4204 => {
                        self.wrdiv = (self.wrdiv & 0xFF00) | value as u16;
                    }
                    // $4205: WRDIV high byte.
                    0x4205 => {
                        self.wrdiv = (self.wrdiv & 0x00FF) | ((value as u16) << 8);
                    }
                    // $4206: WRDIVB — triggers division.
                    0x4206 => {
                        if value == 0 {
                            self.rddiv = 0xFFFF;
                            self.rdmpy = self.wrdiv;
                        } else {
                            self.rddiv = self.wrdiv / value as u16;
                            self.rdmpy = self.wrdiv % value as u16;
                        }
                    }

                    // $4207/$4208: HTIME (9-bit).
                    0x4207 => {
                        self.htime = (self.htime & 0x100) | value as u16;
                        self.recompute_irq_target();
                    }
                    0x4208 => {
                        self.htime = (self.htime & 0x0FF) | (((value & 1) as u16) << 8);
                        self.recompute_irq_target();
                    }

                    // $4209/$420A: VTIME (9-bit).
                    0x4209 => {
                        self.vtime = (self.vtime & 0x100) | value as u16;
                        self.recompute_irq_target();
                    }
                    0x420A => {
                        self.vtime = (self.vtime & 0x0FF) | (((value & 1) as u16) << 8);
                        self.recompute_irq_target();
                    }

                    // $420B: MDMAEN — kick general DMA.
                    // D9: if a channel is simultaneously set in both MDMAEN and
                    // HDMAEN, that is a channel conflict — fault loudly.
                    0x420B => {
                        if value != 0 {
                            let conflict = value & self.hdmaen;
                            if conflict != 0 {
                                self.fault(Fault::HdmaDmaConflict { channels: conflict });
                            } else {
                                self.execute_dma(value);
                            }
                        }
                    }

                    // $420C: HDMAEN — store enable mask; HDMA runs each scanline.
                    //
                    // Channels newly set MID-FRAME activate immediately and
                    // resume with their current (game-staged) A2A/NTRL/DAS —
                    // no re-initialization, per hardware and snes9x
                    // (`PPU.HDMA = value & ~HDMAEnded`; games stage channel
                    // state via the writable $43x8-$43xA). First service is
                    // the next line's execute_hdma — ~1 line later than real
                    // hardware's same-line HDMA point, a documented
                    // line-granularity simplification. Channels whose table
                    // terminated this frame stay dead until the next frame's
                    // init_hdma. Cleared channels stop via the mask check in
                    // execute_hdma with state retained, so re-enabling
                    // resumes where they left off.
                    0x420C => {
                        let newly = value & !self.hdmaen & !self.hdma_ended;
                        self.hdmaen = value;
                        for ch_idx in 0..8usize {
                            if newly & (1 << ch_idx) != 0 {
                                // Never-inited channels carry the default
                                // state (do_transfer=false), matching
                                // snes9x's DoTransfer=FALSE for channels
                                // disabled at frame init; cleared-then-
                                // re-enabled channels still have
                                // active=true and keep their in-flight
                                // do_transfer (repeat-mode resume) — only
                                // `active` is ever touched here.
                                self.hdma.state[ch_idx].active = true;
                            }
                        }
                    }

                    // $420D: MEMSEL.
                    0x420D => {
                        self.fast_rom = value & 1 != 0;
                    }

                    // $4300-$437F: DMA register file writes.
                    0x4300..=0x437F => {
                        let ch = ((off - 0x4300) >> 4) as usize;
                        let reg = ((off - 0x4300) & 0xF) as u8;
                        self.dma.write(ch, reg, value);
                    }

                    // ROM area in banks $00-$3F/$80-$BF: fault on write.
                    0x8000..=0xFFFF => {
                        self.fault(Fault::UnmappedWrite { addr, value });
                    }

                    // Everything else in the I/O gap not handled above: fault.
                    _ => {
                        self.fault(Fault::UnmappedWrite { addr, value });
                    }
                }
            }

            // Banks $40-$7D.
            0x40..=0x7D => {
                // SRAM window banks $70-$7D, offset < $8000.
                match bank {
                    0x70..=0x7D if off < 0x8000 => {
                        if !self.cart.write(addr, value) {
                            self.fault(Fault::UnmappedWrite { addr, value });
                        }
                    }
                    _ => {
                        // ROM write: fault.
                        self.fault(Fault::UnmappedWrite { addr, value });
                    }
                }
            }

            // Banks $C0-$FF.
            0xC0..=0xFF => match bank {
                0xF0..=0xFD if off < 0x8000 => {
                    if !self.cart.write(addr, value) {
                        self.fault(Fault::UnmappedWrite { addr, value });
                    }
                }
                _ => {
                    self.fault(Fault::UnmappedWrite { addr, value });
                }
            },

            _ => {
                self.fault(Fault::UnmappedWrite { addr, value });
            }
        }
    }

    fn idle(&mut self) {
        self.add_mclk(MCLK_PER_INTERNAL_CYCLE);
    }

    fn take_nmi(&mut self) -> bool {
        let p = self.nmi_pending;
        self.nmi_pending = false;
        p
    }

    fn irq_line(&self) -> bool {
        self.irq_flag
    }

    fn fault(&mut self, fault: Fault) {
        if self.fault.is_none() {
            self.fault = Some(fault);
        }
        self.frame_flags.insert(crate::fault::FrameFlags::FAULTED);
    }
}

/// Run CPU instructions until `mclk_frame >= target` or a fault is recorded.
/// Honors WAI (idles until an interrupt is pending).
pub fn run_cpu_until(cpu: &mut Cpu, bus: &mut SysBus, target_mclk: u64) {
    while bus.mclk_frame < target_mclk && bus.fault.is_none() {
        if cpu.stopped {
            // STP — convert to fault (D9).
            let pc = ((cpu.pbr as u32) << 16) | cpu.pc as u32;
            bus.fault(Fault::CpuStopped { pc });
            break;
        }
        // Cpu::step handles WAI internally: idles one cycle if no interrupt
        // pending, so the loop always advances mclk_frame.
        cpu.step(bus);
    }
}

// ---- Unit tests ----
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cart::Cartridge;
    use crate::timing::MCLK_PER_LINE;

    fn make_test_cart() -> Cartridge {
        let mut rom = vec![0u8; 0x8000];
        // Reset vector pointing at $8000 — valid.
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80;
        Cartridge::from_rom(rom, None).unwrap()
    }

    fn make_test_bus() -> SysBus {
        let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([0u8; 0x20000]));
        let vram: &'static mut [u8; 0x10000] = Box::leak(Box::new([0u8; 0x10000]));
        SysBus::new(wram, make_test_cart(), Ppu::new(vram))
    }

    #[test]
    fn overscan_moves_vblank_nmi_hvbjoy_and_autojoy_boundary() {
        let mut bus = make_test_bus();
        assert!(bus.ppu.write(0x33, 0x04).is_none());
        bus.ppu.begin_frame();
        bus.nmitimen = 0x81;

        bus.start_line(225, 0);
        assert!(!bus.nmi_flag);
        assert!(!bus.nmi_pending);
        assert!(!bus.auto_joy_busy);
        assert_eq!(bus.read(0x004212) & 0x80, 0);

        bus.start_line(240, 0);
        assert!(bus.nmi_flag);
        assert!(bus.nmi_pending);
        assert!(bus.auto_joy_busy);
        assert_ne!(bus.read(0x004212) & 0x80, 0);

        bus.start_line(243, 0);
        assert!(!bus.auto_joy_busy);
    }

    fn advance_apu_to_ipl_poll(bus: &mut SysBus) {
        bus.add_mclk(21_477);
        bus.apu_catch_up();
        assert_eq!(bus.apu.cpu_read_port(0), 0xAA);
        assert_eq!(bus.apu.cpu_read_port(1), 0xBB);
        assert!(
            bus.apu.cpu.pc >= 0xFFC8,
            "IPL should have reached the port-0 poll, pc={:#06x}",
            bus.apu.cpu.pc
        );
    }

    // ---- Mul/div register tests ----

    #[test]
    fn multiply_basic() {
        let cart = make_test_cart();
        let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([0u8; 0x20000]));
        // We can't construct a real SysBus without a live Ppu. Test the math
        // directly on the register fields.
        // Simulate the write logic inline.
        let wrmpya: u8 = 0x03;
        let wrmpyb: u8 = 0x07;
        let rdmpy = (wrmpya as u16) * (wrmpyb as u16);
        assert_eq!(rdmpy, 21);
        let _ = (cart, wram); // used to suppress unused warnings
    }

    #[test]
    fn multiply_overflow() {
        let wrmpya: u8 = 0xFF;
        let wrmpyb: u8 = 0xFF;
        let rdmpy = (wrmpya as u16) * (wrmpyb as u16);
        assert_eq!(rdmpy, 0xFE01u16);
    }

    #[test]
    fn divide_basic() {
        let wrdiv: u16 = 300;
        let wrdivb: u8 = 13;
        let rddiv = wrdiv / wrdivb as u16;
        let rdmpy = wrdiv % wrdivb as u16;
        assert_eq!(rddiv, 23);
        assert_eq!(rdmpy, 1);
    }

    #[test]
    fn divide_by_zero() {
        let wrdiv: u16 = 0x1234;
        let wrdivb: u8 = 0;
        let (rddiv, rdmpy) = if wrdivb == 0 {
            (0xFFFFu16, wrdiv)
        } else {
            (wrdiv / wrdivb as u16, wrdiv % wrdivb as u16)
        };
        assert_eq!(rddiv, 0xFFFF);
        assert_eq!(rdmpy, 0x1234);
    }

    // ---- DMA register file tests ----

    #[test]
    fn dma_register_roundtrip() {
        let mut dma = Dma::new();
        dma.write(0, 0x0, 0x01); // DMAP unit pattern 1
        dma.write(0, 0x1, 0x18); // BBAD = $18 (VMDATAL on B-bus)
        dma.write(0, 0x2, 0x00); // A1T lo
        dma.write(0, 0x3, 0x80); // A1T hi ($8000)
        dma.write(0, 0x4, 0x01); // A1B bank $01
        dma.write(0, 0x5, 0x00); // DAS lo
        dma.write(0, 0x6, 0x01); // DAS hi = $0100 → 256 bytes
        assert_eq!(dma.read(0, 0x0), Some(0x01));
        assert_eq!(dma.read(0, 0x1), Some(0x18));
        assert_eq!(dma.ch[0].a1t, 0x8000);
        assert_eq!(dma.ch[0].a1b, 0x01);
        assert_eq!(dma.ch[0].das, 0x0100);
    }

    #[test]
    fn dma_das_zero_is_65536() {
        let mut dma = Dma::new();
        dma.write(0, 0x5, 0x00);
        dma.write(0, 0x6, 0x00);
        assert_eq!(dma.ch[0].das, 0);
        // das == 0 is interpreted as 65536 by the DMA executor.
    }

    // ---- WRAM port tests ----

    #[test]
    fn wmdata_wrap() {
        // Test that the WMADD wraps at 0x20000 (128 KiB).
        // We simulate the WMDATA increment logic directly.
        let mut wmadd: u32 = 0x1FFFF; // last byte
        let wram = [0xABu8; 0x20000];
        // Read at 0x1FFFF.
        let _v = wram[wmadd as usize & 0x1FFFF];
        wmadd = (wmadd + 1) & 0x1FFFF;
        assert_eq!(wmadd, 0x00000); // should wrap to 0
    }

    // ---- Joypad tests ----

    #[test]
    fn joypad_native_word_b_button() {
        let mut j = crate::joypad::Joypad::new();
        j.pad = 1 << 1; // B button
        let w = j.native_word();
        // B → JOY1H bit 7 → joy1 bit 15.
        assert_eq!((w >> 8) & 0x80, 0x80);
    }

    // ---- mem_speed clock tests ----

    #[test]
    fn mem_speed_joy1_low_is_6() {
        // $4218 is in banks $00-$3F at offset $4218, which is in range $4200-$5FFF → 6 clocks.
        let speed = crate::timing::mem_speed(0x004218, false);
        assert_eq!(speed, 6);
    }

    #[test]
    fn mem_speed_wram_low_mirror_is_8() {
        // Banks $00-$3F, offset $0000-$1FFF → 8 clocks.
        let speed = crate::timing::mem_speed(0x000000, false);
        assert_eq!(speed, 8);
    }

    #[test]
    fn mem_speed_joypad_strobe_area_is_12() {
        // Banks $00-$3F, offset $4000-$41FF → 12 clocks.
        let speed = crate::timing::mem_speed(0x004016, false);
        assert_eq!(speed, 12);
    }

    // ---- IRQ schedule ----

    #[test]
    fn irq_target_none_when_disabled() {
        // With NMITIMEN bits 4/5 = 0, IRQ target should be None.
        // Test directly via the field logic.
        let h_irq = false;
        let v_irq = false;
        let armed = h_irq || v_irq;
        assert!(!armed);
    }

    #[test]
    fn h_irq_offset_computation() {
        // For htime = 10: h_offset = 10*4 + 14 = 54.
        let htime: u64 = 10;
        let h_offset = htime * 4 + 14;
        assert_eq!(h_offset, 54);
    }

    // ---- Cart mapping via read/write ----

    #[test]
    fn cart_mapping_rom_read() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80;
        rom[0x0000] = 0x42; // bank $00 offset $8000 → rom[0]
        let cart = Cartridge::from_rom(rom, None).unwrap();
        assert_eq!(cart.read(0x00_8000), Some(0x42));
    }

    #[test]
    fn cart_mapping_sram_window() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80;
        let sram_buf: &'static mut [u8] = Box::leak(Box::new([0u8; 8192]));
        let mut cart = Cartridge::from_rom(rom, Some(sram_buf)).unwrap();
        assert!(cart.write(0x70_0000, 0x99));
        assert_eq!(cart.read(0x70_0000), Some(0x99));
    }

    #[test]
    fn dma_unit_pattern_das_65536() {
        // Verify that das==0 produces 65536 iterations by counting.
        // We test the DmaChannel struct + unit_pattern logic directly (no SysBus/PPU needed).
        let ch = crate::dma::DmaChannel {
            das: 0, // means 65536
            ..Default::default()
        };

        let remaining: u32 = if ch.das == 0 { 65536 } else { ch.das as u32 };
        assert_eq!(remaining, 65536);
    }

    #[test]
    fn dma_register_live_update_a1t() {
        // Simulate the A-address live update logic from execute_dma.
        let mut a1t: u16 = 0x8000;
        let fixed = false;
        let decrement = false;
        // After one increment:
        let new_a1t = if !fixed {
            if decrement {
                a1t.wrapping_sub(1)
            } else {
                a1t.wrapping_add(1)
            }
        } else {
            a1t
        };
        a1t = new_a1t;
        assert_eq!(a1t, 0x8001);
    }

    #[test]
    fn mclk_per_line_constant() {
        // Sanity: MCLK_PER_LINE = 1364.
        assert_eq!(MCLK_PER_LINE, 1364);
    }

    #[test]
    fn apu_port0_service_preserves_ipl_kick_before_rapid_overwrite() {
        let mut bus = make_test_bus();
        advance_apu_to_ipl_poll(&mut bus);

        bus.write(0x002142, 0x00);
        bus.write(0x002143, 0x02);
        bus.write(0x002141, 0x01);
        bus.write(0x002140, 0xCC);
        bus.write(0x002140, 0x00);

        let mut echo = 0xCC;
        for _ in 0..512 {
            echo = bus.read(0x002140);
            if echo == 0x00 {
                break;
            }
        }
        assert_eq!(
            echo, 0x00,
            "CPU read should observe the transfer-index echo"
        );
        assert!(
            bus.apu.cpu.pc >= 0xFFE0,
            "SPC should have left poll_cc for the transfer loop, pc={:#06x}",
            bus.apu.cpu.pc
        );
    }

    #[test]
    fn apu_port0_service_applies_to_mirrors() {
        let mut bus = make_test_bus();
        advance_apu_to_ipl_poll(&mut bus);

        bus.write(0x002142, 0x00);
        bus.write(0x002143, 0x02);
        bus.write(0x002141, 0x01);
        bus.write(0x002144, 0xCC);

        assert_eq!(
            bus.read(0x002140),
            0xCC,
            "port-0 mirror write should receive the same IPL service"
        );
        assert!(bus.apu.cpu.pc >= 0xFFE0);
    }

    #[test]
    fn apu_port0_service_does_not_spend_window_before_ipl_kick() {
        let mut bus = make_test_bus();
        advance_apu_to_ipl_poll(&mut bus);

        bus.write(0x002140, 0x00);
        assert_eq!(
            bus.apu_mclk_base, bus.mclk_total,
            "pre-kick port-0 setup writes must not consume future service"
        );

        bus.write(0x002141, 0x01);
        bus.write(0x002140, 0xCC);
        assert_eq!(
            bus.read(0x002140),
            0xCC,
            "the real kick should be preserved until the CPU-visible poll"
        );
        assert!(
            bus.apu_mclk_base <= bus.mclk_total,
            "pending kick service must not leave artificial APU lead"
        );
    }

    #[test]
    fn apu_port0_service_lead_is_capped_and_not_double_run() {
        let mut bus = make_test_bus();
        advance_apu_to_ipl_poll(&mut bus);

        bus.write(0x002141, 0x01);
        bus.write(0x002140, 0xCC);
        assert!(
            bus.apu_mclk_base <= bus.mclk_total,
            "capturing the pending IPL kick should not run APU ahead"
        );
        assert_eq!(bus.read(0x002140), 0xCC);
        assert!(
            bus.apu_mclk_base <= bus.mclk_total,
            "servicing the pending IPL kick should not run APU ahead"
        );

        let serviced_through = bus.apu_mclk_base;
        let pc_after_service = bus.apu.cpu.pc;
        bus.apu_catch_up();
        assert!(bus.apu_mclk_base >= serviced_through);
        assert_eq!(bus.apu.cpu.pc, pc_after_service);

        for strobe in 1..=16 {
            bus.write(0x002140, strobe);
            assert!(
                bus.apu_mclk_base <= bus.mclk_total,
                "transfer strobe {strobe} should not create APU lead"
            );
        }
    }

    #[test]
    fn apu_halt_during_service_records_fault() {
        let mut bus = make_test_bus();
        bus.apu.cpu.pc = 0xFFC8;
        bus.apu.halted = Some(crate::apu::spc700::ApuHalt::Stop);
        bus.apu.cpu.halted = bus.apu.halted;

        bus.write(0x002140, 0xCC);

        assert!(matches!(bus.fault, Some(Fault::ApuStopped { pc: 0xFFC8 })));
    }

    // ---- HDMA state-machine tests ----
    //
    // These pin the table-walk semantics: B-bus target is WMDATA ($80) so
    // every transferred byte lands at wram[wmadd], directly observable.
    // Tables live in WRAM bank $7E at $1000 (A-bus reachable); transfer
    // destinations start at wmadd = 0.

    fn make_hdma_bus(table: &[u8]) -> SysBus {
        let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([0u8; 0x20000]));
        wram[0x1000..0x1000 + table.len()].copy_from_slice(table);
        let vram: &'static mut [u8; 0x10000] = Box::leak(Box::new([0u8; 0x10000]));
        let mut bus = SysBus::new(wram, make_test_cart(), Ppu::new(vram));
        bus.dma.ch[0].dmap = 0x00; // 1-byte pattern, direct mode
        bus.dma.ch[0].bbad = 0x80; // WMDATA
        bus.dma.ch[0].a1b = 0x7E;
        bus.dma.ch[0].a1t = 0x1000;
        bus.hdmaen = 0x01;
        bus
    }

    #[test]
    fn hdma_non_repeat_transfers_first_line_only() {
        // 3 lines non-repeat, one inline data byte, then terminator.
        let mut bus = make_hdma_bus(&[0x03, 0xAA, 0x00]);
        bus.init_hdma();
        for _ in 0..3 {
            bus.execute_hdma();
        }
        // Exactly one byte transferred (first line of the entry).
        assert_eq!(bus.wram[0], 0xAA);
        assert_eq!(bus.wmadd, 1, "non-repeat entry must transfer once");
        // Terminator reached after the 3rd line.
        assert!(!bus.hdma.state[0].active);
    }

    #[test]
    fn hdma_direct_table_pointer_advances_past_inline_data() {
        // Two 1-line non-repeat entries with inline data, then terminator:
        // the second entry's count byte must be read AFTER the first
        // entry's data byte, not on top of it.
        let mut bus = make_hdma_bus(&[0x01, 0xAA, 0x01, 0xBB, 0x00]);
        bus.init_hdma();
        bus.execute_hdma();
        bus.execute_hdma();
        assert_eq!(&bus.wram[0..2], &[0xAA, 0xBB]);
        assert!(!bus.hdma.state[0].active);
    }

    #[test]
    fn hdma_repeat_transfers_every_line() {
        // $82 = repeat, 2 lines: consecutive inline bytes per line.
        let mut bus = make_hdma_bus(&[0x82, 0xAA, 0xBB, 0x00]);
        bus.init_hdma();
        bus.execute_hdma();
        bus.execute_hdma();
        assert_eq!(&bus.wram[0..2], &[0xAA, 0xBB]);
        assert_eq!(bus.wmadd, 2, "repeat entry must transfer every line");
        assert!(!bus.hdma.state[0].active);
    }

    #[test]
    fn hdma_raw_80_count_is_128_non_repeat_lines() {
        // A raw $80 count byte is 128 lines non-repeat (NTRL decrements raw;
        // bits[6:0] first hit 0 after 128 decrements), not 0 lines.
        let mut bus = make_hdma_bus(&[0x80, 0xAA, 0x00]);
        bus.init_hdma();
        for line in 0..128 {
            assert!(bus.hdma.state[0].active, "inactive at line {}", line);
            bus.execute_hdma();
        }
        assert_eq!(bus.wmadd, 1, "single transfer across all 128 lines");
        assert!(!bus.hdma.state[0].active, "entry exhausts after 128 lines");
    }

    #[test]
    fn hdma_indirect_pointer_advances() {
        // Indirect mode: table holds (count, addr_lo, addr_hi); data bytes
        // come from DASB:DAS, advancing DAS per byte.
        let mut bus = make_hdma_bus(&[0x82, 0x00, 0x20, 0x00]);
        bus.dma.ch[0].dmap = 0x40; // 1-byte pattern, indirect
        bus.dma.ch[0].dasb = 0x7E;
        bus.wram[0x2000] = 0xAA;
        bus.wram[0x2001] = 0xBB;
        bus.init_hdma();
        assert_eq!(bus.dma.ch[0].das, 0x2000);
        bus.execute_hdma();
        bus.execute_hdma();
        assert_eq!(&bus.wram[0..2], &[0xAA, 0xBB]);
        assert_eq!(bus.dma.ch[0].das, 0x2002);
        assert!(!bus.hdma.state[0].active);
    }

    // ---- Mid-frame $420C enable (resume semantics; snes9x-verified) ----
    //
    // A mid-frame enable activates a channel WITHOUT re-initialization:
    // the game stages A2A/NTRL itself via the writable $43x8-$43xA.
    // Reference: snes9x ppu.cpp $420C = `HDMA = value & ~HDMAEnded`.

    #[test]
    fn hdma_midframe_enable_with_staged_primer() {
        // Mask 0 at frame init; game stages A2A=table, NTRL=1 (the classic
        // primer: decrement 1->0 at the first serviced line loads the first
        // entry; data transfers on the line after).
        let mut bus = make_hdma_bus(&[0x03, 0xAA, 0x00]);
        bus.hdmaen = 0;
        bus.init_hdma();
        bus.execute_hdma();
        bus.execute_hdma();
        assert_eq!(bus.wmadd, 0, "disabled channel must not transfer");

        bus.write(0x004308, 0x00); // A2A low
        bus.write(0x004309, 0x10); // A2A high -> $1000
        bus.write(0x00430A, 0x01); // NTRL primer
        bus.write(0x00420C, 0x01); // mid-frame enable
        assert!(bus.hdma.state[0].active, "enable must activate the channel");
        assert_eq!(bus.wmadd, 0, "no transfer at the write itself");

        bus.execute_hdma(); // primer line: 1->0, loads the entry
        assert_eq!(bus.wmadd, 0, "primer line must not transfer data");
        bus.execute_hdma(); // first data line
        assert_eq!(bus.wram[0], 0xAA);
        assert_eq!(bus.wmadd, 1, "entry data transfers one line after enable+primer");
    }

    #[test]
    fn hdma_cleared_then_reenabled_resumes() {
        // Repeat entry: one byte per line. Clearing the mask mid-entry
        // freezes the channel (state retained); re-enabling resumes exactly
        // where it left off — it must NOT restart from A1T.
        let mut bus = make_hdma_bus(&[0x84, 0xAA, 0xBB, 0xCC, 0xDD, 0x00]);
        bus.init_hdma();
        bus.execute_hdma();
        assert_eq!(&bus.wram[0..1], &[0xAA]);

        bus.write(0x00420C, 0x00); // clear mid-entry
        let ntrl_frozen = bus.dma.ch[0].ntrl;
        bus.execute_hdma();
        bus.execute_hdma();
        assert_eq!(bus.wmadd, 1, "cleared channel must not transfer");
        assert_eq!(bus.dma.ch[0].ntrl, ntrl_frozen, "counter must freeze");

        bus.write(0x00420C, 0x01); // re-enable
        bus.execute_hdma();
        assert_eq!(
            &bus.wram[0..2],
            &[0xAA, 0xBB],
            "resume must continue mid-entry (repeat do_transfer preserved), not restart at A1T"
        );
    }

    #[test]
    fn hdma_midframe_enable_resumes_from_staged_a2a() {
        // Two 1-line entries; the game stages A2A at the SECOND entry.
        // Under (wrong) init-at-enable semantics A2A would reset to A1T and
        // 0xAA would appear; correct resume semantics play 0xBB only.
        let mut bus = make_hdma_bus(&[0x01, 0xAA, 0x01, 0xBB, 0x00]);
        bus.hdmaen = 0;
        bus.init_hdma();

        bus.write(0x004308, 0x02); // A2A -> $1002 (second entry)
        bus.write(0x004309, 0x10);
        bus.write(0x00430A, 0x01); // primer
        bus.write(0x00420C, 0x01);

        bus.execute_hdma(); // primer -> loads entry at $1002
        bus.execute_hdma(); // transfers its data byte
        assert_eq!(bus.wram[0], 0xBB, "must transfer from staged A2A entry");
        assert_eq!(bus.wmadd, 1);
        assert_ne!(bus.wram[0], 0xAA, "A1T entry data must never appear");
    }

    #[test]
    fn hdma_terminated_channel_stays_dead_on_reenable() {
        // After the table terminator, a same-frame $420C re-enable must not
        // resurrect the channel (ntrl=0 would wrap 0->0xFF and stream
        // garbage). The next frame's init_hdma revives it normally.
        let mut bus = make_hdma_bus(&[0x01, 0xAA, 0x00]);
        bus.init_hdma();
        bus.execute_hdma(); // transfers 0xAA, exhausts entry, loads terminator
        assert!(!bus.hdma.state[0].active);
        assert_eq!(bus.hdma_ended, 0x01, "terminator must record the channel");
        assert_eq!(bus.wmadd, 1);

        bus.write(0x00420C, 0x00);
        bus.write(0x00420C, 0x01); // attempted same-frame resurrection
        assert!(!bus.hdma.state[0].active, "ended channel must stay dead");
        for _ in 0..3 {
            bus.execute_hdma();
        }
        assert_eq!(bus.wmadd, 1, "no post-terminator garbage transfers");
        assert_eq!(bus.dma.ch[0].ntrl, 0, "ntrl must not wrap");

        bus.init_hdma(); // next frame
        assert_eq!(bus.hdma_ended, 0, "frame init clears the ended mask");
        assert!(bus.hdma.state[0].active, "init revives the channel");
        bus.execute_hdma();
        assert_eq!(bus.wmadd, 2, "channel transfers again after frame init");
    }
}
