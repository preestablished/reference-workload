//! 16-bit CPU core (65C816-class): full instruction set, decimal mode,
//! native + emulation modes, interrupts.
//!
//! OWNER (implementation): CPU agent. The CPU is written against the
//! [`Bus`] trait only — it never tracks time itself (every bus access /
//! internal cycle is clocked by the bus implementation) and holds no state
//! outside this struct (D5).
//!
//! Implemented clean-room from public 65C816-class hardware documentation
//! (datasheet register/flag layout, published opcode matrix, documented
//! decimal-mode and addressing-mode behavior). No emulator source consulted.

use crate::bus::Bus;

mod addressing;
mod alu;
mod exec;

/// Processor status flag bits (P register).
pub mod flags {
    /// Carry.
    pub const C: u8 = 0x01;
    /// Zero.
    pub const Z: u8 = 0x02;
    /// IRQ disable.
    pub const I: u8 = 0x04;
    /// Decimal mode.
    pub const D: u8 = 0x08;
    /// Index register width (native): 1 = 8-bit X/Y. Break flag in
    /// emulation-mode pushed copies.
    pub const X: u8 = 0x10;
    /// Accumulator/memory width (native): 1 = 8-bit A.
    pub const M: u8 = 0x20;
    /// Overflow.
    pub const V: u8 = 0x40;
    /// Negative.
    pub const N: u8 = 0x80;
}

/// Interrupt vector addresses (24-bit, all in bank $00).
mod vectors {
    pub const NATIVE_COP: u32 = 0x00_FFE4;
    pub const NATIVE_BRK: u32 = 0x00_FFE6;
    pub const NATIVE_NMI: u32 = 0x00_FFEA;
    pub const NATIVE_IRQ: u32 = 0x00_FFEE;
    pub const EMU_COP: u32 = 0x00_FFF4;
    pub const EMU_NMI: u32 = 0x00_FFFA;
    pub const EMU_RESET: u32 = 0x00_FFFC;
    pub const EMU_IRQ_BRK: u32 = 0x00_FFFE;
}

/// Architectural registers + halt latches. No hidden state (D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cpu {
    /// Accumulator (full 16 bits; M flag selects active width).
    pub a: u16,
    /// Index X (X flag forces the high byte to zero while set).
    pub x: u16,
    /// Index Y.
    pub y: u16,
    /// Stack pointer (forced to $01xx in emulation mode).
    pub s: u16,
    /// Direct-page register.
    pub d: u16,
    /// Data-bank register.
    pub dbr: u8,
    /// Program-bank register.
    pub pbr: u8,
    /// Program counter (16-bit, within PBR bank).
    pub pc: u16,
    /// Processor status.
    pub p: u8,
    /// Emulation-mode flag (E).
    pub e: bool,
    /// Set by `WAI`; cleared when an interrupt (or pending-IRQ-with-I-set)
    /// releases the wait.
    pub waiting: bool,
    /// Set by `STP`; the core converts this into a fault (D9).
    pub stopped: bool,
}

impl Cpu {
    /// Power-on register state (pre-reset). `reset` must be called with a
    /// bus before stepping.
    pub fn new() -> Cpu {
        Cpu {
            a: 0,
            x: 0,
            y: 0,
            s: 0x01FF,
            d: 0,
            dbr: 0,
            pbr: 0,
            pc: 0,
            p: flags::M | flags::X | flags::I,
            e: true,
            waiting: false,
            stopped: false,
        }
    }

    /// Reset sequence: enter emulation mode, load the reset vector from
    /// $00FFFC/D, set status per the documented reset state.
    pub fn reset<B: Bus>(&mut self, bus: &mut B) {
        // Documented reset state: emulation mode, M/X/I set, D cleared.
        self.e = true;
        self.p = flags::M | flags::X | flags::I;
        self.p &= !flags::D;
        // Emulation forces the high bytes of S/X/Y and S into page 1.
        self.s = 0x0100 | (self.s & 0x00FF);
        self.x &= 0x00FF;
        self.y &= 0x00FF;
        self.d = 0;
        self.dbr = 0;
        self.pbr = 0;
        self.waiting = false;
        self.stopped = false;

        // The reset sequence spends several internal cycles before fetching
        // the vector; approximate the documented count with idle cycles.
        bus.idle();
        bus.idle();
        let lo = bus.read(vectors::EMU_RESET) as u16;
        let hi = bus.read(vectors::EMU_RESET + 1) as u16;
        self.pc = lo | (hi << 8);
    }

