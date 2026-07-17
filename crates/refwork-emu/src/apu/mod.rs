//! APU module: SPC700 CPU, ARAM, timers, DSP, IPL ROM overlay, and I/O ports.
//!
//! ## M2 `Apu` struct
//!
//! Owns: SPC700 CPU registers, 64 KiB ARAM, three hardware timers, four I/O
//! ports (SPC-side $F4–$F7; CPU-side `cpu_read_port` / `cpu_write_port`
//! methods expose them to the bus), DSP address/data ports ($F2/$F3) backed
//! by the real 8-voice DSP, control register $F1, test register $F0, and the
//! IPL ROM overlay at $FFC0–$FFFF.
//!
//! ## Clock model (deterministic, integer-only)
//!
//! The SPC700 runs at a nominal 1.024 MHz. The 65C816 master clock runs at
//! 21.477272 MHz. The integer ratio used here is:
//!
//!   **SPC_RATIO = 1024 / 21477**
//!
//! Accumulator model: maintain a `u64` SPC-cycle accumulator. On each
//! `advance_master_cycles(n)` call:
//!
//!   `spc_accum += n * SPC_NUM`
//!
//! where SPC_NUM = 1024, SPC_DEN = 21477. Drain whole SPC cycles:
//!
//!   `spc_cycles_to_run = spc_accum / SPC_DEN`
//!   `spc_accum %= SPC_DEN`
//!
//! The DSP is clocked 1 sample per 32 SPC cycles from the same accumulator.
//!
//! **Why this ratio?** The SPC700 datasheet specifies 1.024 MHz; the NTSC
//! master clock is 315/88 MHz ≈ 21.47727 MHz. The exact integer fraction
//! that minimises drift while staying in integer arithmetic: 1024 / 21477
//! (where 21477 ≈ 21.477 × 1000). Over one NTSC frame (357,368 master
//! clocks) this yields ≈ 17,038.9 SPC cycles (357,368 × 1024/21477), the
//! documented relationship.
//!
//! ## Memory map
//!
//! | Range         | Description                                         |
//! |---------------|-----------------------------------------------------|
//! | $0000–$00EF   | Direct-page RAM (page 0)                            |
//! | $00F0–$00FF   | I/O registers (timers/ports/control/DSP/test)       |
//! | $0100–$FFBF   | General-purpose RAM                                 |
//! | $FFC0–$FFFF   | IPL ROM (if $F1 bit 7 set) else RAM                 |
//!
//! Writes to $FFC0–$FFFF always land in underlying RAM; reads return the IPL
//! ROM bytes when the enable bit is set.

pub mod aram;
pub mod dsp;
pub mod ipl;
pub mod spc700;
pub mod timers;

use aram::Aram;
use dsp::Dsp;
use ipl::IPL_ROM;
use spc700::{ApuHalt, Spc700};
use timers::Timer;

/// Numerator for the SPC700 / master-clock ratio (integer accumulator model).
///
/// SPC_NUM / SPC_DEN ≈ 1.024 MHz / 21.477 MHz ≈ 1/20.97.
/// Using 1024 / 21477 keeps the ratio exact enough that over one NTSC frame
/// (357,368 master clocks) we drain ≈ 17,039 SPC cycles.
pub const SPC_NUM: u64 = 1024;

/// Denominator for the SPC700 / master-clock ratio (integer accumulator model).
pub const SPC_DEN: u64 = 21477;

/// SPC700 cycles per DSP sample.
pub const DSP_CLOCKS_PER_SAMPLE: u64 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IplHleState {
    WaitCommand,
    ReceiveBlock {
        ptr: u16,
        last_signal: u8,
        expected: u8,
    },
    Done,
}

// ─── M2 Apu struct ───────────────────────────────────────────────────────────

