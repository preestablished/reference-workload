//! Effective-address resolution for every 65C816-class addressing mode and
//! width-aware data access helpers.
//!
//! An effective address is a full 24-bit pointer plus the bank-relative
//! offset (so 16-bit accesses know whether to wrap within the bank). The
//! 65C816 wraps differently per mode; the rules implemented here follow the
//! documented hardware behavior:
//!
//! - Direct-page address computation wraps within bank $00 (the DP pointer
//!   is always in bank $00).
//! - Emulation-mode direct-page indexing, when DL == 0, wraps the index
//!   inside the direct page (low byte only).
//! - Absolute / absolute-long / data accesses use the data bank; absolute
//!   indexing can cross banks; long indexing carries into the bank.

use super::Cpu;
use crate::bus::Bus;

/// A resolved effective address: full 24-bit base for the first byte.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ea {
    /// 24-bit address of the low byte of the operand.
    pub addr: u32,
    /// When true, multi-byte accesses wrap within bank $00 (direct-page and
    /// stack-relative semantics). Otherwise the address simply increments
    /// (carrying into the next bank if it overflows $FFFF).
    pub wrap_bank0: bool,
}

impl Ea {
    fn flat(addr: u32) -> Ea {
        Ea {
            addr,
            wrap_bank0: false,
        }
    }

    fn dp(addr: u32) -> Ea {
        Ea {
            addr,
            wrap_bank0: true,
        }
    }

    /// Address of the `i`-th byte of a multi-byte operand.
    pub(crate) fn byte_addr(&self, i: u32) -> u32 {
        if self.wrap_bank0 {
            // Wrap within bank $00 (low 16 bits).
            (self.addr & 0xFF_0000) | ((self.addr.wrapping_add(i)) & 0x00_FFFF)
        } else {
            (self.addr.wrapping_add(i)) & 0xFF_FFFF
        }
    }
}

impl Cpu {
    // ---------------------------------------------------------------------
    // Width-aware data access against a resolved effective address.
    // ---------------------------------------------------------------------

    pub(crate) fn read8<B: Bus>(&mut self, bus: &mut B, ea: Ea) -> u8 {
        bus.read(ea.byte_addr(0))
    }

    pub(crate) fn write8<B: Bus>(&mut self, bus: &mut B, ea: Ea, v: u8) {
        bus.write(ea.byte_addr(0), v);
    }

    pub(crate) fn read16<B: Bus>(&mut self, bus: &mut B, ea: Ea) -> u16 {
        let lo = bus.read(ea.byte_addr(0)) as u16;
        let hi = bus.read(ea.byte_addr(1)) as u16;
        lo | (hi << 8)
    }

    pub(crate) fn write16<B: Bus>(&mut self, bus: &mut B, ea: Ea, v: u16) {
        bus.write(ea.byte_addr(0), v as u8);
        bus.write(ea.byte_addr(1), (v >> 8) as u8);
    }

    /// Read M-width data (8 or 16 bits) from an effective address.
    pub(crate) fn read_m<B: Bus>(&mut self, bus: &mut B, ea: Ea) -> u16 {
        if self.m8() {
            self.read8(bus, ea) as u16
        } else {
            self.read16(bus, ea)
        }
    }

    /// Write M-width data to an effective address.
    pub(crate) fn write_m<B: Bus>(&mut self, bus: &mut B, ea: Ea, v: u16) {
        if self.m8() {
            self.write8(bus, ea, v as u8);
        } else {
            self.write16(bus, ea, v);
        }
    }

    // ---------------------------------------------------------------------
    // Address-mode resolvers. Each fetches its operand bytes from the
    // program stream and returns the effective address.
    // ---------------------------------------------------------------------

