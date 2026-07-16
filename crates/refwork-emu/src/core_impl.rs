//! `Core` — the public emulator facade and frame scheduler
//! (ARCHITECTURE.md §3 "Core API sketch").
//!
//! OWNER (implementation): integration agent.

use crate::bus::{run_cpu_until, SysBus};
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::fault::{Fault, FrameFlags};
use crate::ppu::Ppu;
use crate::timing::{FB_BYTES, FIRST_VISIBLE_LINE, LINES_PER_FRAME, MCLK_PER_FRAME, MCLK_PER_LINE};
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
    pub sub_screen: u8,
    pub main_window: u8,
    pub cgwsel: u8,
    pub cgadsub: u8,
    pub fixed_color: u16,
    pub cgram_nz: usize,
    pub vram_nz: usize,
    pub oam_nz: usize,
    pub cgram_distinct_colors: usize,
    pub frame_final_nonzero: u64,
    pub frame_main_nonbackdrop: u64,
    pub frame_main_color_nonzero: u64,
    pub frame_main_clipped: u64,
    pub frame_math_applied: u64,
    pub frame_mode1_bg_opaque: [u64; 3],
    pub frame_mode1_selected: [u64; 5],
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
    pub post_cc_port0_writes: u64,
    pub first_post_cc_port0_delta_mclk: Option<u64>,
    pub apu_port0_service_count: u64,
    pub first_cc_service_spc_pc: Option<u16>,
    pub spc_port_writes: [u64; 4],
    pub spc_port_reads: [u64; 4],
    pub spc_port_last_write_pc: [Option<u16>; 4],
    pub spc_port_last_read_pc: [Option<u16>; 4],
    pub spc_io_writes: [u64; 16],
    pub spc_io_reads: [u64; 16],
    pub spc_io_last_write_pc: [Option<u16>; 16],
    pub spc_io_last_read_pc: [Option<u16>; 16],
    pub spc_recent_pcs: [u16; 16],
    pub spc_recent_pc_pos: usize,
    pub spc_step_count: u64,
    pub spc_first_pcs: [u16; 16],
    pub spc_first_pc_count: usize,
    pub ipl_first_load_addr: Option<u16>,
    pub ipl_last_load_addr: Option<u16>,
    pub ipl_jump_addr: Option<u16>,
    pub ipl_bytes_stored: u64,
    pub ipl_block_count: u64,
    pub ipl_block_addrs: [Option<u16>; 8],
    pub ipl_block_bytes: [u64; 8],
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
            if line == 0 {
                // Latch SETINI display timing before the bus derives this
                // frame's vblank/NMI/auto-joy boundary.
                self.bus.ppu.begin_frame();
            }
            // Per-line hooks (NMI, auto-joypad, IRQ reschedule).
            self.bus.start_line(line, pad);
            self.bus.ppu.set_line(line);

            // PPU frame/vblank hooks (separate from bus start_line to keep
            // bus unit-testable without a live PPU).
            if line == 0 {
                // HDMA: initialize channel table pointers at start of frame
                // (line 0 = end of v-blank, documented init point).
                self.bus.init_hdma();
            } else if line == self.bus.ppu.vblank_start_line() {
                self.bus.ppu.begin_vblank();
            }

            // HDMA: apply per-scanline register writes before rendering the
            // line (writes land in h-blank before the visible raster begins).
            // HDMA runs on visible lines only (through 224 or overscan line 239); vblank writes
            // are skipped per the documented behavior — the channel state
            // still advances so table reload at line 0 is consistent.
            if line >= FIRST_VISIBLE_LINE && line < self.bus.ppu.vblank_start_line() {
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
            if line >= FIRST_VISIBLE_LINE && line < self.bus.ppu.vblank_start_line() {
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

    /// Drain stereo i16 samples (interleaved L,R) synthesized since the last
    /// call. Native rate: 32000 Hz nominal (1 sample / 32 SPC cycles,
    /// [`crate::AUDIO_SAMPLE_RATE_HZ`], derived from `apu::DSP_CLOCKS_PER_SAMPLE`);
    /// ~532 pairs per 60 fps frame. Returns the number of `i16` values
    /// written to `out`, always even; samples beyond `out`'s capacity (or
    /// beyond what has been produced) remain queued for the next call.
    /// Capture-only — this is a host-frontend affordance and never affects
    /// emulation, frame hashes, or determinism (the S-DSP stream is already
    /// computed every frame; this only taps the value that used to be
    /// discarded).
    #[cfg(feature = "audio")]
    pub fn take_audio_samples(&mut self, out: &mut [i16]) -> usize {
        self.bus.apu.drain_audio(out)
    }

    /// Count of stereo pairs discarded by capture-ring overflow
    /// (overwrite-oldest, e.g. because the host frontend fell behind or
    /// never drains) since construction. Never decreases. Intended for a
    /// frontend's shutdown diagnostics.
    #[cfg(feature = "audio")]
    pub fn audio_dropped_pairs(&self) -> u64 {
        self.bus.apu.audio_dropped_pairs()
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
        let (sub_screen, main_window, cgwsel, cgadsub, fixed_color) =
            self.bus.ppu.diag_compositor();
        let (cgram_nz, vram_nz, oam_nz) = self.bus.ppu.diag_nonzero_counts();
        let cgram_distinct_colors = self.bus.ppu.diag_distinct_cgram_colors();
        let (
            frame_final_nonzero,
            frame_main_nonbackdrop,
            frame_main_color_nonzero,
            frame_main_clipped,
            frame_math_applied,
            frame_mode1_bg_opaque,
            frame_mode1_selected,
        ) = self.bus.ppu.diag_render_counts();
        let spc_pc = self.bus.apu.cpu.pc;
        let nmitimen = self.bus.nmitimen;
        DiagSnapshot {
            frame: self.frame,
            force_blank,
            brightness,
            bg_mode,
            main_screen,
            sub_screen,
            main_window,
            cgwsel,
            cgadsub,
            fixed_color,
            cgram_nz,
            vram_nz,
            oam_nz,
            cgram_distinct_colors,
            frame_final_nonzero,
            frame_main_nonbackdrop,
            frame_main_color_nonzero,
            frame_main_clipped,
            frame_math_applied,
            frame_mode1_bg_opaque,
            frame_mode1_selected,
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
            post_cc_port0_writes: self.bus.diag_post_cc_port0_writes,
            first_post_cc_port0_delta_mclk: self.bus.diag_first_post_cc_port0_delta_mclk,
            apu_port0_service_count: self.bus.diag_apu_port0_service_count,
            first_cc_service_spc_pc: self.bus.diag_first_cc_service_spc_pc,
            spc_port_writes: self.bus.apu.diag_spc_port_writes,
            spc_port_reads: self.bus.apu.diag_spc_port_reads,
            spc_port_last_write_pc: self.bus.apu.diag_spc_port_last_write_pc,
            spc_port_last_read_pc: self.bus.apu.diag_spc_port_last_read_pc,
            spc_io_writes: self.bus.apu.diag_spc_io_writes,
            spc_io_reads: self.bus.apu.diag_spc_io_reads,
            spc_io_last_write_pc: self.bus.apu.diag_spc_io_last_write_pc,
            spc_io_last_read_pc: self.bus.apu.diag_spc_io_last_read_pc,
            spc_recent_pcs: self.bus.apu.diag_spc_recent_pcs,
            spc_recent_pc_pos: self.bus.apu.diag_spc_recent_pc_pos,
            spc_step_count: self.bus.apu.diag_spc_step_count,
            spc_first_pcs: self.bus.apu.diag_spc_first_pcs,
            spc_first_pc_count: self.bus.apu.diag_spc_first_pc_count,
            ipl_first_load_addr: self.bus.apu.diag_ipl_first_load_addr,
            ipl_last_load_addr: self.bus.apu.diag_ipl_last_load_addr,
            ipl_jump_addr: self.bus.apu.diag_ipl_jump_addr,
            ipl_bytes_stored: self.bus.apu.diag_ipl_bytes_stored,
            ipl_block_count: self.bus.apu.diag_ipl_block_count,
            ipl_block_addrs: self.bus.apu.diag_ipl_block_addrs,
            ipl_block_bytes: self.bus.apu.diag_ipl_block_bytes,
            // Is the IPL kick constant currently sitting in the SPC-visible port-0
            // latch? (known protocol constant, not game data.)
            spc_port0_is_cc: self.bus.apu.spc_ports[0] == 0xCC,
        }
    }
}