    /// Service a pending interrupt if one is due, otherwise execute exactly
    /// one instruction. While `waiting` (WAI) with no interrupt pending,
    /// consumes one idle cycle and returns. While `stopped`, returns
    /// immediately.
    ///
    /// Interrupt priority at an instruction boundary: NMI edge
    /// (`bus.take_nmi()`) over IRQ level (`bus.irq_line()`, masked by `I`).
    pub fn step<B: Bus>(&mut self, bus: &mut B) {
        // Hardware invariant: in emulation mode the stack high byte reads as
        // $01 at every instruction boundary, no matter how S was loaded or
        // where a "new"-instruction push left it; enforce it on entry and
        // exit so externally-injected state behaves like silicon.
        if self.e {
            self.s = 0x0100 | (self.s & 0x00FF);
        }
        self.step_inner(bus);
        if self.e {
            self.s = 0x0100 | (self.s & 0x00FF);
        }
    }

    fn step_inner<B: Bus>(&mut self, bus: &mut B) {
        if self.stopped {
            // STP halts the processor; the driver converts this to a fault.
            return;
        }

        if self.waiting {
            // WAI: released by an NMI edge or an asserted IRQ line.
            if bus.take_nmi() {
                self.waiting = false;
                self.service_interrupt(bus, IntKind::Nmi);
                return;
            }
            if bus.irq_line() {
                self.waiting = false;
                if self.p & flags::I == 0 {
                    self.service_interrupt(bus, IntKind::Irq);
                }
                // If IRQ is masked, simply fall through to resume execution
                // at the next instruction this same step.
            } else {
                bus.idle();
                return;
            }
        }

        // Interrupt service at the instruction boundary: NMI over IRQ.
        if bus.take_nmi() {
            self.service_interrupt(bus, IntKind::Nmi);
            return;
        }
        if bus.irq_line() && (self.p & flags::I == 0) {
            self.service_interrupt(bus, IntKind::Irq);
            return;
        }

        let opcode = self.fetch8(bus);
        self.execute(bus, opcode);
    }
}

/// Hardware interrupt kinds (BRK/COP are handled inline by their opcodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntKind {
    Nmi,
    Irq,
    Brk,
    Cop,
}

impl Cpu {
    /// True when the accumulator/memory width is 8-bit.
    #[inline]
    pub(crate) fn m8(&self) -> bool {
        self.p & flags::M != 0
    }

    /// True when the index registers are 8-bit.
    #[inline]
    pub(crate) fn x8(&self) -> bool {
        self.p & flags::X != 0
    }

    /// Set/clear a status flag.
    #[inline]
    pub(crate) fn set_flag(&mut self, mask: u8, on: bool) {
        if on {
            self.p |= mask;
        } else {
            self.p &= !mask;
        }
    }

    /// Apply N/Z for an 8-bit result.
    #[inline]
    pub(crate) fn set_nz8(&mut self, v: u8) {
        self.set_flag(flags::Z, v == 0);
        self.set_flag(flags::N, v & 0x80 != 0);
    }

    /// Apply N/Z for a 16-bit result.
    #[inline]
    pub(crate) fn set_nz16(&mut self, v: u16) {
        self.set_flag(flags::Z, v == 0);
        self.set_flag(flags::N, v & 0x8000 != 0);
    }

    /// Apply N/Z for a value whose width follows the M flag.
    #[inline]
    pub(crate) fn set_nz_m(&mut self, v: u16) {
        if self.m8() {
            self.set_nz8(v as u8);
        } else {
            self.set_nz16(v);
        }
    }

    /// Reconcile register widths after a P or E change. When X=1 the index
    /// high bytes are forced to zero; emulation mode forces M=X=1 and keeps
    /// S in page 1.
    pub(crate) fn normalize_widths(&mut self) {
        if self.e {
            self.p |= flags::M | flags::X;
        }
        if self.x8() {
            self.x &= 0x00FF;
            self.y &= 0x00FF;
        }
        if self.e {
            self.s = 0x0100 | (self.s & 0x00FF);
        }
    }

    // ---- byte fetch (program stream) ----

