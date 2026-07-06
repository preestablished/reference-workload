//! `Core` — the public emulator facade and frame scheduler
//! (ARCHITECTURE.md §3 "Core API sketch").
//!
//! OWNER (implementation): integration agent.

use crate::bus::{run_cpu_until, SysBus};
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::fault::{Fault, FrameFlags};
use crate::ppu::Ppu;
use crate::timing::{
    FB_BYTES, FIRST_VISIBLE_LINE, LAST_VISIBLE_LINE, LINES_PER_FRAME, MCLK_PER_FRAME,
    MCLK_PER_LINE, VBLANK_START_LINE,
};
use crate::WRAM_INIT_BYTE;

/// Construction-time errors. Runtime anomalies are [`Fault`]s, not errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// ROM length is not a non-zero multiple of 32 KiB.
    BadRomSize { len: usize },
    /// Emulation reset vector does not point into mapped ROM.
    BadResetVector { vector: u16 },
    /// Provided SRAM buffer has an unsupported length.
    BadSramSize { len: usize },
}

/// Externally-owned working buffers that double as published regions
/// (zero-copy publication, D7). The harness passes `mmap`-pinned buffers;
/// tests pass leaked boxes.
pub struct RegionBuffers {
    /// 128 KiB work RAM — the core's actual WRAM.
    pub wram: &'static mut [u8; 0x20000],
    /// 64 KiB video RAM (publish-optional; the core allocates internally
    /// when `None`).
    pub vram: Option<&'static mut [u8; 0x10000]>,
    /// Cartridge save RAM when the cart has it.
    pub sram: Option<&'static mut [u8]>,
}

/// The emulator core. All state lives in plain memory owned by this struct
/// (D5); everything is allocated in [`Core::new`] (D8).
pub struct Core {
    cpu: Cpu,
    bus: SysBus,
    frame: u64,
    /// Completed-frame front buffer (XRGB8888, 256×224, stride 1024).
    front: Box<[u8; FB_BYTES]>,
}

/// Clean-room-safe diagnostic snapshot (introspect-only). Every field is a
/// boolean, a count, or a hardware address/config value — nothing here can
/// reconstruct ROM code, audio-driver payload, graphics, or memory contents.
#[cfg(feature = "introspect")]
#[derive(Debug, Clone, Copy)]
pub struct DiagSnapshot {
    pub frame: u64,
    pub force_blank: bool,
    pub brightness: u8,
    pub bg_mode: u8,
    pub main_screen: u8,
    pub cgram_nz: usize,
    pub vram_nz: usize,
    pub oam_nz: usize,
    pub spc_pc: u16,
    pub spc_in_ipl: bool,
    pub nmi_enabled: bool,
    pub autojoy_enabled: bool,
    pub main_pc: u16,
    pub rd_4210: u64,
    pub rd_4211: u64,
    pub rd_4212: u64,
    pub rd_apu: u64,
    pub wr_apu: u64,
    pub wr_cc_port0: bool,
    pub spc_port0_is_cc: bool,
}

impl Core {
    /// Deterministic construction (D3): fills WRAM with the fixed init
    /// pattern, applies documented power-on register state, runs the CPU
    /// reset sequence. No I/O of any kind.
    pub fn new(cart: Cartridge, regions: RegionBuffers) -> Result<Core, CoreError> {
        // Fill WRAM with the deterministic init pattern (D3).
        regions.wram.fill(WRAM_INIT_BYTE);

        // VRAM: use provided buffer or allocate a zeroed 64 KiB box and leak it
        // (one-time allocation at construction, D8).
        let vram: &'static mut [u8; 0x10000] = match regions.vram {
            Some(v) => v,
            None => Box::leak(Box::new([0u8; 0x10000])),
        };

        let ppu = Ppu::new(vram);

        // Construct SysBus with power-on state.
        let mut bus = SysBus::new(regions.wram, cart, ppu);

        // Power-on: mclk_frame = 0, line = 0.
        bus.mclk_frame = 0;
        bus.line = 0;

        // Build CPU and run reset sequence (loads reset vector via bus).
        let mut cpu = Cpu::new();
        cpu.reset(&mut bus);

        // Front buffer (pre-zeroed, D8 — allocated here, never during frame).
        let front: Box<[u8; FB_BYTES]> = Box::new([0u8; FB_BYTES]);