    /// Direct page: `d + offset`, wrapping within bank $00. When E=1 and
    /// DL==0, the effective address wraps within the direct page (low byte).
    pub(crate) fn ea_direct<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        // Extra internal cycle when the direct-page register low byte is
        // nonzero (documented DP penalty).
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let addr = self.d.wrapping_add(off);
        Ea::dp(addr as u32)
    }

    /// Direct page indexed by an index register, with the emulation-mode
    /// DL==0 wrap quirk.
    fn ea_direct_indexed<B: Bus>(&mut self, bus: &mut B, index: u16) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        // Indexing costs one internal cycle.
        bus.idle();
        let addr = if self.e && (self.d & 0x00FF == 0) {
            // Wrap inside the direct page: only the low byte varies.
            (self.d & 0xFF00) | (off.wrapping_add(index) & 0x00FF)
        } else {
            self.d.wrapping_add(off).wrapping_add(index)
        };
        Ea::dp(addr as u32)
    }

    pub(crate) fn ea_direct_x<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let x = self.x;
        self.ea_direct_indexed(bus, x)
    }

    pub(crate) fn ea_direct_y<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let y = self.y;
        self.ea_direct_indexed(bus, y)
    }

    /// Read a 16-bit pointer from the direct page at `base`. The two pointer
    /// bytes are fetched at consecutive 16-bit addresses in bank $00 — the
    /// fetch does not wrap within the page even in emulation mode with
    /// DL==0 (verified empirically against the public single-step corpus;
    /// the E/DL==0 wrap quirk applies to indexed direct-page *data*
    /// addresses, not to the indirect pointer word).
    fn read_dp_ptr16<B: Bus>(&mut self, bus: &mut B, base: u16) -> u16 {
        // `wrapping_add` keeps the second byte inside bank $00 (direct page
        // addresses never carry into bank $01).
        let lo = bus.read(base as u32) as u16;
        let hi = bus.read(base.wrapping_add(1) as u32) as u16;
        lo | (hi << 8)
    }

    /// Read a 24-bit pointer from the direct page (indirect long); the three
    /// bytes wrap within bank $00.
    fn read_dp_ptr24<B: Bus>(&mut self, bus: &mut B, base: u16) -> u32 {
        let lo = bus.read(base as u32) as u32;
        let mid = bus.read(base.wrapping_add(1) as u32) as u32;
        let hi = bus.read(base.wrapping_add(2) as u32) as u32;
        lo | (mid << 8) | (hi << 16)
    }

    /// Read the 16-bit word stored in the direct page at the operand offset
    /// (used by PEI, which pushes the pointer word itself).
    pub(crate) fn read_dp_word<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let base = self.d.wrapping_add(off);
        self.read_dp_ptr16(bus, base)
    }

    /// (Direct) — indirect through a DP pointer, using the data bank.
    pub(crate) fn ea_indirect<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let base = self.d.wrapping_add(off);
        let ptr = self.read_dp_ptr16(bus, base);
        let addr = ((self.dbr as u32) << 16) | ptr as u32;
        Ea::flat(addr)
    }

    /// (Direct,X) — indexed indirect.
    pub(crate) fn ea_indirect_x<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        bus.idle();
        let base = if self.e && (self.d & 0x00FF == 0) {
            (self.d & 0xFF00) | (off.wrapping_add(self.x) & 0x00FF)
        } else {
            self.d.wrapping_add(off).wrapping_add(self.x)
        };
        let ptr = self.read_dp_ptr16(bus, base);
        let addr = ((self.dbr as u32) << 16) | ptr as u32;
        Ea::flat(addr)
    }

    /// (Direct),Y — indirect indexed. Adds Y to the fetched pointer,
    /// carrying into the data bank.
    pub(crate) fn ea_indirect_y<B: Bus>(&mut self, bus: &mut B, write: bool) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let base = self.d.wrapping_add(off);
        let ptr = self.read_dp_ptr16(bus, base);
        let flat = ((self.dbr as u32) << 16) | ptr as u32;
        let eff = (flat + self.y as u32) & 0xFF_FFFF;
        // Page-cross / write penalty: extra internal cycle when the index is
        // 16-bit, or on a page crossing for reads, or always for writes.
        if !self.x8() || write || (flat & 0xFF00) != (eff & 0xFF00) {
            bus.idle();
        }
        Ea::flat(eff)
    }

    /// [Direct] — indirect long through a 24-bit DP pointer.
    pub(crate) fn ea_indirect_long<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let base = self.d.wrapping_add(off);
        let ptr = self.read_dp_ptr24(bus, base);
        Ea::flat(ptr & 0xFF_FFFF)
    }

    /// [Direct],Y — indirect long indexed.
    pub(crate) fn ea_indirect_long_y<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        if self.d & 0x00FF != 0 {
            bus.idle();
        }
        let base = self.d.wrapping_add(off);
        let ptr = self.read_dp_ptr24(bus, base);
        Ea::flat((ptr.wrapping_add(self.y as u32)) & 0xFF_FFFF)
    }

    /// Absolute — 16-bit operand in the data bank.
    pub(crate) fn ea_absolute<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch16(bus);
        let addr = ((self.dbr as u32) << 16) | off as u32;
        Ea::flat(addr)
    }

    /// Absolute indexed by an index register; data bank with cross-bank
    /// carry. Read accesses pay the extra cycle only on a page cross when X
    /// is 8-bit; writes/16-bit-index always pay it.
    fn ea_absolute_indexed<B: Bus>(&mut self, bus: &mut B, index: u16, write: bool) -> Ea {
        let off = self.fetch16(bus);
        let flat = ((self.dbr as u32) << 16) | off as u32;
        let eff = (flat + index as u32) & 0xFF_FFFF;
        if !self.x8() || write || (flat & 0xFF00) != (eff & 0xFF00) {
            bus.idle();
        }
        Ea::flat(eff)
    }

    pub(crate) fn ea_absolute_x<B: Bus>(&mut self, bus: &mut B, write: bool) -> Ea {
        let x = self.x;
        self.ea_absolute_indexed(bus, x, write)
    }

    pub(crate) fn ea_absolute_y<B: Bus>(&mut self, bus: &mut B, write: bool) -> Ea {
        let y = self.y;
        self.ea_absolute_indexed(bus, y, write)
    }

    /// Absolute long — 24-bit operand (bank from the instruction).
    pub(crate) fn ea_long<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch16(bus) as u32;
        let bank = self.fetch8(bus) as u32;
        Ea::flat((bank << 16) | off)
    }

    /// Absolute long indexed by X — carries into the bank.
    pub(crate) fn ea_long_x<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch16(bus) as u32;
        let bank = self.fetch8(bus) as u32;
        let flat = (bank << 16) | off;
        Ea::flat((flat.wrapping_add(self.x as u32)) & 0xFF_FFFF)
    }

    /// Stack relative — `S + offset`, always in bank $00.
    pub(crate) fn ea_stack_rel<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        // One internal cycle for the stack-pointer add.
        bus.idle();
        let addr = self.s.wrapping_add(off);
        Ea::dp(addr as u32)
    }

    /// (Stack relative),Y — pointer in bank $00, indexed by Y into the data
    /// bank.
    pub(crate) fn ea_stack_rel_y<B: Bus>(&mut self, bus: &mut B) -> Ea {
        let off = self.fetch8(bus) as u16;
        bus.idle();
        let base = self.s.wrapping_add(off);
        // Pointer bytes wrap within bank $00.
        let lo = bus.read(base as u32) as u16;
        let hi = bus.read(base.wrapping_add(1) as u32) as u16;
        let ptr = lo | (hi << 8);
        let flat = ((self.dbr as u32) << 16) | ptr as u32;
        // Internal cycle for the Y add.
        bus.idle();
        Ea::flat((flat.wrapping_add(self.y as u32)) & 0xFF_FFFF)
    }
}