    #[inline]
    pub(crate) fn fetch8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let addr = ((self.pbr as u32) << 16) | self.pc as u32;
        self.pc = self.pc.wrapping_add(1);
        bus.read(addr)
    }

    #[inline]
    pub(crate) fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch8(bus) as u16;
        let hi = self.fetch8(bus) as u16;
        lo | (hi << 8)
    }

    // ---- stack helpers ----

    #[inline]
    pub(crate) fn push8<B: Bus>(&mut self, bus: &mut B, v: u8) {
        bus.write(self.s as u32, v);
        if self.e {
            // Emulation: stack wraps inside page 1.
            self.s = 0x0100 | (self.s.wrapping_sub(1) & 0x00FF);
        } else {
            self.s = self.s.wrapping_sub(1);
        }
    }

    #[inline]
    pub(crate) fn pull8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        if self.e {
            self.s = 0x0100 | (self.s.wrapping_add(1) & 0x00FF);
        } else {
            self.s = self.s.wrapping_add(1);
        }
        bus.read(self.s as u32)
    }

    /// Push a 16-bit value high byte first (so the low byte ends up at the
    /// lower stack address, matching pull order).
    #[inline]
    pub(crate) fn push16<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.push8(bus, (v >> 8) as u8);
        self.push8(bus, v as u8);
    }

    #[inline]
    pub(crate) fn pull16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.pull8(bus) as u16;
        let hi = self.pull8(bus) as u16;
        lo | (hi << 8)
    }

    // "New" 65C816 instructions (PEA/PEI/PER/PHB/PHD/PLB/PLD/JSL/RTL) use
    // full 16-bit stack arithmetic even in emulation mode — the stack may
    // temporarily leave page 1 mid-instruction; S's high byte is re-forced
    // to $01 at the instruction boundary (see `step`). Only the 6502-era
    // instructions wrap within page 1.

    #[inline]
    pub(crate) fn push8_n<B: Bus>(&mut self, bus: &mut B, v: u8) {
        bus.write(self.s as u32, v);
        self.s = self.s.wrapping_sub(1);
    }

    #[inline]
    pub(crate) fn pull8_n<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.s = self.s.wrapping_add(1);
        bus.read(self.s as u32)
    }

    #[inline]
    pub(crate) fn push16_n<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.push8_n(bus, (v >> 8) as u8);
        self.push8_n(bus, v as u8);
    }

    #[inline]
    pub(crate) fn pull16_n<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.pull8_n(bus) as u16;
        let hi = self.pull8_n(bus) as u16;
        lo | (hi << 8)
    }

    // ---- interrupt service ----

    /// Push state and jump to the appropriate vector. BRK/COP are dispatched
    /// here from their opcode handlers (with PC already advanced past the
    /// signature byte).
    pub(crate) fn service_interrupt<B: Bus>(&mut self, bus: &mut B, kind: IntKind) {
        // Two internal cycles of interrupt-recognition overhead.
        bus.idle();
        bus.idle();

        if self.e {
            // Emulation entry pushes PCH, PCL, P (B per source), no PBR.
            self.push8(bus, (self.pc >> 8) as u8);
            self.push8(bus, self.pc as u8);
            let mut pushed = self.p;
            // In the pushed copy, bit4 (X/B) is set for BRK/COP software
            // interrupts, clear for hardware IRQ/NMI.
            match kind {
                IntKind::Brk | IntKind::Cop => pushed |= flags::X,
                IntKind::Nmi | IntKind::Irq => pushed &= !flags::X,
            }
            self.push8(bus, pushed);
            self.set_flag(flags::I, true);
            self.set_flag(flags::D, false);
            self.pbr = 0;
            let vec = match kind {
                IntKind::Cop => vectors::EMU_COP,
                IntKind::Nmi => vectors::EMU_NMI,
                IntKind::Irq | IntKind::Brk => vectors::EMU_IRQ_BRK,
            };
            let lo = bus.read(vec) as u16;
            let hi = bus.read(vec + 1) as u16;
            self.pc = lo | (hi << 8);
        } else {
            // Native entry pushes PBR, PCH, PCL, P (full byte).
            self.push8(bus, self.pbr);
            self.push8(bus, (self.pc >> 8) as u8);
            self.push8(bus, self.pc as u8);
            self.push8(bus, self.p);
            self.set_flag(flags::I, true);
            self.set_flag(flags::D, false);
            self.pbr = 0;
            let vec = match kind {
                IntKind::Cop => vectors::NATIVE_COP,
                IntKind::Brk => vectors::NATIVE_BRK,
                IntKind::Nmi => vectors::NATIVE_NMI,
                IntKind::Irq => vectors::NATIVE_IRQ,
            };
            let lo = bus.read(vec) as u16;
            let hi = bus.read(vec + 1) as u16;
            self.pc = lo | (hi << 8);
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu::new()
    }
}

#[cfg(test)]
mod tests;