        Ok(Core {
            cpu,
            bus,
            frame: 0,
            front,
        })
    }

    /// Run exactly one video frame with `pad` (platform bit order,
    /// API.md §3.4) latched for the whole frame (D6). Returns immediately
    /// with [`FrameFlags::FAULTED`] once a fault is recorded.
    pub fn run_one_frame(&mut self, pad: u16) -> FrameFlags {
        // Return immediately if already faulted.
        if self.bus.fault.is_some() {
            let mut flags = self.bus.frame_flags;
            flags.insert(FrameFlags::FAULTED);
            return flags;
        }

        // Latch pad for the whole frame (D6).
        self.bus.joypad.pad = pad & 0x0FFF;

        // Reset per-frame accumulator bits (keep FAULTED sticky if already set,
        // but per spec FAULTED returns early so we clear all here).
        self.bus.frame_flags = FrameFlags::default();

        // Carry-over from previous frame: subtract one frame's clocks to keep
        // mclk_frame as within-frame absolute, preserving CPU overshoot.
        // Deliberately a single subtraction, not a loop: an overrun larger
        // than a frame (e.g. a maximum-size DMA burst, ~1.5 frames) carries
        // forward and consumes the following frames' CPU budget — time is
        // conserved, the behavior is deterministic, and per-line events
        // (NMI flag, auto-joypad) still fire on schedule.
        if self.bus.mclk_frame >= MCLK_PER_FRAME {
            self.bus.mclk_frame -= MCLK_PER_FRAME;
        }

        // Main scanline loop.
        let mut faulted_early = false;
        for line in 0..LINES_PER_FRAME {
            // Per-line hooks (NMI, auto-joypad, IRQ reschedule).
            self.bus.start_line(line, pad);
            self.bus.ppu.set_line(line);

            // PPU frame/vblank hooks (separate from bus start_line to keep
            // bus unit-testable without a live PPU).
            if line == 0 {
                self.bus.ppu.begin_frame();
                // HDMA: initialize channel table pointers at start of frame
                // (line 0 = end of v-blank, documented init point).
                self.bus.init_hdma();
            } else if line == VBLANK_START_LINE {
                self.bus.ppu.begin_vblank();
            }

            // HDMA: apply per-scanline register writes before rendering the
            // line (writes land in h-blank before the visible raster begins).
            // HDMA runs on visible lines only (1..=224); v-blank HDMA writes
            // are skipped per the documented behavior — the channel state
            // still advances so table reload at line 0 is consistent.
            if (FIRST_VISIBLE_LINE..=LAST_VISIBLE_LINE).contains(&line) {
                self.bus.execute_hdma();
                if self.bus.fault.is_some() {
                    faulted_early = true;
                    break;
                }
            }

            // Run CPU until the end of this scanline.
            let target = (line as u64 + 1) * MCLK_PER_LINE;
            run_cpu_until(&mut self.cpu, &mut self.bus, target);

            if self.bus.fault.is_some() {
                faulted_early = true;
                break;
            }

            // APU catch-up at end of scanline (b): advance the APU to the
            // current master-clock boundary so it stays within one scanline
            // of the CPU's time view. This is the second catch-up point;
            // the first fires on every CPU access to $2140–$2143.
            self.bus.apu_catch_up();

            if self.bus.fault.is_some() {
                faulted_early = true;
                break;
            }

            // Render visible scanlines.
            if (FIRST_VISIBLE_LINE..=LAST_VISIBLE_LINE).contains(&line) {
                self.bus.ppu.render_scanline(line);
            }

            if self.bus.fault.is_some() {
                faulted_early = true;
                break;
            }
        }

        if faulted_early || self.bus.fault.is_some() {
            self.bus.frame_flags.insert(FrameFlags::FAULTED);
            return self.bus.frame_flags;
        }

        // Clean frame end: blit back buffer to front buffer and increment counter.
        self.front.copy_from_slice(&*self.bus.ppu.back);
        self.frame += 1;

        self.bus.frame_flags
    }

    /// Copy the completed frame into the published framebuffer (D7: the
    /// host never sees a torn frame).
    pub fn blit_completed_frame(&self, dst: &mut [u8; FB_BYTES]) {
        dst.copy_from_slice(&*self.front);
    }

    /// Last completed frame number (0 before the first frame completes).
    pub fn frame_counter(&self) -> u64 {
        self.frame
    }

    /// The fault that halted the core, if any (D9).
    pub fn fault(&self) -> Option<Fault> {
        self.bus.fault
    }

    /// Read access to the published `wram` region (the core's working WRAM,
    /// D7). The harness publishes the buffer it owns; host-side tools use
    /// this accessor to compute frame hashes (`blake3(wram ‖ framebuffer)`).
    pub fn wram(&self) -> &[u8; 0x20000] {
        self.bus.wram
    }

    /// TEST-ONLY: side-effect-free bus read for `ramdiff` and golden-trace
    /// tests. Returns 0 for unmapped/side-effectful addresses (I/O space is
    /// defined to peek as 0 — tools cannot distinguish that from a real zero
    /// byte; only WRAM/ROM/SRAM peeks are meaningful).
    #[cfg(feature = "introspect")]
    pub fn debug_peek(&self, bus_addr: u32) -> u8 {
        self.bus.peek(bus_addr).unwrap_or(0)
    }

    /// Clean-room-safe diagnostic snapshot (introspect-only, compiled out of the
    /// guest binary). Emits booleans, counts, and hardware addresses/config —
    /// never ROM bytes, framebuffer pixels, memory contents, or APU/DMA payload.
    #[cfg(feature = "introspect")]
    pub fn diag_snapshot(&self) -> DiagSnapshot {
        let (force_blank, brightness, bg_mode, main_screen) = self.bus.ppu.diag();
        let (cgram_nz, vram_nz, oam_nz) = self.bus.ppu.diag_nonzero_counts();
        let spc_pc = self.bus.apu.cpu.pc;
        let nmitimen = self.bus.nmitimen;
        DiagSnapshot {
            frame: self.frame,
            force_blank,
            brightness,
            bg_mode,
            main_screen,
            cgram_nz,
            vram_nz,
            oam_nz,
            spc_pc,
            // SPC still executing the 64-byte IPL boot ROM ($FFC0-$FFFF) means the
            // audio-driver upload has not handed off to game code yet.
            spc_in_ipl: spc_pc >= 0xFFC0,
            nmi_enabled: (nmitimen & 0x80) != 0,
            autojoy_enabled: (nmitimen & 0x01) != 0,
            main_pc: self.cpu.pc,
            rd_4210: self.bus.diag_rd_4210,
            rd_4211: self.bus.diag_rd_4211,
            rd_4212: self.bus.diag_rd_4212,
            rd_apu: self.bus.diag_rd_apu,
            wr_apu: self.bus.diag_wr_apu,
            wr_cc_port0: self.bus.diag_wr_cc_port0,
            // Is the IPL kick constant currently sitting in the SPC-visible port-0
            // latch? (known protocol constant, not game data.)
            spc_port0_is_cc: self.bus.apu.spc_ports[0] == 0xCC,
        }
    }
}