/// Full APU (M2): SPC700 CPU + ARAM + timers + DSP + I/O ports + IPL ROM.
///
/// Package 02 wires this into the bus by calling `cpu_read_port` /
/// `cpu_write_port` from the main CPU's bus handlers for $2140–$2143.
/// The SPC700 is stepped via `advance_master_cycles` which applies the
/// integer accumulator timing model.
///
/// For the single-step corpus runner, use `Apu::new_corpus` (only available
/// under `cfg(feature = "introspect")`) which puts the APU in flat-RAM mode.
pub struct Apu {
    pub cpu: Spc700,
    pub aram: Aram,
    /// Three hardware timers (indices 0, 1, 2).
    pub timers: [Timer; 3],
    /// Four I/O ports as seen by the SPC700 ($F4–$F7).
    /// Writes from the main CPU land here; the SPC700 reads/writes them too.
    pub spc_ports: [u8; 4],
    /// Four I/O ports as seen by the main CPU ($2140–$2143).
    pub cpu_ports: [u8; 4],
    /// Control register $F1 shadow.
    pub ctrl: u8,
    /// Real 8-voice DSP.
    pub dsp: Dsp,
    /// DSP address register (written via $F2, used as index into the DSP register file).
    dsp_addr: u8,
    /// Halt state, if the SPC700 stopped via SLEEP/STOP/test.
    pub halted: Option<ApuHalt>,
    ipl_hle: IplHleState,
    #[cfg(feature = "introspect")]
    pub diag_spc_port_writes: [u64; 4],
    #[cfg(feature = "introspect")]
    pub diag_spc_port_reads: [u64; 4],
    #[cfg(feature = "introspect")]
    pub diag_spc_port_last_write_pc: [Option<u16>; 4],
    #[cfg(feature = "introspect")]
    pub diag_spc_port_last_read_pc: [Option<u16>; 4],
    #[cfg(feature = "introspect")]
    pub diag_spc_io_writes: [u64; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_io_reads: [u64; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_io_last_write_pc: [Option<u16>; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_io_last_read_pc: [Option<u16>; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_recent_pcs: [u16; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_recent_pc_pos: usize,
    #[cfg(feature = "introspect")]
    pub diag_spc_step_count: u64,
    #[cfg(feature = "introspect")]
    pub diag_spc_first_pcs: [u16; 16],
    #[cfg(feature = "introspect")]
    pub diag_spc_first_pc_count: usize,
    #[cfg(feature = "introspect")]
    pub diag_ipl_first_load_addr: Option<u16>,
    #[cfg(feature = "introspect")]
    pub diag_ipl_last_load_addr: Option<u16>,
    #[cfg(feature = "introspect")]
    pub diag_ipl_jump_addr: Option<u16>,
    #[cfg(feature = "introspect")]
    pub diag_ipl_bytes_stored: u64,
    #[cfg(feature = "introspect")]
    pub diag_ipl_block_count: u64,
    #[cfg(feature = "introspect")]
    pub diag_ipl_block_addrs: [Option<u16>; 8],
    #[cfg(feature = "introspect")]
    pub diag_ipl_block_bytes: [u64; 8],

    /// SPC-cycle accumulator for the integer timing model.
    /// Holds fractional SPC cycles (0..SPC_DEN).
    spc_accum: u64,
    /// SPC cycles executed beyond a previous call's budget (instruction
    /// overshoot), repaid by shrinking future budgets. Bounded by the largest
    /// single step() return (36, the IPL-HLE command step).
    spc_debt: u64,
    /// DSP sample sub-cycle counter: counts SPC cycles until next sample.
    dsp_cycle_accum: u64,
}

impl Default for Apu {
    fn default() -> Self {
        Apu::new()
    }
}

impl Apu {
    /// Power-on state: IPL ROM enabled, timers disabled.
    pub fn new() -> Self {
        let mut aram = Aram::new();
        // Shadow the IPL ROM into the flat ARAM so that `cpu.execute()` —
        // which reads operand bytes via the raw slice — sees the correct IPL
        // bytes at $FFC0–$FFFF even though `mem_read` has an overlay there.
        // The overlay remains in place for production `mem_read` calls; this
        // just ensures the flat-ARAM path also executes correctly.
        for (i, &b) in IPL_ROM.iter().enumerate() {
            aram.write(0xFFC0 + i as u16, b);
        }
        // Build DSP with power-on state, then disable echo writes so the echo
        // buffer (ESA=0 by default → ARAM[$0000+]) cannot corrupt the IPL's
        // transfer-pointer cells at ARAM[$0002:$0003] during SPC program
        // upload.  FLG bit 5 = ECEN (echo write disable); setting it to $20
        // leaves all other FLG bits clear (no RESET, no MUTE, noise rate 0).
        let mut dsp = Dsp::new();
        dsp.write_reg(0x6C, 0x20); // FLG: echo write disable

        Apu {
            cpu: Spc700::new(),
            aram,
            timers: [
                Timer::new(timers::DIVIDER_01),
                Timer::new(timers::DIVIDER_01),
                Timer::new(timers::DIVIDER_2),
            ],
            spc_ports: [0; 4],
            cpu_ports: [0xAA, 0xBB, 0, 0], // ready signature at power-on
            ctrl: 0x80,                    // IPL ROM enabled by default
            dsp,
            dsp_addr: 0,
            halted: None,
            ipl_hle: IplHleState::WaitCommand,
            #[cfg(feature = "introspect")]
            diag_spc_port_writes: [0; 4],
            #[cfg(feature = "introspect")]
            diag_spc_port_reads: [0; 4],
            #[cfg(feature = "introspect")]
            diag_spc_port_last_write_pc: [None; 4],
            #[cfg(feature = "introspect")]
            diag_spc_port_last_read_pc: [None; 4],
            #[cfg(feature = "introspect")]
            diag_spc_io_writes: [0; 16],
            #[cfg(feature = "introspect")]
            diag_spc_io_reads: [0; 16],
            #[cfg(feature = "introspect")]
            diag_spc_io_last_write_pc: [None; 16],
            #[cfg(feature = "introspect")]
            diag_spc_io_last_read_pc: [None; 16],
            #[cfg(feature = "introspect")]
            diag_spc_recent_pcs: [0; 16],
            #[cfg(feature = "introspect")]
            diag_spc_recent_pc_pos: 0,
            #[cfg(feature = "introspect")]
            diag_spc_step_count: 0,
            #[cfg(feature = "introspect")]
            diag_spc_first_pcs: [0; 16],
            #[cfg(feature = "introspect")]
            diag_spc_first_pc_count: 0,
            #[cfg(feature = "introspect")]
            diag_ipl_first_load_addr: None,
            #[cfg(feature = "introspect")]
            diag_ipl_last_load_addr: None,
            #[cfg(feature = "introspect")]
            diag_ipl_jump_addr: None,
            #[cfg(feature = "introspect")]
            diag_ipl_bytes_stored: 0,
            #[cfg(feature = "introspect")]
            diag_ipl_block_count: 0,
            #[cfg(feature = "introspect")]
            diag_ipl_block_addrs: [None; 8],
            #[cfg(feature = "introspect")]
            diag_ipl_block_bytes: [0; 8],
            spc_accum: 0,
            spc_debt: 0,
            dsp_cycle_accum: 0,
        }
    }

    /// Corpus-mode APU: flat-RAM semantics, no I/O overlay. Only available
    /// under `introspect` feature (corpus runner uses it directly).
    #[cfg(feature = "introspect")]
    pub fn new_corpus() -> Self {
        let mut a = Self::new();
        a.cpu = Spc700::new_corpus();
        a
    }

    // ---- IPL ROM control ----

    /// True when the IPL ROM is mapped over $FFC0–$FFFF (bit 7 of $F1).
    #[inline]
    pub fn ipl_enabled(&self) -> bool {
        self.ctrl & 0x80 != 0
    }

    // ---- Memory access (SPC700 view) ----

    /// Read a byte from the SPC700's address space, applying I/O and IPL
    /// overlays.
    pub fn mem_read(&mut self, addr: u16) -> u8 {
        if self.cpu.corpus_mode {
            return self.aram.read(addr);
        }
        match addr {
            // IPL ROM overlay.
            0xFFC0..=0xFFFF if self.ipl_enabled() => IPL_ROM[(addr - 0xFFC0) as usize],
            // I/O register range.
            0x00F0..=0x00FF => self.io_read(addr as u8),
            _ => self.aram.read(addr),
        }
    }

    /// Write a byte to the SPC700's address space.
    #[cfg_attr(not(any(test, feature = "introspect")), allow(dead_code))]
    pub fn mem_write(&mut self, addr: u16, value: u8) {
        if self.cpu.corpus_mode {
            self.aram.write(addr, value);
            return;
        }
        // Writes always land in ARAM, then we handle side-effects for I/O.
        self.aram.write(addr, value);
        if (0x00F0..=0x00FF).contains(&addr) {
            self.io_write(addr as u8, value);
        }
    }

    // ---- I/O register read ($F0–$FF) ----

    fn io_read(&mut self, reg: u8) -> u8 {
        match reg {
            // $F0: test register (write-only semantics; reads return ARAM)
            0xF0 => self.aram.read(0x00F0),
            // $F1: control (write-only; reads undefined — return 0)
            0xF1 => 0,
            // $F2: DSP address register
            0xF2 => self.dsp_addr,
            // $F3: DSP data register
            0xF3 => self.dsp.read_reg(self.dsp_addr),
            // $F4–$F7: I/O ports (SPC side reads what main CPU wrote)
            0xF4 => self.spc_ports[0],
            0xF5 => self.spc_ports[1],
            0xF6 => self.spc_ports[2],
            0xF7 => self.spc_ports[3],
            // $F8–$F9: extra RAM bytes (no special function in baseline)
            0xF8 | 0xF9 => self.aram.read(reg as u16),
            // $FA–$FC: timer target registers (write-only; reads return ARAM)
            0xFA..=0xFC => self.aram.read(reg as u16),
            // $FD–$FF: timer output counters — read and clear
            0xFD => self.timers[0].read_output(),
            0xFE => self.timers[1].read_output(),
            0xFF => self.timers[2].read_output(),
            _ => self.aram.read(reg as u16),
        }
    }

    // ---- I/O register write ($F0–$FF) ----

    fn io_write(&mut self, reg: u8, value: u8) {
        match reg {
            // $F0: test register — nonzero value triggers halt
            0xF0 => {
                if value != 0 {
                    self.halted = Some(ApuHalt::TestTrigger(value));
                    self.cpu.halted = self.halted;
                }
            }
            // $F1: control register
            0xF1 => {
                // Bits 0/1/2: timer enables (edge-detected in Timer::set_enabled)
                self.timers[0].set_enabled(value & 0x01 != 0);
                self.timers[1].set_enabled(value & 0x02 != 0);
                self.timers[2].set_enabled(value & 0x04 != 0);
                // Bits 4/5: clear port pairs
                if value & 0x10 != 0 {
                    self.spc_ports[0] = 0;
                    self.spc_ports[1] = 0;
                }
                if value & 0x20 != 0 {
                    self.spc_ports[2] = 0;
                    self.spc_ports[3] = 0;
                }
                // Bit 7: IPL ROM enable (simply store it)
                self.ctrl = value;
            }
            // $F2: DSP address register
            0xF2 => {
                self.dsp_addr = value;
            }
            // $F3: DSP data register
            0xF3 => {
                self.dsp.write_reg(self.dsp_addr, value);
            }
            // $F4–$F7: SPC→CPU output ports
            0xF4 => {
                self.cpu_ports[0] = value;
            }
            0xF5 => {
                self.cpu_ports[1] = value;
            }
            0xF6 => {
                self.cpu_ports[2] = value;
            }
            0xF7 => {
                self.cpu_ports[3] = value;
            }
            // $FA–$FC: timer target registers
            0xFA => self.timers[0].write_target(value),
            0xFB => self.timers[1].write_target(value),
            0xFC => self.timers[2].write_target(value),
            // $FD–$FF: timer outputs are read-only; writes ignored
            0xFD..=0xFF => {}
            _ => {}
        }
    }

    // ---- Main-CPU↔APU port interface ----

    /// Main CPU reads a port ($2140+idx, idx 0..=3). Returns what the SPC700
    /// last wrote to the corresponding output register ($F4+idx).
    pub fn cpu_read_port(&self, idx: u8) -> u8 {
        self.cpu_ports[idx as usize & 3]
    }

    /// Main CPU writes a port ($2140+idx). Stores to the SPC-visible input
    /// register ($F4+idx).
    pub fn cpu_write_port(&mut self, idx: u8, value: u8) {
        self.spc_ports[idx as usize & 3] = value;
    }

    // ---- Host-frontend audio capture (feature "audio" only) ----
    //
    // Delegates to the DSP (`self.dsp` is already `pub`, so callers could
    // reach through it directly, but a passthrough here matches this
    // struct's existing layering — everything else on `Apu` that exposes
    // DSP behavior goes through a method, not a bare field reach-through).

    /// Move up to `out.len() / 2` pending stereo pairs into `out`
    /// (interleaved L,R). Returns the number of `i16` values written,
    /// always even. See [`dsp::Dsp::drain_audio`] for the interleaving and
    /// overflow policy.
    #[cfg(feature = "audio")]
    pub fn drain_audio(&mut self, out: &mut [i16]) -> usize {
        self.dsp.drain_audio(out)
    }

    /// Count of stereo pairs discarded by capture-ring overflow
    /// (overwrite-oldest) since construction. Never decreases.
    #[cfg(feature = "audio")]
    pub fn audio_dropped_pairs(&self) -> u64 {
        self.dsp.audio_dropped_pairs()
    }

    #[inline]
    fn should_run_ipl_hle(&self) -> bool {
        !self.cpu.corpus_mode
            && self.ipl_enabled()
            && self.cpu.pc >= 0xFFC0
            && self.ipl_hle != IplHleState::Done
    }

    fn ipl_command_addr(&self) -> u16 {
        (self.spc_ports[2] as u16) | ((self.spc_ports[3] as u16) << 8)
    }

    fn set_ipl_pointer(&mut self, ptr: u16) {
        self.aram.write(0x0000, ptr as u8);
        self.aram.write(0x0001, (ptr >> 8) as u8);
    }

    fn finish_ipl_jump(&mut self, addr: u16, command: u8) {
        self.set_ipl_pointer(addr);
        self.cpu.a = command;
        self.cpu.x = command;
        self.cpu.y = command;
        self.cpu.psw = (self.cpu.psw & !(spc700::psw::N | spc700::psw::Z))
            | if command == 0 {
                spc700::psw::Z
            } else {
                command & spc700::psw::N
            };
        self.cpu.pc = addr;
        self.ipl_hle = IplHleState::Done;
    }

    #[cfg(feature = "introspect")]
    fn record_ipl_block_start(&mut self, addr: u16) {
        let idx = self.diag_ipl_block_count as usize;
        if idx < self.diag_ipl_block_addrs.len() {
            self.diag_ipl_block_addrs[idx] = Some(addr);
        }
        self.diag_ipl_block_count += 1;
    }

    #[cfg(feature = "introspect")]
    fn record_ipl_byte_stored(&mut self) {
        self.diag_ipl_bytes_stored += 1;
        let idx = self.diag_ipl_block_count.saturating_sub(1) as usize;
        if idx < self.diag_ipl_block_bytes.len() {
            self.diag_ipl_block_bytes[idx] += 1;
        }
    }

    /// Clean-room implementation of the public SPC IPL upload protocol.
    ///
    /// The byte ROM remains available for corpus tests, but production uses this
    /// observable state machine so commercial uploaders get zero-based byte
    /// indices, multi-block termination, and command-0 jump semantics.
    fn step_ipl_hle(&mut self) -> u32 {
        match self.ipl_hle {
            IplHleState::WaitCommand => {
                self.cpu.pc = 0xFFCC;
                if self.spc_ports[0] != 0xCC {
                    return 7;
                }

                let command = self.spc_ports[1];
                let addr = self.ipl_command_addr();
                self.cpu_ports[0] = 0xCC;
                #[cfg(feature = "introspect")]
                {
                    self.diag_spc_port_writes[0] += 1;
                    self.diag_spc_port_last_write_pc[0] = Some(self.cpu.pc);
                    self.diag_spc_io_writes[4] += 1;
                    self.diag_spc_io_last_write_pc[4] = Some(self.cpu.pc);
                }
                if command == 0 {
                    #[cfg(feature = "introspect")]
                    {
                        self.diag_ipl_jump_addr = Some(addr);
                    }
                    self.finish_ipl_jump(addr, command);
                } else {
                    #[cfg(feature = "introspect")]
                    {
                        if self.diag_ipl_first_load_addr.is_none() {
                            self.diag_ipl_first_load_addr = Some(addr);
                        }
                        self.diag_ipl_last_load_addr = Some(addr);
                        self.record_ipl_block_start(addr);
                    }
                    self.set_ipl_pointer(addr);
                    self.cpu.y = 0;
                    self.cpu.pc = 0xFFE0;
                    self.ipl_hle = IplHleState::ReceiveBlock {
                        ptr: addr,
                        last_signal: 0xCC,
                        expected: 0,
                    };
                }
                36
            }
            IplHleState::ReceiveBlock {
                mut ptr,
                mut last_signal,
                mut expected,
            } => {
                self.cpu.pc = 0xFFE2;
                let signal = self.spc_ports[0];
                if signal == last_signal {
                    return 7;
                }

                if signal == expected {
                    let data = self.spc_ports[1];
                    self.mem_write(ptr, data);
                    #[cfg(feature = "introspect")]
                    {
                        self.record_ipl_byte_stored();
                    }
                    ptr = ptr.wrapping_add(1);
                    self.set_ipl_pointer(ptr);
                    self.cpu_ports[0] = signal;
                    #[cfg(feature = "introspect")]
                    {
                        self.diag_spc_port_writes[0] += 1;
                        self.diag_spc_port_last_write_pc[0] = Some(self.cpu.pc);
                        self.diag_spc_io_writes[4] += 1;
                        self.diag_spc_io_last_write_pc[4] = Some(self.cpu.pc);
                    }
                    last_signal = signal;
                    expected = expected.wrapping_add(1);
                    self.cpu.y = expected;
                    self.cpu.pc = 0xFFE0;
                    self.ipl_hle = IplHleState::ReceiveBlock {
                        ptr,
                        last_signal,
                        expected,
                    };
                    return 18;
                }

                let command = self.spc_ports[1];
                let addr = self.ipl_command_addr();
                self.cpu_ports[0] = signal;
                #[cfg(feature = "introspect")]
                {
                    self.diag_spc_port_writes[0] += 1;
                    self.diag_spc_port_last_write_pc[0] = Some(self.cpu.pc);
                    self.diag_spc_io_writes[4] += 1;
                    self.diag_spc_io_last_write_pc[4] = Some(self.cpu.pc);
                }
                if command == 0 {
                    #[cfg(feature = "introspect")]
                    {
                        self.diag_ipl_jump_addr = Some(addr);
                    }
                    self.finish_ipl_jump(addr, command);
                } else {
                    #[cfg(feature = "introspect")]
                    {
                        self.diag_ipl_last_load_addr = Some(addr);
                        self.record_ipl_block_start(addr);
                    }
                    self.set_ipl_pointer(addr);
                    self.cpu.y = 0;
                    self.cpu.pc = 0xFFE0;
                    self.ipl_hle = IplHleState::ReceiveBlock {
                        ptr: addr,
                        last_signal: signal,
                        expected: 0,
                    };
                }
                24
            }
            IplHleState::Done => 0,
        }
    }

    // ---- Stepping ----

    /// Step the SPC700 core once (corpus/test mode: caller passes a flat
    /// memory slice). Returns cycle count.
    #[cfg(feature = "introspect")]
    pub fn step_corpus(&mut self, mem: &mut [u8; 0x10000]) -> u32 {
        self.cpu.step(mem)
    }

    /// Step the SPC700 core one instruction against the APU's own ARAM (with
    /// I/O and IPL overlays active). Returns SPC700 cycle count.
    ///
    /// This is the production path: the SPC700 accesses memory through
    /// the APU's own ARAM with the I/O overlay applied. We implement this
    /// by using a trampoline: read/write through mem_read/mem_write.
    pub fn step(&mut self) -> u32 {
        if self.halted.is_some() {
            return 0;
        }
        if self.should_run_ipl_hle() {
            return self.step_ipl_hle();
        }

        // Fetch opcode with I/O overlay applied.
        let pc = self.cpu.pc;
        #[cfg(feature = "introspect")]
        {
            let idx = self.diag_spc_recent_pc_pos & (self.diag_spc_recent_pcs.len() - 1);
            self.diag_spc_recent_pcs[idx] = pc;
            self.diag_spc_recent_pc_pos = self.diag_spc_recent_pc_pos.wrapping_add(1);
            self.diag_spc_step_count = self.diag_spc_step_count.wrapping_add(1);
            if self.diag_spc_first_pc_count < self.diag_spc_first_pcs.len() {
                self.diag_spc_first_pcs[self.diag_spc_first_pc_count] = pc;
                self.diag_spc_first_pc_count += 1;
            }
        }
        let opcode = self.mem_read(pc);
        self.cpu.pc = pc.wrapping_add(1);

        // Execution model: the SPC700 core addresses a flat 64 KiB slice, so
        // the bidirectional I/O page ($F0-$FF) is emulated around each
        // instruction with an explicit sync contract:
        //
        //   sync-in  — stage every readable I/O value into ARAM so flat
        //              reads observe live state: $F4-$F7 = main-CPU-written
        //              ports, $F2 = DSP address, $F3 = current DSP register,
        //              $F1 = control shadow, $FD-$FF = live timer outputs.
        //   execute  — the core records exact $F0-$FF accesses in
        //              `io_written_mask` / `io_read_mask` (bit N = $F0+N).
        //   sync-out — apply write side effects ONLY for registers whose
        //              write bit is set (ports -> cpu_ports, $F0 test
        //              trigger, $F1 control, $F2/$F3 DSP, $FA-$FC timer
        //              targets), and read side effects only for registers
        //              whose read bit is set ($FD-$FF clear-on-read).
        //
        // The masks make the sync exact and value-independent: echoing an
        // unchanged value still counts as a write, and an untouched register
        // is never spuriously re-applied from a stale ARAM byte.

        // Save SPC port output before sync-in clobbers ARAM $F4-$F7.
        let spc_out_save = self.cpu_ports;

        {
            let raw = self.aram.as_raw_mut();
            // Main-CPU-written ports -> ARAM $F4-$F7.
            raw[0xF4] = self.spc_ports[0];
            raw[0xF5] = self.spc_ports[1];
            raw[0xF6] = self.spc_ports[2];
            raw[0xF7] = self.spc_ports[3];
            // Control shadow (reads of $F1 observe the live control value).
            raw[0xF1] = self.ctrl;
            // DSP address/data ports.
            raw[0xF2] = self.dsp_addr;
            raw[0xF3] = self.dsp.read_reg(self.dsp_addr);
            // Live timer outputs (clear-on-read applied in sync-out below).
            raw[0xFD] = self.timers[0].peek_output();
            raw[0xFE] = self.timers[1].peek_output();
            raw[0xFF] = self.timers[2].peek_output();
        }

        // Execute the pre-fetched opcode. The cpu.pc is already advanced past it.
        let raw = self.aram.as_raw_mut();
        self.cpu.io_written_mask = 0;
        self.cpu.io_read_mask = 0;
        let cycles = self.cpu.execute(raw, opcode);

        // Sync-out: ports first ($F4-$F7 are bidirectional — restore the
        // saved SPC output unless this instruction actually wrote the port).
        let io_mask = self.cpu.io_written_mask;
        #[cfg(feature = "introspect")]
        {
            for reg in 0..16 {
                if io_mask & (1 << reg) != 0 {
                    self.diag_spc_io_writes[reg] += 1;
                    self.diag_spc_io_last_write_pc[reg] = Some(pc);
                }
            }
        }
        for (i, &saved) in spc_out_save.iter().enumerate() {
            let port_bit = 1u16 << (4 + i); // bit 4 = $F4, bit 5 = $F5, …
            if io_mask & port_bit != 0 {
                self.cpu_ports[i] = self.aram.read(0xF4 + i as u16);
                #[cfg(feature = "introspect")]
                {
                    self.diag_spc_port_writes[i] += 1;
                    self.diag_spc_port_last_write_pc[i] = Some(pc);
                }
            } else {
                self.cpu_ports[i] = saved;
            }
        }

        // Write side effects, gated on the exact write mask.
        if io_mask & (1 << 0x0) != 0 {
            // $F0 test register: nonzero write halts (D9).
            let f0 = self.aram.read(0xF0);
            if f0 != 0 && self.halted.is_none() {
                self.halted = Some(ApuHalt::TestTrigger(f0));
                self.cpu.halted = self.halted;
            }
        }
        if io_mask & (1 << 0x1) != 0 {
            // $F1 control register: timer enables, port clears, IPL enable.
            let f1 = self.aram.read(0xF1);
            self.io_write(0xF1, f1);
        }
        if io_mask & (1 << 0x2) != 0 {
            self.dsp_addr = self.aram.read(0xF2);
        }
        if io_mask & (1 << 0x3) != 0 {
            // $F3 DSP data: a write goes to the currently addressed register
            // (the documented read-only alias range $80-$FF is ignored).
            if self.dsp_addr & 0x80 == 0 {
                let f3 = self.aram.read(0xF3);
                self.dsp.write_reg(self.dsp_addr, f3);
            }
        }
        if io_mask & (1 << 0xA) != 0 {
            let fa = self.aram.read(0xFA);
            self.timers[0].write_target(fa);
        }
        if io_mask & (1 << 0xB) != 0 {
            let fb = self.aram.read(0xFB);
            self.timers[1].write_target(fb);
        }
        if io_mask & (1 << 0xC) != 0 {
            let fc = self.aram.read(0xFC);
            self.timers[2].write_target(fc);
        }

        // Read side effects: timer outputs clear on read.
        let read_mask = self.cpu.io_read_mask;
        #[cfg(feature = "introspect")]
        {
            for reg in 0..16 {
                if read_mask & (1 << reg) != 0 {
                    self.diag_spc_io_reads[reg] += 1;
                    self.diag_spc_io_last_read_pc[reg] = Some(pc);
                }
            }
            for i in 0..4 {
                if read_mask & (1 << (0x4 + i)) != 0 {
                    self.diag_spc_port_reads[i] += 1;
                    self.diag_spc_port_last_read_pc[i] = Some(pc);
                }
            }
        }
        if read_mask & (1 << 0xD) != 0 {
            self.timers[0].clear_output();
        }
        if read_mask & (1 << 0xE) != 0 {
            self.timers[1].clear_output();
        }
        if read_mask & (1 << 0xF) != 0 {
            self.timers[2].clear_output();
        }

        if let Some(h) = self.cpu.halted {
            self.halted = Some(h);
        }

        cycles
    }

    // ---- Master-clock advance (integer accumulator) ----

    /// Advance the APU by `master_cycles` 65C816 master-clock cycles.
    ///
    /// Uses the integer accumulator model:
    ///   `spc_accum += master_cycles * SPC_NUM`
    ///   SPC cycles to drain = `spc_accum / SPC_DEN`
    ///   `spc_accum %= SPC_DEN`
    ///
    /// SPC700 instructions execute in whole-cycle steps, so the final step of
    /// a call can push the cycles actually run past the drained budget. That
    /// excess is carried forward as `spc_debt` and repaid out of *future*
    /// budgets (before the step loop runs), rather than being handed to the
    /// timers/DSP for free. This keeps the long-run SPC rate exactly
    /// `SPC_NUM/SPC_DEN` of the master clock regardless of call granularity —
    /// the accumulator model's contract stays true whether a caller advances
    /// once per frame or once per bus access. `spc_debt` is bounded (see
    /// `debug_assert!` below): repayment (`repaid = spc_debt.min(spc_to_run)`)
    /// happens before the step loop runs. FIX5: when `spc_to_run < spc_debt`,
    /// repayment consumes the entire budget (`spc_to_run` is driven to 0) and
    /// the step loop is skipped for this call with nonzero *residual* debt
    /// left over — debt at loop entry is not "always 0" in that case. The
    /// bound instead holds because the loop only ever *runs* when residual
    /// debt is exactly 0 (having been fully repaid), so any cycles the loop
    /// produces start from a zero base; the largest possible overshoot from
    /// a single call's loop is one step's worth of cycles (36, the IPL-HLE
    /// command step).
    ///
    /// The DSP is clocked one sample per `DSP_CLOCKS_PER_SAMPLE` SPC cycles.
    ///
    /// Returns the ApuHalt if a halt condition was reached during advance.
    pub fn advance_master_cycles(&mut self, master_cycles: u64) -> Option<ApuHalt> {
        self.spc_accum += master_cycles * SPC_NUM;
        let mut spc_to_run = self.spc_accum / SPC_DEN;
        self.spc_accum %= SPC_DEN;

        // Repay overshoot from earlier calls before running new cycles.
        let repaid = self.spc_debt.min(spc_to_run);
        self.spc_debt -= repaid;
        spc_to_run -= repaid;

        let mut spc_ran: u64 = 0;
        while spc_ran < spc_to_run {
            if self.halted.is_some() {
                break;
            }

            // Step the SPC700 one instruction.
            let cycles = self.step() as u64;
            let step_cycles = if cycles == 0 { 1 } else { cycles };
            spc_ran += step_cycles;

            // Advance timers.
            self.timers[0].advance(step_cycles as u32);
            self.timers[1].advance(step_cycles as u32);
            self.timers[2].advance(step_cycles as u32);

            // Advance DSP sample accumulator.
            self.dsp_cycle_accum += step_cycles;
            while self.dsp_cycle_accum >= DSP_CLOCKS_PER_SAMPLE {
                self.dsp_cycle_accum -= DSP_CLOCKS_PER_SAMPLE;
                // DSP step: get a mutable reference to ARAM for BRR/echo accesses.
                let aram_raw = self.aram.as_raw_mut();
                self.dsp.step_sample(aram_raw);
            }
        }

        // Carry the final instruction's overshoot into future budgets.
        // saturating_sub: a halted early-break leaves spc_ran <= spc_to_run -> 0.
        self.spc_debt += spc_ran.saturating_sub(spc_to_run);
        // FIX5: the analytical bound on spc_debt is 35 (one step short of
        // the max single `step()` return of 36 cycles — the overshoot is
        // at most `step_cycles - 1`). The debug_assert threshold is 64, a
        // deliberate safety margin above that analytical bound rather than
        // the bound itself: this assert is a tripwire against a future
        // change silently widening the max step cost (e.g. a slower
        // IPL-HLE command or a new instruction timing), not a tight
        // correctness check — a tight `< 35` would fire on any legitimate
        // widening before anyone had a chance to update the analysis.
        debug_assert!(self.spc_debt < 64);

        self.halted
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Apu (M2) timer tests ----

    #[test]
    fn apu_timer_enable_via_ctrl() {
        let mut apu = Apu::new();
        // Enable timer 0 via $F1 bit 0.
        apu.mem_write(0x00F1, 0x01);
        assert!(apu.timers[0].enabled);
        assert!(!apu.timers[1].enabled);
        assert!(!apu.timers[2].enabled);
    }

    #[test]
    fn apu_port_clear_bit4() {
        let mut apu = Apu::new();
        apu.spc_ports[0] = 0xAA;
        apu.spc_ports[1] = 0xBB;
        apu.cpu_ports[0] = 0x11;
        apu.cpu_ports[1] = 0x22;
        // Bit 4 of $F1 clears ports 0/1.
        apu.mem_write(0x00F1, 0x10);
        assert_eq!(apu.spc_ports[0], 0);
        assert_eq!(apu.spc_ports[1], 0);
        assert_eq!(
            apu.cpu_ports[0], 0x11,
            "SPC->CPU output port 0 is not cleared by PC10"
        );
        assert_eq!(
            apu.cpu_ports[1], 0x22,
            "SPC->CPU output port 1 is not cleared by PC10"
        );
        // Ports 2/3 should be unaffected.
        assert_eq!(apu.spc_ports[2], 0);
    }

    #[test]
    fn apu_port_clear_bit5() {
        let mut apu = Apu::new();
        apu.spc_ports[2] = 0xCC;
        apu.spc_ports[3] = 0xDD;
        apu.cpu_ports[2] = 0x11;
        apu.cpu_ports[3] = 0x22;
        apu.mem_write(0x00F1, 0x20);
        assert_eq!(apu.spc_ports[2], 0);
        assert_eq!(apu.spc_ports[3], 0);
        assert_eq!(
            apu.cpu_ports[2], 0x11,
            "SPC->CPU output port 2 is not cleared by PC32"
        );
        assert_eq!(
            apu.cpu_ports[3], 0x22,
            "SPC->CPU output port 3 is not cleared by PC32"
        );
    }

    #[test]
    fn apu_ipl_read_overlay() {
        let mut apu = Apu::new();
        // IPL enabled ($F1 bit 7 set by default in new()).
        assert!(apu.ipl_enabled());
        // Reading $FFC0 should return IPL_ROM[0].
        let v = apu.mem_read(0xFFC0);
        assert_eq!(v, IPL_ROM[0]);
    }

    #[test]
    fn apu_ipl_write_goes_to_aram() {
        let mut apu = Apu::new();
        apu.mem_write(0xFFC0, 0x42);
        // Write landed in ARAM even though IPL is enabled.
        assert_eq!(apu.aram.read(0xFFC0), 0x42);
        // But reads still see the IPL ROM overlay.
        assert_eq!(apu.mem_read(0xFFC0), IPL_ROM[0]);
    }

    #[test]
    fn apu_ipl_disabled_shows_ram() {
        let mut apu = Apu::new();
        apu.mem_write(0xFFC0, 0x99);
        // Disable IPL ROM (clear bit 7).
        apu.mem_write(0x00F1, 0x00);
        assert!(!apu.ipl_enabled());
        assert_eq!(apu.mem_read(0xFFC0), 0x99);
    }

    // ---- CPU port interface ----

    #[test]
    fn cpu_write_port_visible_to_spc() {
        let mut apu = Apu::new();
        apu.cpu_write_port(0, 0xCC);
        // The SPC700 reading $F4 should see $CC.
        assert_eq!(apu.spc_ports[0], 0xCC);
    }

    #[test]
    fn cpu_read_port_sees_spc_output() {
        let mut apu = Apu::new();
        // Simulate SPC writing $F4.
        apu.cpu_ports[0] = 0xAA;
        assert_eq!(apu.cpu_read_port(0), 0xAA);
    }

    // ---- DSP register access ----

    #[test]
    fn dsp_reg_write_read_roundtrip() {
        let mut apu = Apu::new();
        // Write to DSP register via $F2/$F3 I/O port.
        apu.mem_write(0x00F2, 0x5D); // DIR register
        apu.mem_write(0x00F3, 0x80); // DIR = $80 → samples at $8000
                                     // Read back.
        apu.mem_write(0x00F2, 0x5D);
        let v = apu.mem_read(0x00F3);
        assert_eq!(v, 0x80, "DSP register roundtrip failed");
    }

    // ---- Clock ratio ----

    #[test]
    fn spc_clock_ratio_nonzero() {
        const { assert!(SPC_NUM > 0) };
        const { assert!(SPC_DEN > 0) };
        // Ratio should be roughly 1/21 (about 4.76% of master clock).
        let ratio_pct = (SPC_NUM * 100) / SPC_DEN;
        assert!((4..=6).contains(&ratio_pct), "ratio should be ~4.76%");
    }

    #[test]
    fn advance_master_cycles_no_panic() {
        let mut apu = Apu::new();
        // Advance with IPL ROM executing: should not panic.
        // We stop at 1000 master cycles (the IPL ROM will be running).
        // Note: the APU runs in non-corpus mode; the IPL will start executing.
        // We just want no panics and no infinite loops.
        let halt = apu.advance_master_cycles(21477); // ~1 ms
                                                     // No halt expected during IPL startup.
        assert!(
            halt.is_none() || matches!(halt, Some(ApuHalt::Sleep)),
            "unexpected halt: {:?}",
            halt
        );
    }

    /// Production-mode IPL handshake: after ~1 ms the IPL ROM has written
    /// $AA to port 0 and $BB to port 1.  Then the host writes $CC, and the
    /// IPL echoes $CC. This exercises the production `Apu::step()` path.
    #[test]
    fn ipl_production_handshake() {
        let mut apu = Apu::new();

        // Advance enough for the IPL startup instructions to run.
        apu.advance_master_cycles(21477); // ~1 ms / ~1024 SPC cycles

        // After startup: IPL has written $AA to port 0 and $BB to port 1.
        assert_eq!(
            apu.cpu_read_port(0),
            0xAA,
            "IPL should have written $AA to port 0"
        );
        assert_eq!(
            apu.cpu_read_port(1),
            0xBB,
            "IPL should have written $BB to port 1"
        );

        // Host writes $CC to port 0 to start the upload.
        apu.cpu_write_port(0, 0xCC);

        // Give the IPL time to acknowledge (poll_cc loop + ack write).
        apu.advance_master_cycles(21477 * 5);

        // IPL should have echoed $CC on port 0.
        assert_eq!(
            apu.cpu_read_port(0),
            0xCC,
            "IPL should have echoed $CC on port 0"
        );
    }

    #[test]
    fn ipl_production_zero_based_upload_and_jump() {
        let mut apu = Apu::new();
        apu.advance_master_cycles(21_477);

        apu.cpu_write_port(2, 0x00);
        apu.cpu_write_port(3, 0x02);
        apu.cpu_write_port(1, 0x01);
        apu.cpu_write_port(0, 0xCC);
        apu.advance_master_cycles(4096);
        assert_eq!(apu.cpu_read_port(0), 0xCC);

        apu.cpu_write_port(1, 0x42);
        apu.cpu_write_port(0, 0x00);
        apu.advance_master_cycles(4096);
        assert_eq!(apu.cpu_read_port(0), 0x00);
        assert_eq!(apu.aram.read(0x0200), 0x42);

        apu.cpu_write_port(2, 0x00);
        apu.cpu_write_port(3, 0x02);
        apu.cpu_write_port(1, 0x00);
        apu.cpu_write_port(0, 0x02);
        apu.advance_master_cycles(4096);
        assert!(
            (0x0200..0xFFC0).contains(&apu.cpu.pc),
            "SPC should have jumped into uploaded program space, pc={:#06x}",
            apu.cpu.pc
        );
        assert_eq!(apu.cpu.a, 0);
        assert_eq!(apu.cpu.x, 0);
        assert_eq!(apu.cpu.y, 0);
        assert_ne!(apu.cpu.psw & spc700::psw::Z, 0);
    }

    // ---- IPL upload protocol round-trip ----
    //
    // This test simulates the host-side upload sequence against the real
    // `Apu::step` path (corpus mode): load the IPL ROM, run it, and verify
    // that a small payload lands in ARAM at the target address and the SPC700
    // jumps there.

    #[cfg(feature = "introspect")]
    #[test]
    fn ipl_upload_roundtrip() {
        use crate::apu::ipl::IPL_ROM;

        // Set up corpus-mode APU with a private flat ARAM copy for stepping.
        let mut apu = Apu::new_corpus();
        // Copy IPL ROM into high ARAM (in corpus mode, memory is flat).
        for (i, &b) in IPL_ROM.iter().enumerate() {
            apu.aram.write(0xFFC0 + i as u16, b);
        }
        // Set reset vector ($FFFE/$FFFF in ARAM) to $FFC0.
        apu.aram.write(0xFFFE, 0xC0);
        apu.aram.write(0xFFFF, 0xFF);
        apu.cpu.pc = 0xFFC0;

        // Port layout:
        // Host writes $CC to port 0 to start.
        // APU reads port 0 at the poll_cc loop; place it in ARAM[$F4].
        apu.aram.write(0x00F4, 0xCC);
        // addr_lo = $00 (port 2 = ARAM[$F6]), addr_hi = $01 (port 3 = ARAM[$F7])
        // → load address = $0100
        apu.aram.write(0x00F6, 0x00); // addr_lo
        apu.aram.write(0x00F7, 0x01); // addr_hi

        // Step through the IPL ROM startup sequence. Cap at 300 steps.
        for _ in 0..300 {
            let mem = apu.aram.as_raw_mut();
            let _ = apu.cpu.step(mem);
            // Stop when PC has left the initial $FFC0 block and advanced.
            if apu.cpu.pc > 0xFFC8 {
                break;
            }
        }
        // Minimal assertion: the IPL ROM startup ran and PC advanced.
        assert!(apu.cpu.pc != 0xFFC0, "IPL ROM should advance PC");
    }

    // ---- Overshoot bound tests (post debt-carry fix) ----
    //
    // `advance_master_cycles` drains whole SPC cycles from the integer
    // accumulator, then steps whole instructions until `spc_ran >=
    // spc_to_run`. The final instruction can push `spc_ran` past
    // `spc_to_run`; that excess is carried forward as `spc_debt` and repaid
    // out of the next call's budget before any further cycles run, so it
    // never reaches the timers or the DSP sample accumulator for free. One
    // big call overshoots at most once (bounded by one instruction); many
    // tiny calls (the `$2140-$2143` catch-up pattern in bus.rs) no longer
    // compound the overshoot per drained call, because each call's debt is
    // repaid before it steps a single new instruction.
    //
    // These tests pin the CORRECT post-fix clocking: a single call and a
    // pathologically chunked sequence over the same span both stay within
    // one instruction's worth of the exact SPC_NUM/SPC_DEN budget.

    /// SPC cycles actually delivered to the timers/DSP by
    /// `advance_master_cycles`, reconstructed from the DSP sample counter
    /// plus the sub-sample remainder.
    fn spc_cycles_delivered(apu: &Apu) -> u64 {
        apu.dsp.sample_count * DSP_CLOCKS_PER_SAMPLE + apu.dsp_cycle_accum
    }

    /// Exact SPC-cycle budget the accumulator model authorized across all
    /// `advance_master_cycles` calls totalling `total_master` cycles:
    /// sum(spc_to_run) == (total_master * SPC_NUM - final_accum) / SPC_DEN.
    fn spc_cycles_budgeted(apu: &Apu, total_master: u64) -> u64 {
        (total_master * SPC_NUM - apu.spc_accum) / SPC_DEN
    }

    #[test]
    fn advance_master_cycles_single_call_overshoot_is_bounded() {
        // One big call may overshoot the budget by at most one
        // instruction's worth of cycles (the IPL HLE WaitCommand step is a
        // fixed 7 SPC cycles, so at most 6 excess cycles here).
        const FRAME_MCLK: u64 = 357_368; // one NTSC frame
        let mut apu = Apu::new();
        apu.advance_master_cycles(FRAME_MCLK);

        let delivered = spc_cycles_delivered(&apu);
        let budget = spc_cycles_budgeted(&apu, FRAME_MCLK);
        assert!(
            delivered >= budget && delivered - budget <= 6,
            "single-call overshoot must be bounded by one instruction: \
             delivered={delivered} budget={budget}"
        );
    }

    #[test]
    fn advance_master_cycles_chunked_overshoot_is_bounded() {
        // Same total master cycles, delivered in 30-cycle chunks (the
        // scale of a per-CPU-access `apu_catch_up`). With debt-carry, each
        // chunk repays the prior call's overshoot before stepping a new
        // instruction, so only the final call's overshoot survives — the
        // same one-instruction bound as the single-call test, regardless of
        // how many calls it took to get there.
        const FRAME_MCLK: u64 = 357_368;
        const CHUNK: u64 = 30;

        let mut apu = Apu::new();
        let mut advanced = 0u64;
        while advanced < FRAME_MCLK {
            let c = CHUNK.min(FRAME_MCLK - advanced);
            apu.advance_master_cycles(c);
            advanced += c;
        }

        let delivered = spc_cycles_delivered(&apu);
        let budget = spc_cycles_budgeted(&apu, FRAME_MCLK);
        println!(
            "chunked overshoot: budget={budget} SPC cycles, delivered={delivered} \
             ({} permille), excess={}",
            delivered * 1000 / budget,
            delivered - budget
        );
        // Correct clocking: delivered - budget <= 6 (one instruction, as in
        // the single-call test) -- the debt-carry model prevents the excess
        // from compounding across chunks.
        assert!(
            delivered >= budget && delivered - budget <= 6,
            "chunked overshoot must be bounded by one instruction, same as \
             the single-call case: delivered={delivered} budget={budget}"
        );
    }

    #[test]
    fn advance_master_cycles_chunk_invariant_over_long_run_and_pathological_chunking() {
        // Debt-carry's core guarantee: over a long run, total SPC cycles
        // actually executed track the exact SPC_NUM/SPC_DEN budget within
        // one outstanding debt's worth (<= 35, per the `spc_debt` field's
        // documented bound), no matter how pathologically the master-cycle
        // span is chunked into `advance_master_cycles` calls -- including
        // many single-master-cycle calls, which is the extreme end of the
        // `$2140-$2143` per-bus-access catch-up pattern this fix targets.
        const TOTAL_MCLK: u64 = 357_368 * 3; // three NTSC frames

        for &chunk in &[1u64, 2, 3, 7, 30, 4096, 357_368] {
            let mut apu = Apu::new();
            let mut advanced = 0u64;
            while advanced < TOTAL_MCLK {
                let c = chunk.min(TOTAL_MCLK - advanced);
                apu.advance_master_cycles(c);
                advanced += c;
            }

            let delivered = spc_cycles_delivered(&apu);
            let budget = spc_cycles_budgeted(&apu, TOTAL_MCLK);
            assert!(
                delivered >= budget,
                "chunk={chunk}: delivered SPC cycles must never fall short \
                 of the budget: delivered={delivered} budget={budget}"
            );
            assert!(
                delivered - budget <= 35,
                "chunk={chunk}: outstanding debt exceeded its documented \
                 bound: delivered={delivered} budget={budget} \
                 excess={}",
                delivered - budget
            );
        }
    }

    #[test]
    fn advance_master_cycles_freezes_debt_and_samples_while_halted() {
        // Once halted, the step loop breaks before running any instruction
        // (`if self.halted.is_some() { break; }` precedes `step()`), so a
        // halted call can only ever repay outstanding debt (never grow it)
        // and never advances the DSP sample accumulator. Force the halt
        // directly via the `pub halted` field -- the same state `step()`
        // sets internally for SLEEP/STOP/test-trigger -- rather than driving
        // a specific SPC700 opcode sequence to reach it.
        let mut apu = Apu::new();
        apu.advance_master_cycles(1000);
        apu.halted = Some(ApuHalt::Sleep);

        // One halted call fully repays any debt outstanding from the prior
        // (non-halted) run, since repayment is unconditional and precedes
        // the loop, and this call's own spc_to_run comfortably exceeds the
        // <= 35 debt bound. From here, debt is stable at 0.
        apu.advance_master_cycles(21_477);
        let debt_before = apu.spc_debt;
        let samples_before = apu.dsp.sample_count;

        apu.advance_master_cycles(357_368); // one more full frame, still halted

        assert_eq!(
            apu.spc_debt, debt_before,
            "spc_debt must not change across a halted advance"
        );
        assert_eq!(
            apu.dsp.sample_count, samples_before,
            "no DSP samples should be produced while halted"
        );
    }

    // ---- Audio capture wiring (feature "audio" only) ----
    //
    // refwork-emu has no `Core`-constructing tests and no synthetic-ROM
    // fixture (the ROM builder lives in xtask, which depends on this
    // crate), so these pin the tap at the `Apu` level instead, following
    // the `advance_master_cycles_no_panic` precedent above: construct an
    // `Apu` directly and advance it. `Core::take_audio_samples` is a
    // trivial delegation chain (`Core` -> `SysBus::apu` -> `Apu::drain_audio`
    // -> `Dsp::drain_audio`) exercised end-to-end by ramdiff (package 02).
    #[cfg(feature = "audio")]
    mod audio_tests {
        use super::*;

        /// Master cycles to advance in the tests below. A fresh `Apu` never
        /// receives a host `$CC` handshake write, so `step()` stays in the
        /// IPL HLE `WaitCommand` busy-poll (`step_ipl_hle`, a fixed 7 SPC
        /// cycles/step, never halts) for the whole span — DSP stepping
        /// proceeds steadily and this duration is well below the capture
        /// ring's 4096-pair capacity.
        const TEST_MASTER_CYCLES: u64 = 200_000;

        /// Expected drained pair count from the documented clock model,
        /// computed with the same integer arithmetic `advance_master_cycles`
        /// uses (SPC_NUM/SPC_DEN accumulator, one DSP sample per
        /// `DSP_CLOCKS_PER_SAMPLE` SPC cycles). `advance_master_cycles` can
        /// run a few SPC cycles past the exact target before it notices
        /// `spc_ran >= spc_to_run` (bounded by one step's cycle count), so
        /// callers allow a small explicit tolerance rather than exact
        /// equality — this is the "accumulator remainder" the fixed 7-cycle
        /// HLE step size can shift a sample count by.
        fn expected_pairs(master_cycles: u64) -> u64 {
            (master_cycles * SPC_NUM / SPC_DEN) / DSP_CLOCKS_PER_SAMPLE
        }

        /// Explicit small tolerance (in pairs) around `expected_pairs`,
        /// covering the accumulator-remainder overshoot described above.
        const TOLERANCE_PAIRS: u64 = 4;

        #[test]
        fn audio_tap_wired_into_apu_stepping() {
            let mut apu = Apu::new();
            apu.advance_master_cycles(TEST_MASTER_CYCLES);

            let mut out = [0i16; 4096];
            let n = apu.drain_audio(&mut out);
            assert_eq!(n % 2, 0, "drain_audio must always write an even count");
            let pairs = (n / 2) as u64;

            let expected = expected_pairs(TEST_MASTER_CYCLES);
            let diff = pairs.abs_diff(expected);
            assert!(
                diff <= TOLERANCE_PAIRS,
                "drained {pairs} pairs, expected ~{expected} (+/- {TOLERANCE_PAIRS}) \
                 for {TEST_MASTER_CYCLES} master cycles"
            );
            assert!(pairs > 0, "expected some pairs to have been produced");
            assert_eq!(
                apu.audio_dropped_pairs(),
                0,
                "ring should not overflow for this test's small sample count"
            );
        }

        #[test]
        fn audio_tap_deterministic_across_independent_apus() {
            let mut apu1 = Apu::new();
            let mut apu2 = Apu::new();

            // Identical cycle sequence advanced on both.
            for chunk in [50_000u64, 30_000, 70_000, 50_000] {
                apu1.advance_master_cycles(chunk);
                apu2.advance_master_cycles(chunk);
            }

            let mut out1 = [0i16; 8192];
            let mut out2 = [0i16; 8192];
            let n1 = apu1.drain_audio(&mut out1);
            let n2 = apu2.drain_audio(&mut out2);

            assert_eq!(
                n1, n2,
                "two independently constructed Apus should drain the same \
                 pair count for an identical cycle sequence"
            );
            assert_eq!(
                &out1[..n1],
                &out2[..n2],
                "two independently constructed Apus should produce identical \
                 sample streams for an identical cycle sequence"
            );
            assert!(n1 > 0, "expected some pairs to have been produced");
        }

        // ---- Chunk-invariant sample-count tests (post debt-carry fix) ----
        //
        // Companions to the SPC-budget tests in the parent module: measure
        // chunk-size independence at the level the original overshoot-leak
        // symptom was observed — drained stereo pairs. With debt-carry, the
        // pair count is invariant (within one instruction's worth of
        // samples) to how the same total master-cycle span is chunked.

        /// Drain everything currently queued, returning pairs drained.
        fn drain_all_pairs(apu: &mut Apu) -> u64 {
            let mut buf = [0i16; 4096];
            let mut pairs = 0u64;
            loop {
                let n = apu.drain_audio(&mut buf);
                pairs += (n / 2) as u64;
                if n < buf.len() {
                    return pairs;
                }
            }
        }

        /// Total pairs produced since construction: drained now plus any
        /// lost to capture-ring overflow (counted, not recoverable).
        fn total_pairs_produced(apu: &mut Apu) -> u64 {
            drain_all_pairs(apu) + apu.audio_dropped_pairs()
        }

        /// Advance `total` master cycles in `chunk`-sized calls, draining
        /// between calls so the ring never overflows. Returns pairs.
        fn advance_chunked_counting_pairs(apu: &mut Apu, total: u64, chunk: u64) -> u64 {
            let mut advanced = 0u64;
            let mut pairs = 0u64;
            let mut buf = [0i16; 4096];
            while advanced < total {
                let c = chunk.min(total - advanced);
                apu.advance_master_cycles(c);
                advanced += c;
                let n = apu.drain_audio(&mut buf);
                pairs += (n / 2) as u64;
            }
            pairs + drain_all_pairs(apu) + apu.audio_dropped_pairs()
        }

        #[test]
        fn sample_count_independent_of_advance_chunk_size() {
            // Identical Apus, identical total master cycles; only the call
            // granularity differs. Debt-carry keeps the two pair counts
            // within one instruction's worth of samples of each other,
            // regardless of chunking.
            const FRAME_MCLK: u64 = 357_368; // one NTSC frame

            let mut single = Apu::new();
            single.advance_master_cycles(FRAME_MCLK);
            let single_pairs = total_pairs_produced(&mut single);

            let mut chunked = Apu::new();
            let chunked_pairs = advance_chunked_counting_pairs(&mut chunked, FRAME_MCLK, 30);

            println!(
                "one frame ({FRAME_MCLK} mclk): single-call={single_pairs} pairs, \
                 30-cycle chunks={chunked_pairs} pairs ({} permille)",
                chunked_pairs * 1000 / single_pairs
            );
            assert!(
                chunked_pairs.abs_diff(single_pairs) <= 1,
                "chunked and single-call pair counts should match within one \
                 instruction's worth of samples: single={single_pairs} \
                 chunked={chunked_pairs}"
            );
        }

        #[test]
        fn sample_rate_one_second_single_vs_chunked() {
            // One second of master cycles must produce ~32,000 pairs
            // regardless of call granularity. 21,477,000 master cycles is
            // exactly 1,024,000 SPC cycles under SPC_NUM/SPC_DEN, i.e.
            // exactly 32,000 DSP samples.
            const ONE_SECOND_MCLK: u64 = 21_477_000;

            let mut single = Apu::new();
            single.advance_master_cycles(ONE_SECOND_MCLK);
            let single_pairs = total_pairs_produced(&mut single);

            let mut chunked = Apu::new();
            let chunked_pairs =
                advance_chunked_counting_pairs(&mut chunked, ONE_SECOND_MCLK, 30);

            println!(
                "one second ({ONE_SECOND_MCLK} mclk): single-call={single_pairs} pairs, \
                 30-cycle chunks={chunked_pairs} pairs ({} permille)",
                chunked_pairs * 1000 / single_pairs
            );
            assert_eq!(
                single_pairs, 32_000,
                "single-call rate should be exactly 32 kHz for one second"
            );
            assert!(
                chunked_pairs.abs_diff(32_000) <= 1,
                "chunked rate should also be ~32 kHz for one second, \
                 regardless of call granularity; got {chunked_pairs}"
            );
        }
    }
}
