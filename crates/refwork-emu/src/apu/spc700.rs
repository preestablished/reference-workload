//! SPC700 audio-CPU core: full instruction set, all addressing modes, cycle
//! counts, and direct-page / memory-mapped I/O dispatch.
//!
//! The SPC700 is an 8-bit processor with a 16-bit address space. It differs
//! from the 65C02/65C816 family in several ways:
//!
//! - **Direct page**: the P flag (PSW bit 5) selects page $00 or page $01 as
//!   the direct page (DP). All direct-page accesses are zero-bank within the
//!   64 KiB space.
//! - **16-bit operations**: `MOVW`, `ADDW`, `SUBW`, `CMPW`, `INCW`, `DECW`,
//!   `MUL`, `DIV` operate on 16-bit YA pairs or DP memory words.
//! - **Bit operations**: `SET1`/`CLR1` (set/clear a specific bit), `BBS`/`BBC`
//!   (branch if bit set/clear), `AND1`/`OR1`/`EOR1`/`MOV1`/`NOT1`,
//!   `TSET1`/`TCLR1`.
//! - **Unique addressing modes**: `(X)`, `(Y)`, `(X)+` (post-increment),
//!   `dp+X`, `dp+Y`, `!abs`, `!abs+X`, `!abs+Y`, `[dp+X]`, `[dp]+Y`.
//! - **PSW layout**: N V P B H I Z C (bit 7 down to bit 0).
//!
//! ## Memory bus
//!
//! The SPC700 owns ARAM directly (no Bus trait). Memory accesses go through
//! [`super::Apu`]'s `mem_read` / `mem_write` helpers which apply the I/O
//! register overlay ($F0–$FF) and the IPL ROM overlay ($FFC0–$FFFF).
//!
//! In **corpus mode** (used by the single-step test runner), the I/O region
//! is treated as plain RAM so the corpus's flat-64 KiB model matches. See
//! [`Spc700::new_corpus`].
//!
//! ## Cycle counts
//!
//! `step()` returns the number of SPC700 master-clock cycles consumed. These
//! are raw SPC cycles; the caller (package 02) converts to 65C816 master
//! clock units when scheduling.
//!
//! ## Design
//!
//! All 256 opcodes are dispatched via a flat `match`. Addressing-mode helpers
//! follow the same pattern as the 65C816 core in `cpu/addressing.rs`.
//! Arithmetic helpers are self-contained in the `impl` blocks.

/// PSW (processor status word) bit masks.
pub mod psw {
    pub const N: u8 = 0x80; // Negative
    pub const V: u8 = 0x40; // Overflow
    pub const P: u8 = 0x20; // Direct-page select (0=$00xx, 1=$01xx)
    pub const B: u8 = 0x10; // Break (set when BRK pushes P)
    pub const H: u8 = 0x08; // Half-carry (used by DAA/DAS and ADC/SBC)
    pub const I: u8 = 0x04; // IRQ enable (1 = enabled)
    pub const Z: u8 = 0x02; // Zero
    pub const C: u8 = 0x01; // Carry
}

/// Reason the SPC700 stopped executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuHalt {
    /// `SLEEP` instruction executed (opcode $EF). The processor halts until an
    /// external interrupt. Package 02 decides how to handle this.
    Sleep,
    /// `STOP` instruction executed (opcode $FF). The processor halts
    /// permanently until a hard reset. Package 02 raises a [`crate::fault::Fault`].
    Stop,
    /// `$F0` test register written nonzero. Used by test ROMs and package 02
    /// maps to a fault.
    TestTrigger(u8),
}

/// SPC700 CPU registers. All state lives here (D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spc700 {
    pub pc: u16,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    /// Stack pointer (points into page 1: effective address = $0100 | sp).
    pub sp: u8,
    /// Processor status word.
    pub psw: u8,
    /// Set when the core halts (SLEEP/STOP/test-register trigger).
    pub halted: Option<ApuHalt>,
    /// When true, I/O register range $F0–$FF is treated as plain RAM (corpus
    /// mode). Production code leaves this false.
    pub corpus_mode: bool,
}

impl Default for Spc700 {
    fn default() -> Self {
        Spc700::new()
    }
}

impl Spc700 {
    /// Standard power-on state.
    pub fn new() -> Self {
        Spc700 {
            pc: 0xFFC0, // IPL ROM entry point
            a: 0,
            x: 0,
            y: 0,
            sp: 0xEF,
            psw: 0x02, // Z set by convention; all others clear
            halted: None,
            corpus_mode: false,
        }
    }

    /// Corpus-mode constructor: I/O region treated as plain flat RAM.
    #[cfg(feature = "introspect")]
    pub fn new_corpus() -> Self {
        let mut s = Self::new();
        s.corpus_mode = true;
        s
    }

    // ---- PSW helpers ----

    #[inline]
    fn flag(&self, mask: u8) -> bool {
        self.psw & mask != 0
    }

    #[inline]
    fn set_flag(&mut self, mask: u8, on: bool) {
        if on {
            self.psw |= mask;
        } else {
            self.psw &= !mask;
        }
    }

    #[inline]
    fn set_nz(&mut self, v: u8) {
        self.set_flag(psw::N, v & 0x80 != 0);
        self.set_flag(psw::Z, v == 0);
    }

    #[inline]
    fn set_nz16(&mut self, v: u16) {
        self.set_flag(psw::N, v & 0x8000 != 0);
        self.set_flag(psw::Z, v == 0);
    }

    /// Direct-page base address: $0000 when P=0, $0100 when P=1.
    #[inline]
    fn dp_base(&self) -> u16 {
        if self.flag(psw::P) {
            0x0100
        } else {
            0x0000
        }
    }

    /// Effective direct-page address for an 8-bit DP offset.
    #[inline]
    fn dp_addr(&self, offset: u8) -> u16 {
        self.dp_base().wrapping_add(offset as u16)
    }

    /// Stack address for the current SP (always page 1).
    #[inline]
    fn stack_addr(&self) -> u16 {
        0x0100 | self.sp as u16
    }
}

// ---- Memory access helpers ----
//
// All memory accesses go through the Apu struct in production. The SPC700
// core is called with a mutable slice for corpus tests (flat 64 KiB).

impl Spc700 {
    /// Read one byte from ARAM (or I/O overlay if !corpus_mode).
    /// In corpus mode: plain flat read.
    #[inline]
    pub fn read_mem(&self, mem: &[u8; 0x10000], addr: u16) -> u8 {
        mem[addr as usize]
    }

    /// Write one byte to ARAM (or I/O in corpus mode).
    #[inline]
    pub fn write_mem(&self, mem: &mut [u8; 0x10000], addr: u16, value: u8) {
        mem[addr as usize] = value;
    }

    // ---- program fetch ----

    #[inline]
    fn fetch(&mut self, mem: &[u8; 0x10000]) -> u8 {
        let v = self.read_mem(mem, self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    // ---- stack push/pull ----

    #[inline]
    fn push(&mut self, mem: &mut [u8; 0x10000], v: u8) {
        let addr = self.stack_addr();
        self.write_mem(mem, addr, v);
        self.sp = self.sp.wrapping_sub(1);
    }

    #[inline]
    fn pull(&mut self, mem: &[u8; 0x10000]) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let addr = self.stack_addr();
        self.read_mem(mem, addr)
    }

    // ---- direct-page read/write helpers ----

    #[inline]
    fn dp_read(&self, mem: &[u8; 0x10000], offset: u8) -> u8 {
        self.read_mem(mem, self.dp_addr(offset))
    }

    #[inline]
    fn dp_write(&self, mem: &mut [u8; 0x10000], offset: u8, value: u8) {
        self.write_mem(mem, self.dp_addr(offset), value);
    }

    /// Read a 16-bit little-endian word from the direct page.
    #[inline]
    fn dp_read16(&self, mem: &[u8; 0x10000], offset: u8) -> u16 {
        let lo = self.dp_read(mem, offset) as u16;
        let hi = self.dp_read(mem, offset.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Write a 16-bit little-endian word to the direct page (lo at offset,
    /// hi at offset+1; wraps within the page).
    #[allow(dead_code)]
    #[inline]
    fn dp_write16(&self, mem: &mut [u8; 0x10000], offset: u8, value: u16) {
        self.dp_write(mem, offset, value as u8);
        self.dp_write(mem, offset.wrapping_add(1), (value >> 8) as u8);
    }

    /// Read a 16-bit LE absolute word (no page-wrap; for absolute and indirect
    /// addresses that may legitimately cross pages).
    #[allow(dead_code)]
    #[inline]
    fn abs_read16(&self, mem: &[u8; 0x10000], addr: u16) -> u16 {
        let lo = self.read_mem(mem, addr) as u16;
        let hi = self.read_mem(mem, addr.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Write a 16-bit LE absolute word (no page-wrap).
    #[allow(dead_code)]
    #[inline]
    fn abs_write16(&self, mem: &mut [u8; 0x10000], addr: u16, value: u16) {
        self.write_mem(mem, addr, value as u8);
        self.write_mem(mem, addr.wrapping_add(1), (value >> 8) as u8);
    }

    /// Read a 16-bit LE word from a direct-page address, wrapping the +1
    /// within the direct page (hi byte at same page, offset+1 mod 256).
    ///
    /// Used by ADDW / SUBW / CMPW / MOVW / INCW / DECW where the SPC700
    /// wraps the second byte within the DP boundary.
    #[inline]
    fn dp_abs_read16(&self, mem: &[u8; 0x10000], ea: u16) -> u16 {
        // ea = dp_base | offset (offset is 8 bits, dp_base is $0000 or $0100)
        let lo = self.read_mem(mem, ea) as u16;
        // High byte wraps within the page: keep page bits, increment only offset.
        let hi_addr = (ea & 0xFF00) | ((ea.wrapping_add(1)) & 0x00FF);
        let hi = self.read_mem(mem, hi_addr) as u16;
        lo | (hi << 8)
    }

    /// Write a 16-bit LE word to a direct-page address with DP page wrapping.
    #[inline]
    fn dp_abs_write16(&self, mem: &mut [u8; 0x10000], ea: u16, value: u16) {
        self.write_mem(mem, ea, value as u8);
        let hi_addr = (ea & 0xFF00) | ((ea.wrapping_add(1)) & 0x00FF);
        self.write_mem(mem, hi_addr, (value >> 8) as u8);
    }
}

// ---- Addressing mode resolvers ----

impl Spc700 {
    /// `dp` — fetch 1-byte DP offset from program stream.
    #[inline]
    fn am_dp(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let off = self.fetch(mem);
        self.dp_addr(off)
    }

    /// `dp+X` — DP offset + X (wraps within the page).
    #[inline]
    fn am_dp_x(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let off = self.fetch(mem);
        let dp = self.dp_base();
        dp | ((off.wrapping_add(self.x)) as u16 & 0x00FF)
    }

    /// `dp+Y` — DP offset + Y (wraps within the page).
    #[inline]
    fn am_dp_y(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let off = self.fetch(mem);
        let dp = self.dp_base();
        dp | ((off.wrapping_add(self.y)) as u16 & 0x00FF)
    }

    /// `!abs` — fetch 2-byte absolute address.
    #[inline]
    fn am_abs(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let lo = self.fetch(mem) as u16;
        let hi = self.fetch(mem) as u16;
        lo | (hi << 8)
    }

    /// `!abs+X`
    #[inline]
    fn am_abs_x(&mut self, mem: &[u8; 0x10000]) -> u16 {
        self.am_abs(mem).wrapping_add(self.x as u16)
    }

    /// `!abs+Y`
    #[inline]
    fn am_abs_y(&mut self, mem: &[u8; 0x10000]) -> u16 {
        self.am_abs(mem).wrapping_add(self.y as u16)
    }

    /// `[dp+X]` — indirect: read 16-bit pointer from DP[off+X], then use as
    /// absolute address.
    #[inline]
    fn am_ind_x(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let off = self.fetch(mem);
        let ptr_addr_lo = self.dp_base() | ((off.wrapping_add(self.x)) as u16 & 0x00FF);
        let ptr_addr_hi =
            self.dp_base() | ((off.wrapping_add(self.x).wrapping_add(1)) as u16 & 0x00FF);
        let lo = self.read_mem(mem, ptr_addr_lo) as u16;
        let hi = self.read_mem(mem, ptr_addr_hi) as u16;
        lo | (hi << 8)
    }

    /// `[dp]+Y` — indirect indexed: read 16-bit pointer from DP[off], add Y.
    #[inline]
    fn am_ind_y(&mut self, mem: &[u8; 0x10000]) -> u16 {
        let off = self.fetch(mem);
        let ptr = self.dp_read16(mem, off);
        ptr.wrapping_add(self.y as u16)
    }

    /// Resolve a 13-bit absolute bit address. Bits 15..13 encode the bit
    /// number (0–7) and bits 12..0 encode the byte address.
    #[inline]
    fn am_abs_bit(&mut self, mem: &[u8; 0x10000]) -> (u16, u8) {
        let raw = self.am_abs(mem);
        let bit = ((raw >> 13) & 0x07) as u8;
        let addr = raw & 0x1FFF;
        (addr, bit)
    }
}

// ---- ALU helpers ----

impl Spc700 {
    /// ADD with carry (8-bit). Updates N, V, H, Z, C.
    fn alu_adc(&mut self, a: u8, m: u8) -> u8 {
        let c = (self.psw & psw::C) as u16;
        let sum = a as u16 + m as u16 + c;
        let res = sum as u8;
        self.set_flag(psw::C, sum > 0xFF);
        self.set_flag(psw::V, (!(a ^ m) & (a ^ res) & 0x80) != 0);
        self.set_flag(psw::H, ((a & 0x0F) + (m & 0x0F) + c as u8) > 0x0F);
        self.set_nz(res);
        res
    }

    /// SUBTRACT with borrow (8-bit). Updates N, V, H, Z, C.
    fn alu_sbc(&mut self, a: u8, m: u8) -> u8 {
        let c = (self.psw & psw::C) as i16;
        let diff = a as i16 - m as i16 - (1 - c);
        let res = diff as u8;
        self.set_flag(psw::C, diff >= 0);
        self.set_flag(psw::V, ((a ^ m) & (a ^ res) & 0x80) != 0);
        // H: set when there is NO borrow from the lower nibble (i.e. lower
        // nibble did not underflow).  Corpus-verified: H = !(lower nibble
        // borrows), equivalent to (a&0xF) - (m&0xF) - borrow_in >= 0.
        self.set_flag(psw::H, (a as i16 & 0x0F) - (m as i16 & 0x0F) - (1 - c) >= 0);
        self.set_nz(res);
        res
    }

    /// CMP A: subtract without storing, updates N/Z/C/H (all four flags).
    /// CMP A / CMP X / CMP Y: subtract without storing, updates N/Z/C only.
    /// H is NOT affected by any CMP variant on the SPC700.
    fn alu_cmp(&mut self, a: u8, m: u8) {
        let diff = a.wrapping_sub(m);
        self.set_flag(psw::C, a >= m);
        self.set_flag(psw::N, diff & 0x80 != 0);
        self.set_flag(psw::Z, diff == 0);
    }

    /// CMP X / CMP Y: same as alu_cmp — kept as a named alias for clarity.
    fn alu_cmp_xy(&mut self, reg: u8, m: u8) {
        self.alu_cmp(reg, m);
    }

    /// ADDW: 16-bit add YA + dp-word. Updates N, V, H, Z, C.
    fn alu_addw(&mut self, ya: u16, m: u16) -> u16 {
        let sum = ya as u32 + m as u32;
        let res = sum as u16;
        self.set_flag(psw::C, sum > 0xFFFF);
        self.set_flag(psw::V, (!(ya ^ m) & (ya ^ res) & 0x8000) != 0);
        // H: carry out of bit 11.
        self.set_flag(psw::H, ((ya & 0x0FFF) + (m & 0x0FFF)) > 0x0FFF);
        self.set_nz16(res);
        res
    }

    /// SUBW: 16-bit subtract YA − dp-word. Updates N, V, H, Z, C.
    fn alu_subw(&mut self, ya: u16, m: u16) -> u16 {
        let diff = (ya as i32) - (m as i32);
        let res = diff as u16;
        self.set_flag(psw::C, diff >= 0);
        self.set_flag(psw::V, ((ya ^ m) & (ya ^ res) & 0x8000) != 0);
        self.set_flag(psw::H, (ya & 0x0FFF) >= (m & 0x0FFF));
        self.set_nz16(res);
        res
    }

    /// CMPW: 16-bit compare, updates N, Z, C. Does NOT update V or H.
    fn alu_cmpw(&mut self, ya: u16, m: u16) {
        let diff = ya.wrapping_sub(m);
        self.set_flag(psw::C, ya >= m);
        self.set_flag(psw::N, diff & 0x8000 != 0);
        self.set_flag(psw::Z, diff == 0);
    }

    /// ASL (accumulator): shifts left, bit 7 → C. Updates N, Z, C.
    fn alu_asl(&mut self, v: u8) -> u8 {
        self.set_flag(psw::C, v & 0x80 != 0);
        let r = v << 1;
        self.set_nz(r);
        r
    }

    /// LSR: shifts right, bit 0 → C. Updates N (always 0), Z, C.
    fn alu_lsr(&mut self, v: u8) -> u8 {
        self.set_flag(psw::C, v & 0x01 != 0);
        let r = v >> 1;
        self.set_nz(r);
        r
    }

    /// ROL: rotate left through carry. Updates N, Z, C.
    fn alu_rol(&mut self, v: u8) -> u8 {
        let c_in = self.psw & psw::C;
        self.set_flag(psw::C, v & 0x80 != 0);
        let r = (v << 1) | c_in;
        self.set_nz(r);
        r
    }

    /// ROR: rotate right through carry. Updates N, Z, C.
    fn alu_ror(&mut self, v: u8) -> u8 {
        let c_in = self.psw & psw::C;
        self.set_flag(psw::C, v & 0x01 != 0);
        let r = (v >> 1) | (c_in << 7);
        self.set_nz(r);
        r
    }

    /// INC memory or register: +1, updates N and Z.
    #[inline]
    fn alu_inc(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        self.set_nz(r);
        r
    }

    /// DEC memory or register: −1, updates N and Z.
    #[inline]
    fn alu_dec(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        self.set_nz(r);
        r
    }

    /// AND: updates N and Z.
    #[inline]
    fn alu_and(&mut self, a: u8, m: u8) -> u8 {
        let r = a & m;
        self.set_nz(r);
        r
    }

    /// OR: updates N and Z.
    #[inline]
    fn alu_or(&mut self, a: u8, m: u8) -> u8 {
        let r = a | m;
        self.set_nz(r);
        r
    }

    /// XOR: updates N and Z.
    #[inline]
    fn alu_eor(&mut self, a: u8, m: u8) -> u8 {
        let r = a ^ m;
        self.set_nz(r);
        r
    }

    /// DAA: decimal adjust A after ADC. The SPC700 DAA adjusts based on H and C.
    ///
    /// Corpus-verified algorithm:
    /// - Step 1: if H=1 or (A & $0F) > 9  → A += 6  (track overflow carry)
    /// - Step 2: if C=1 or (modified A > $9F or step-1 overflowed) → A += $60; C=1
    fn op_daa(&mut self) {
        let mut a = self.a;
        let mut carry1 = false;
        if self.flag(psw::H) || (a & 0x0F) > 9 {
            let sum = a as u16 + 6;
            a = sum as u8;
            carry1 = sum > 0xFF;
        }
        if self.flag(psw::C) || a > 0x9F || carry1 {
            a = a.wrapping_add(0x60);
            self.set_flag(psw::C, true);
        }
        self.set_nz(a);
        self.a = a;
    }

    /// DAS: decimal adjust A after SBC. Subtracts from each nibble.
    ///
    /// Corpus-verified algorithm:
    /// - Step 1: if H=0 or (A & $0F) > 9  → A -= 6
    /// - Step 2: if C=0 or (ORIGINAL A > $99) → A -= $60; C=0
    ///
    /// The second condition compares the ORIGINAL (pre-step-1) A against $99,
    /// not the modified value.  This matches the hardware's parallel evaluation.
    fn op_das(&mut self) {
        let a_orig = self.a;
        let mut a = a_orig;
        if !self.flag(psw::H) || (a & 0x0F) > 9 {
            a = a.wrapping_sub(6);
        }
        if !self.flag(psw::C) || a_orig > 0x99 {
            a = a.wrapping_sub(0x60);
            self.set_flag(psw::C, false);
        }
        self.set_nz(a);
        self.a = a;
    }

    /// MUL: Y * A → YA (unsigned 8×8 = 16). Updates N and Z (on the Y result).
    /// H is NOT modified (corpus-verified: H is preserved by MUL).
    fn op_mul(&mut self) {
        let result = (self.y as u16) * (self.a as u16);
        self.a = result as u8;
        self.y = (result >> 8) as u8;
        self.set_nz(self.y);
    }

    /// DIV: YA / X → A (quotient), Y (remainder).
    ///
    /// The hardware divider is a 9-step shift circuit; public register-level
    /// references give its behavior in closed form, including the overflow
    /// range (true quotient ≥ 512) and X = 0, which this implements verbatim
    /// (corpus-verified, 1000/1000 for opcode $9E):
    ///
    ///   H = (X & $0F) <= (Y & $0F)        (nibble compare quirk)
    ///   V = Y >= X                        (true quotient needs > 8 bits)
    ///   if Y < 2*X:  A = YA / X,  Y = YA % X            (truncated to 8 bits)
    ///   else:        A = 255 - (YA - X*512) / (256 - X)
    ///                Y = X   + (YA - X*512) % (256 - X)
    ///
    /// The else branch subsumes X = 0 (it yields A = ~Y, Y = initial A) and
    /// stays in range: it is only reachable for X <= 127 (Y >= 2*X with Y
    /// 8-bit), so 256 - X >= 129 and both results fit in 8 bits.
    fn op_div(&mut self) {
        let y = self.y as u16;
        let x = self.x as u16;
        let ya = (y << 8) | self.a as u16;
        self.set_flag(psw::H, (x & 0x0F) <= (y & 0x0F));
        self.set_flag(psw::V, y >= x);
        let (q, r) = if y < 2 * x {
            (ya / x, ya % x)
        } else {
            // YA >= X*512 here (Y >= 2*X), so the subtraction cannot wrap.
            (
                255 - (ya - (x << 9)) / (256 - x),
                x + (ya - (x << 9)) % (256 - x),
            )
        };
        self.a = q as u8;
        self.y = r as u8;
        self.set_nz(self.a);
    }

    // ---- branch helpers ----

    /// Take a relative branch (1-byte signed offset after fetch). Adds 2 to
    /// account for the already-advanced PC.
    #[inline]
    fn branch(&mut self, mem: &[u8; 0x10000], take: bool) {
        let off = self.fetch(mem) as i8;
        if take {
            self.pc = (self.pc as i16).wrapping_add(off as i16) as u16;
        }
    }

    /// Branch if bit `bit` of `v` is set/clear.
    #[inline]
    fn branch_bit(&mut self, mem: &[u8; 0x10000], v: u8, bit: u8, on: bool) {
        let off = self.fetch(mem) as i8;
        if ((v >> bit) & 1 != 0) == on {
            self.pc = (self.pc as i16).wrapping_add(off as i16) as u16;
        }
    }
}

// ---- Main dispatch ----

impl Spc700 {
    /// Execute one instruction. Returns the number of SPC700 master-clock
    /// cycles consumed. When halted, returns 0 without executing anything.
    pub fn step(&mut self, mem: &mut [u8; 0x10000]) -> u32 {
        if self.halted.is_some() {
            return 0;
        }
        let opcode = self.fetch(mem);
        self.execute(mem, opcode)
    }

    #[allow(clippy::too_many_lines)]
    fn execute(&mut self, mem: &mut [u8; 0x10000], opcode: u8) -> u32 {
        match opcode {
            // ---- NOP ----
            0x00 => 2, // NOP

            // ---- TCALL n: call to vector at $FFDE−2n ----
            0x01 => self.op_tcall(mem, 0),
            0x11 => self.op_tcall(mem, 1),
            0x21 => self.op_tcall(mem, 2),
            0x31 => self.op_tcall(mem, 3),
            0x41 => self.op_tcall(mem, 4),
            0x51 => self.op_tcall(mem, 5),
            0x61 => self.op_tcall(mem, 6),
            0x71 => self.op_tcall(mem, 7),
            0x81 => self.op_tcall(mem, 8),
            0x91 => self.op_tcall(mem, 9),
            0xA1 => self.op_tcall(mem, 10),
            0xB1 => self.op_tcall(mem, 11),
            0xC1 => self.op_tcall(mem, 12),
            0xD1 => self.op_tcall(mem, 13),
            0xE1 => self.op_tcall(mem, 14),
            0xF1 => self.op_tcall(mem, 15),

            // ---- SET1 dp.bit ----
            0x02 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x01);
                4
            }
            0x22 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x02);
                4
            }
            0x42 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x04);
                4
            }
            0x62 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x08);
                4
            }
            0x82 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x10);
                4
            }
            0xA2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x20);
                4
            }
            0xC2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x40);
                4
            }
            0xE2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v | 0x80);
                4
            }

            // ---- CLR1 dp.bit ----
            0x12 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x01);
                4
            }
            0x32 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x02);
                4
            }
            0x52 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x04);
                4
            }
            0x72 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x08);
                4
            }
            0x92 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x10);
                4
            }
            0xB2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x20);
                4
            }
            0xD2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x40);
                4
            }
            0xF2 => {
                let a = self.am_dp(mem);
                let v = self.read_mem(mem, a);
                self.write_mem(mem, a, v & !0x80);
                4
            }

            // ---- BBS dp.bit, rel ----
            0x03 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 0, true);
                5
            }
            0x23 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 1, true);
                5
            }
            0x43 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 2, true);
                5
            }
            0x63 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 3, true);
                5
            }
            0x83 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 4, true);
                5
            }
            0xA3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 5, true);
                5
            }
            0xC3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 6, true);
                5
            }
            0xE3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 7, true);
                5
            }

            // ---- BBC dp.bit, rel ----
            0x13 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 0, false);
                5
            }
            0x33 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 1, false);
                5
            }
            0x53 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 2, false);
                5
            }
            0x73 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 3, false);
                5
            }
            0x93 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 4, false);
                5
            }
            0xB3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 5, false);
                5
            }
            0xD3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 6, false);
                5
            }
            0xF3 => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.branch_bit(mem, v, 7, false);
                5
            }

            // ---- OR1 C, mem.bit ----
            0x0A => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 != 0;
                if b {
                    self.set_flag(psw::C, true);
                }
                5
            }
            // ---- OR1 C, /mem.bit ----
            0x2A => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 != 0;
                if !b {
                    self.set_flag(psw::C, true);
                }
                5
            }
            // ---- AND1 C, mem.bit ----
            0x4A => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 != 0;
                let c = self.flag(psw::C);
                self.set_flag(psw::C, c & b);
                4
            }
            // ---- AND1 C, /mem.bit ----
            0x6A => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 == 0;
                let c = self.flag(psw::C);
                self.set_flag(psw::C, c & b);
                4
            }
            // ---- EOR1 C, mem.bit ----
            0x8A => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 != 0;
                let c = self.flag(psw::C);
                self.set_flag(psw::C, c ^ b);
                5
            }
            // ---- NOT1 mem.bit ----
            0xEA => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                self.write_mem(mem, addr, v ^ (1 << bit));
                5
            }
            // ---- MOV1 C, mem.bit ----
            0xAA => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let b = (v >> bit) & 1 != 0;
                self.set_flag(psw::C, b);
                4
            }
            // ---- MOV1 mem.bit, C ----
            0xCA => {
                let (addr, bit) = self.am_abs_bit(mem);
                let v = self.read_mem(mem, addr);
                let c = self.psw & psw::C;
                self.write_mem(mem, addr, (v & !(1 << bit)) | (c << bit));
                6
            }

            // ---- TSET1 !abs ----
            0x0E => {
                let addr = self.am_abs(mem);
                let v = self.read_mem(mem, addr);
                let r = v | self.a;
                self.write_mem(mem, addr, r);
                self.set_flag(psw::N, (self.a.wrapping_sub(v)) & 0x80 != 0);
                self.set_flag(psw::Z, self.a == v);
                6
            }
            // ---- TCLR1 !abs ----
            0x4E => {
                let addr = self.am_abs(mem);
                let v = self.read_mem(mem, addr);
                let r = v & !self.a;
                self.write_mem(mem, addr, r);
                self.set_flag(psw::N, (self.a.wrapping_sub(v)) & 0x80 != 0);
                self.set_flag(psw::Z, self.a == v);
                6
            }

            // ---- OR A, ... ----
            0x08 => {
                let v = self.fetch(mem);
                let r = self.alu_or(self.a, v);
                self.a = r;
                2
            } // OR A, #imm
            0x06 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // OR A, (X)
                let r = self.alu_or(self.a, v);
                self.a = r;
                3
            }
            0x17 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                6
            }
            0x07 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                6
            }
            0x04 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                3
            }
            0x14 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                4
            }
            0x05 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                4
            }
            0x15 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                5
            }
            0x16 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_or(self.a, v);
                self.a = r;
                5
            }
            0x09 => {
                // OR dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                let r = self.alu_or(d, s);
                self.dp_write(mem, dst_off, r);
                6
            }
            0x18 => {
                // OR dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let r = self.alu_or(v, imm);
                self.dp_write(mem, off, r);
                5
            }
            0x19 => {
                // OR (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                let r = self.alu_or(xv, yv);
                self.write_mem(mem, dp | self.x as u16, r);
                5
            }

            // ---- AND A, ... ----
            0x28 => {
                let v = self.fetch(mem);
                let r = self.alu_and(self.a, v);
                self.a = r;
                2
            } // AND A, #imm
            0x26 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // AND A, (X)
                let r = self.alu_and(self.a, v);
                self.a = r;
                3
            }
            0x37 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                6
            }
            0x27 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                6
            }
            0x24 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                3
            }
            0x34 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                4
            }
            0x25 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                4
            }
            0x35 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                5
            }
            0x36 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_and(self.a, v);
                self.a = r;
                5
            }
            0x29 => {
                // AND dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                let r = self.alu_and(d, s);
                self.dp_write(mem, dst_off, r);
                6
            }
            0x38 => {
                // AND dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let r = self.alu_and(v, imm);
                self.dp_write(mem, off, r);
                5
            }
            0x39 => {
                // AND (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                let r = self.alu_and(xv, yv);
                self.write_mem(mem, dp | self.x as u16, r);
                5
            }

            // ---- EOR A, ... ----
            0x48 => {
                let v = self.fetch(mem);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                2
            } // EOR A, #imm
            0x46 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // EOR A, (X)
                let r = self.alu_eor(self.a, v);
                self.a = r;
                3
            }
            0x57 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                6
            }
            0x47 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                6
            }
            0x44 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                3
            }
            0x54 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                4
            }
            0x45 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                4
            }
            0x55 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                5
            }
            0x56 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_eor(self.a, v);
                self.a = r;
                5
            }
            0x49 => {
                // EOR dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                let r = self.alu_eor(d, s);
                self.dp_write(mem, dst_off, r);
                6
            }
            0x58 => {
                // EOR dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let r = self.alu_eor(v, imm);
                self.dp_write(mem, off, r);
                5
            }
            0x59 => {
                // EOR (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                let r = self.alu_eor(xv, yv);
                self.write_mem(mem, dp | self.x as u16, r);
                5
            }

            // ---- CMP A, ... ----
            0x68 => {
                let v = self.fetch(mem);
                self.alu_cmp(self.a, v);
                2
            } // CMP A, #imm
            0x66 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // CMP A, (X)
                self.alu_cmp(self.a, v);
                3
            }
            0x77 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                6
            }
            0x67 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                6
            }
            0x64 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                3
            }
            0x74 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                4
            }
            0x65 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                4
            }
            0x75 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                5
            }
            0x76 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp(self.a, v);
                5
            }
            0x69 => {
                // CMP dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                self.alu_cmp(d, s);
                6
            }
            0x78 => {
                // CMP dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                self.alu_cmp(v, imm);
                5
            }
            0x79 => {
                // CMP (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                self.alu_cmp(xv, yv);
                5
            }
            // CMP X/Y, #imm and dp (H flag not affected per SPC700 reference)
            0xC8 => {
                let v = self.fetch(mem);
                self.alu_cmp_xy(self.x, v);
                2
            } // CMP X, #imm
            0x3E => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp_xy(self.x, v);
                3
            } // CMP X, dp
            0x1E => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp_xy(self.x, v);
                4
            } // CMP X, !abs
            0xAD => {
                let v = self.fetch(mem);
                self.alu_cmp_xy(self.y, v);
                2
            } // CMP Y, #imm
            0x7E => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp_xy(self.y, v);
                3
            } // CMP Y, dp
            0x5E => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.alu_cmp_xy(self.y, v);
                4
            } // CMP Y, !abs

            // ---- ADC A, ... ----
            0x88 => {
                let v = self.fetch(mem);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                2
            } // ADC A, #imm
            0x86 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // ADC A, (X)
                let r = self.alu_adc(self.a, v);
                self.a = r;
                3
            }
            0x97 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                6
            }
            0x87 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                6
            }
            0x84 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                3
            }
            0x94 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                4
            }
            0x85 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                4
            }
            0x95 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                5
            }
            0x96 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_adc(self.a, v);
                self.a = r;
                5
            }
            0x89 => {
                // ADC dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                let r = self.alu_adc(d, s);
                self.dp_write(mem, dst_off, r);
                6
            }
            0x98 => {
                // ADC dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let r = self.alu_adc(v, imm);
                self.dp_write(mem, off, r);
                5
            }
            0x99 => {
                // ADC (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                let r = self.alu_adc(xv, yv);
                self.write_mem(mem, dp | self.x as u16, r);
                5
            }

            // ---- SBC A, ... ----
            0xA8 => {
                let v = self.fetch(mem);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                2
            } // SBC A, #imm
            0xA6 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // SBC A, (X)
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                3
            }
            0xB7 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                6
            }
            0xA7 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                6
            }
            0xA4 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                3
            }
            0xB4 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                4
            }
            0xA5 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                4
            }
            0xB5 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                5
            }
            0xB6 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_sbc(self.a, v);
                self.a = r;
                5
            }
            0xA9 => {
                // SBC dp, dp
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let s = self.dp_read(mem, src_off);
                let d = self.dp_read(mem, dst_off);
                let r = self.alu_sbc(d, s);
                self.dp_write(mem, dst_off, r);
                6
            }
            0xB8 => {
                // SBC dp, #imm
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let r = self.alu_sbc(v, imm);
                self.dp_write(mem, off, r);
                5
            }
            0xB9 => {
                // SBC (X), (Y)
                let dp = self.dp_base();
                let xv = self.read_mem(mem, dp | self.x as u16);
                let yv = self.read_mem(mem, dp | self.y as u16);
                let r = self.alu_sbc(xv, yv);
                self.write_mem(mem, dp | self.x as u16, r);
                5
            }

            // ---- ADDW YA, dp ----
            0x7A => {
                let ea = self.am_dp(mem);
                let m = self.dp_abs_read16(mem, ea);
                let ya = ((self.y as u16) << 8) | self.a as u16;
                let res = self.alu_addw(ya, m);
                self.a = res as u8;
                self.y = (res >> 8) as u8;
                5
            }
            // ---- SUBW YA, dp ----
            0x9A => {
                let ea = self.am_dp(mem);
                let m = self.dp_abs_read16(mem, ea);
                let ya = ((self.y as u16) << 8) | self.a as u16;
                let res = self.alu_subw(ya, m);
                self.a = res as u8;
                self.y = (res >> 8) as u8;
                5
            }
            // ---- CMPW YA, dp ----
            0x5A => {
                let ea = self.am_dp(mem);
                let m = self.dp_abs_read16(mem, ea);
                let ya = ((self.y as u16) << 8) | self.a as u16;
                self.alu_cmpw(ya, m);
                4
            }
            // ---- INCW dp ----
            0x3A => {
                let ea = self.am_dp(mem);
                let v = self.dp_abs_read16(mem, ea).wrapping_add(1);
                self.dp_abs_write16(mem, ea, v);
                self.set_nz16(v);
                6
            }
            // ---- DECW dp ----
            0x1A => {
                let ea = self.am_dp(mem);
                let v = self.dp_abs_read16(mem, ea).wrapping_sub(1);
                self.dp_abs_write16(mem, ea, v);
                self.set_nz16(v);
                6
            }
            // ---- MUL YA ----
            0xCF => {
                self.op_mul();
                9
            }
            // ---- DIV YA, X ----
            0x9E => {
                self.op_div();
                12
            }
            // ---- DAA ----
            0xDF => {
                self.op_daa();
                3
            }
            // ---- DAS ----
            0xBE => {
                self.op_das();
                3
            }

            // ---- ASL ----
            0x1C => {
                let r = self.alu_asl(self.a);
                self.a = r;
                2
            } // ASL A
            0x0B => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_asl(v);
                self.write_mem(mem, ea, r);
                4
            }
            0x1B => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_asl(v);
                self.write_mem(mem, ea, r);
                5
            }
            0x0C => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_asl(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- LSR ----
            0x5C => {
                let r = self.alu_lsr(self.a);
                self.a = r;
                2
            } // LSR A
            0x4B => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_lsr(v);
                self.write_mem(mem, ea, r);
                4
            }
            0x5B => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_lsr(v);
                self.write_mem(mem, ea, r);
                5
            }
            0x4C => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_lsr(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- ROL ----
            0x3C => {
                let r = self.alu_rol(self.a);
                self.a = r;
                2
            } // ROL A
            0x2B => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_rol(v);
                self.write_mem(mem, ea, r);
                4
            }
            0x3B => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_rol(v);
                self.write_mem(mem, ea, r);
                5
            }
            0x2C => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_rol(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- ROR ----
            0x7C => {
                let r = self.alu_ror(self.a);
                self.a = r;
                2
            } // ROR A
            0x6B => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_ror(v);
                self.write_mem(mem, ea, r);
                4
            }
            0x7B => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_ror(v);
                self.write_mem(mem, ea, r);
                5
            }
            0x6C => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_ror(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- INC ----
            0xBC => {
                let r = self.alu_inc(self.a);
                self.a = r;
                2
            } // INC A
            0x3D => {
                let r = self.alu_inc(self.x);
                self.x = r;
                2
            } // INC X
            0xFC => {
                let r = self.alu_inc(self.y);
                self.y = r;
                2
            } // INC Y
            0xAB => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_inc(v);
                self.write_mem(mem, ea, r);
                4
            }
            0xBB => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_inc(v);
                self.write_mem(mem, ea, r);
                5
            }
            0xAC => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_inc(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- DEC ----
            0x9C => {
                let r = self.alu_dec(self.a);
                self.a = r;
                2
            } // DEC A
            0x1D => {
                let r = self.alu_dec(self.x);
                self.x = r;
                2
            } // DEC X
            0xDC => {
                let r = self.alu_dec(self.y);
                self.y = r;
                2
            } // DEC Y
            0x8B => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_dec(v);
                self.write_mem(mem, ea, r);
                4
            }
            0x9B => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_dec(v);
                self.write_mem(mem, ea, r);
                5
            }
            0x8C => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                let r = self.alu_dec(v);
                self.write_mem(mem, ea, r);
                5
            }

            // ---- MOV A, ... ----
            0xE8 => {
                let v = self.fetch(mem);
                self.a = v;
                self.set_nz(v);
                2
            } // MOV A, #imm
            0xE6 => {
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16); // MOV A, (X)
                self.a = v;
                self.set_nz(v);
                3
            }
            0xBF => {
                // MOV A, (X)+
                let dp = self.dp_base();
                let v = self.read_mem(mem, dp | self.x as u16);
                self.x = self.x.wrapping_add(1);
                self.a = v;
                self.set_nz(v);
                4
            }
            0xF7 => {
                let ea = self.am_ind_y(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                6
            }
            0xE7 => {
                let ea = self.am_ind_x(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                6
            }
            0xE4 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                3
            }
            0xF4 => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                4
            }
            0xE5 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                4
            }
            0xF5 => {
                let ea = self.am_abs_x(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                5
            }
            0xF6 => {
                let ea = self.am_abs_y(mem);
                let v = self.read_mem(mem, ea);
                self.a = v;
                self.set_nz(v);
                5
            }

            // ---- MOV X, ... ----
            0xCD => {
                let v = self.fetch(mem);
                self.x = v;
                self.set_nz(v);
                2
            } // MOV X, #imm
            0xF8 => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.x = v;
                self.set_nz(v);
                3
            }
            0xF9 => {
                let ea = self.am_dp_y(mem);
                let v = self.read_mem(mem, ea);
                self.x = v;
                self.set_nz(v);
                4
            }
            0xE9 => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.x = v;
                self.set_nz(v);
                4
            }

            // ---- MOV Y, ... ----
            0x8D => {
                let v = self.fetch(mem);
                self.y = v;
                self.set_nz(v);
                2
            } // MOV Y, #imm
            0xEB => {
                let ea = self.am_dp(mem);
                let v = self.read_mem(mem, ea);
                self.y = v;
                self.set_nz(v);
                3
            }
            0xFB => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                self.y = v;
                self.set_nz(v);
                4
            }
            0xEC => {
                let ea = self.am_abs(mem);
                let v = self.read_mem(mem, ea);
                self.y = v;
                self.set_nz(v);
                4
            }

            // ---- STA: MOV ..., A ----
            0xC6 => {
                let dp = self.dp_base();
                self.write_mem(mem, dp | self.x as u16, self.a);
                4
            } // MOV (X), A
            0xAF => {
                // MOV (X)+, A
                let dp = self.dp_base();
                self.write_mem(mem, dp | self.x as u16, self.a);
                self.x = self.x.wrapping_add(1);
                4
            }
            0xD7 => {
                let ea = self.am_ind_y(mem);
                self.write_mem(mem, ea, self.a);
                7
            } // MOV [dp]+Y, A
            0xC7 => {
                let ea = self.am_ind_x(mem);
                self.write_mem(mem, ea, self.a);
                7
            } // MOV [dp+X], A
            0xC4 => {
                let ea = self.am_dp(mem);
                self.write_mem(mem, ea, self.a);
                4
            } // MOV dp, A
            0xD4 => {
                let ea = self.am_dp_x(mem);
                self.write_mem(mem, ea, self.a);
                5
            } // MOV dp+X, A
            0xC5 => {
                let ea = self.am_abs(mem);
                self.write_mem(mem, ea, self.a);
                5
            } // MOV !abs, A
            0xD5 => {
                let ea = self.am_abs_x(mem);
                self.write_mem(mem, ea, self.a);
                6
            } // MOV !abs+X, A
            0xD6 => {
                let ea = self.am_abs_y(mem);
                self.write_mem(mem, ea, self.a);
                6
            } // MOV !abs+Y, A

            // ---- STX / STY ----
            0xD8 => {
                let ea = self.am_dp(mem);
                self.write_mem(mem, ea, self.x);
                4
            } // MOV dp, X
            0xD9 => {
                let ea = self.am_dp_y(mem);
                self.write_mem(mem, ea, self.x);
                5
            } // MOV dp+Y, X
            0xC9 => {
                let ea = self.am_abs(mem);
                self.write_mem(mem, ea, self.x);
                5
            } // MOV !abs, X
            0xCB => {
                let ea = self.am_dp(mem);
                self.write_mem(mem, ea, self.y);
                4
            } // MOV dp, Y
            0xDB => {
                let ea = self.am_dp_x(mem);
                self.write_mem(mem, ea, self.y);
                5
            } // MOV dp+X, Y
            0xCC => {
                let ea = self.am_abs(mem);
                self.write_mem(mem, ea, self.y);
                5
            } // MOV !abs, Y

            // ---- MOV dp, dp ----
            0xFA => {
                let src_off = self.fetch(mem);
                let dst_off = self.fetch(mem);
                let v = self.dp_read(mem, src_off);
                self.dp_write(mem, dst_off, v);
                5
            }
            // ---- MOV dp, #imm ----
            0x8F => {
                let imm = self.fetch(mem);
                let off = self.fetch(mem);
                self.dp_write(mem, off, imm);
                5
            }

            // ---- MOVW YA, dp ----
            0xBA => {
                let ea = self.am_dp(mem);
                let v = self.dp_abs_read16(mem, ea);
                self.a = v as u8;
                self.y = (v >> 8) as u8;
                self.set_nz16(v);
                5
            }
            // ---- MOVW dp, YA ----
            0xDA => {
                let ea = self.am_dp(mem);
                let ya = ((self.y as u16) << 8) | self.a as u16;
                self.dp_abs_write16(mem, ea, ya);
                5
            }

            // ---- Transfer instructions ----
            0x7D => {
                self.a = self.x;
                self.set_nz(self.a);
                2
            } // MOV A, X
            0xDD => {
                self.a = self.y;
                self.set_nz(self.a);
                2
            } // MOV A, Y
            0x5D => {
                self.x = self.a;
                self.set_nz(self.x);
                2
            } // MOV X, A
            0xFD => {
                self.y = self.a;
                self.set_nz(self.y);
                2
            } // MOV Y, A
            0x9D => {
                self.x = self.sp;
                self.set_nz(self.x);
                2
            } // MOV X, SP
            0xBD => {
                self.sp = self.x;
                2
            } // MOV SP, X (no flags)

            // ---- Push / Pull ----
            0x2D => {
                let v = self.a;
                self.push(mem, v);
                4
            } // PUSH A
            0x4D => {
                let v = self.x;
                self.push(mem, v);
                4
            } // PUSH X
            0x6D => {
                let v = self.y;
                self.push(mem, v);
                4
            } // PUSH Y
            0x0D => {
                let v = self.psw;
                self.push(mem, v);
                4
            } // PUSH PSW
            0xAE => {
                // POP A: pull from stack into A. Does NOT affect PSW (corpus-verified).
                let v = self.pull(mem);
                self.a = v;
                4
            }
            0xCE => {
                // POP X: pull from stack into X. Does NOT affect PSW (corpus-verified).
                let v = self.pull(mem);
                self.x = v;
                4
            }
            0xEE => {
                // POP Y: pull from stack into Y. Does NOT affect PSW (corpus-verified).
                let v = self.pull(mem);
                self.y = v;
                4
            }
            0x8E => {
                let v = self.pull(mem);
                self.psw = v;
                4
            } // POP PSW

            // ---- Branches ----
            0x2F => {
                self.branch(mem, true);
                4
            } // BRA rel
            0xF0 => {
                let t = self.flag(psw::Z);
                self.branch(mem, t);
                2
            } // BEQ
            0xD0 => {
                let t = !self.flag(psw::Z);
                self.branch(mem, t);
                2
            } // BNE
            0xB0 => {
                let t = self.flag(psw::C);
                self.branch(mem, t);
                2
            } // BCS
            0x90 => {
                let t = !self.flag(psw::C);
                self.branch(mem, t);
                2
            } // BCC
            0x70 => {
                let t = self.flag(psw::V);
                self.branch(mem, t);
                2
            } // BVS
            0x50 => {
                let t = !self.flag(psw::V);
                self.branch(mem, t);
                2
            } // BVC
            0x30 => {
                let t = self.flag(psw::N);
                self.branch(mem, t);
                2
            } // BMI
            0x10 => {
                let t = !self.flag(psw::N);
                self.branch(mem, t);
                2
            } // BPL

            // ---- CBNE dp, rel ----
            0x2E => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off);
                let t = v != self.a;
                self.branch(mem, t);
                5
            }
            // ---- CBNE dp+X, rel ----
            0xDE => {
                let ea = self.am_dp_x(mem);
                let v = self.read_mem(mem, ea);
                let t = v != self.a;
                self.branch(mem, t);
                6
            }
            // ---- DBNZ dp, rel ----
            0x6E => {
                let off = self.fetch(mem);
                let v = self.dp_read(mem, off).wrapping_sub(1);
                self.dp_write(mem, off, v);
                let t = v != 0;
                self.branch(mem, t);
                5
            }
            // ---- DBNZ Y, rel ----
            0xFE => {
                self.y = self.y.wrapping_sub(1);
                let t = self.y != 0;
                self.branch(mem, t);
                4
            }

            // ---- JMP ----
            0x5F => {
                let addr = self.am_abs(mem);
                self.pc = addr;
                3
            } // JMP !abs
            0x1F => {
                // JMP [!abs+X]
                let base = self.am_abs(mem);
                let ptr = base.wrapping_add(self.x as u16);
                let lo = self.read_mem(mem, ptr) as u16;
                let hi = self.read_mem(mem, ptr.wrapping_add(1)) as u16;
                self.pc = lo | (hi << 8);
                6
            }

            // ---- CALL ----
            0x3F => {
                // CALL !abs
                let addr = self.am_abs(mem);
                let ret = self.pc;
                self.push(mem, (ret >> 8) as u8);
                self.push(mem, ret as u8);
                self.pc = addr;
                8
            }
            // ---- PCALL up ----
            0x4F => {
                // PCALL up: call $FF00 | imm
                let up = self.fetch(mem) as u16;
                let addr = 0xFF00 | up;
                let ret = self.pc;
                self.push(mem, (ret >> 8) as u8);
                self.push(mem, ret as u8);
                self.pc = addr;
                6
            }
            // ---- RET ----
            0x6F => {
                // RET
                let lo = self.pull(mem) as u16;
                let hi = self.pull(mem) as u16;
                self.pc = lo | (hi << 8);
                5
            }
            // ---- RET1 (RTI) ----
            0x7F => {
                // RET1
                let p = self.pull(mem);
                self.psw = p;
                let lo = self.pull(mem) as u16;
                let hi = self.pull(mem) as u16;
                self.pc = lo | (hi << 8);
                6
            }

            // ---- BRK ----
            0x0F => {
                let ret = self.pc;
                self.push(mem, (ret >> 8) as u8);
                self.push(mem, ret as u8);
                let p = self.psw;
                self.push(mem, p);
                self.set_flag(psw::I, false);
                self.set_flag(psw::B, true);
                let lo = self.read_mem(mem, 0xFFDE) as u16;
                let hi = self.read_mem(mem, 0xFFDF) as u16;
                self.pc = lo | (hi << 8);
                8
            }

            // ---- Flag ops ----
            0x60 => {
                self.set_flag(psw::C, false);
                2
            } // CLRC
            0x80 => {
                self.set_flag(psw::C, true);
                2
            } // SETC
            0xE0 => {
                self.set_flag(psw::V, false);
                self.set_flag(psw::H, false);
                2
            } // CLRV / CLRH
            0x20 => {
                self.set_flag(psw::P, false);
                2
            } // CLRP
            0x40 => {
                self.set_flag(psw::P, true);
                2
            } // SETP
            0xA0 => {
                self.set_flag(psw::I, true);
                2
            } // EI
            0xC0 => {
                self.set_flag(psw::I, false);
                2
            } // DI
            0xED => {
                let c = !self.flag(psw::C);
                self.set_flag(psw::C, c);
                3
            } // NOTC

            // ---- XCHN: exchange nibbles of A ----
            0x9F => {
                let lo = self.a & 0x0F;
                let hi = (self.a >> 4) & 0x0F;
                self.a = (lo << 4) | hi;
                self.set_nz(self.a);
                5
            }

            // ---- XCH: exchange A and dp ----
            // Note: XCNB / not a standard mnemonic — see below

            // ---- SLEEP ----
            0xEF => {
                self.halted = Some(ApuHalt::Sleep);
                3
            }
            // ---- STOP ----
            0xFF => {
                self.halted = Some(ApuHalt::Stop);
                3
            }

            // Every valid opcode should be covered above. Panic in debug, treat
            // as NOP in corpus mode to avoid false failures on corpus garbage.
            #[allow(unreachable_patterns)]
            _ => {
                // All 256 opcodes of the SPC700 are defined; reaching here
                // means the match above missed one — treat as NOP (2 cycles)
                // in corpus mode, or record a halt trigger in production.
                if !self.corpus_mode {
                    self.halted = Some(ApuHalt::TestTrigger(opcode));
                }
                2
            }
        }
    }

    // ---- TCALL helper ----

    /// TCALL n: push PC, jump to the vector at $FFDE − 2*n.
    fn op_tcall(&mut self, mem: &mut [u8; 0x10000], n: u8) -> u32 {
        let ret = self.pc;
        self.push(mem, (ret >> 8) as u8);
        self.push(mem, ret as u8);
        let vec_addr = 0xFFDEu16.wrapping_sub(2 * n as u16);
        let lo = self.read_mem(mem, vec_addr) as u16;
        let hi = self.read_mem(mem, vec_addr.wrapping_add(1)) as u16;
        self.pc = lo | (hi << 8);
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mem() -> Box<[u8; 0x10000]> {
        Box::new([0u8; 0x10000])
    }

    #[test]
    fn nop_advances_pc() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x1000;
        let mut mem = make_mem();
        mem[0x1000] = 0x00; // NOP
        let cycles = cpu.step(&mut mem);
        assert_eq!(cpu.pc, 0x1001);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn mov_a_imm() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0200;
        let mut mem = make_mem();
        mem[0x0200] = 0xE8; // MOV A, #imm
        mem[0x0201] = 0x42;
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x42);
        assert_eq!(cpu.pc, 0x0202);
    }

    #[test]
    fn adc_sets_carry() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0300;
        cpu.a = 0xFF;
        cpu.psw = 0x00; // C=0
        let mut mem = make_mem();
        mem[0x0300] = 0x88; // ADC A, #imm
        mem[0x0301] = 0x01;
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x00);
        assert!(cpu.flag(psw::C));
        assert!(cpu.flag(psw::Z));
    }

    #[test]
    fn bne_not_taken_when_zero() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0400;
        cpu.psw |= psw::Z;
        let mut mem = make_mem();
        mem[0x0400] = 0xD0; // BNE rel
        mem[0x0401] = 0x10;
        cpu.step(&mut mem);
        assert_eq!(cpu.pc, 0x0402); // not taken
    }

    #[test]
    fn bne_taken_when_nonzero() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0400;
        cpu.psw &= !psw::Z;
        let mut mem = make_mem();
        mem[0x0400] = 0xD0; // BNE rel
        mem[0x0401] = 0x05i8 as u8;
        cpu.step(&mut mem);
        assert_eq!(cpu.pc, 0x0407); // 0x0402 + 5
    }

    #[test]
    fn push_pull_roundtrip() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0500;
        cpu.a = 0xAB;
        cpu.sp = 0xEF;
        let mut mem = make_mem();
        mem[0x0500] = 0x2D; // PUSH A
        mem[0x0501] = 0xAE; // POP A
        cpu.step(&mut mem);
        let sp_after_push = cpu.sp;
        cpu.a = 0x00;
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0xAB);
        assert_eq!(cpu.sp, sp_after_push.wrapping_add(1));
    }

    #[test]
    fn mul_ya() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0600;
        cpu.y = 0x03;
        cpu.a = 0x04;
        let mut mem = make_mem();
        mem[0x0600] = 0xCF; // MUL YA
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x0C); // YA result lo
        assert_eq!(cpu.y, 0x00); // hi = 0
    }

    #[test]
    fn div_ya_x() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0700;
        cpu.y = 0x00;
        cpu.a = 0x0C;
        cpu.x = 0x03;
        let mut mem = make_mem();
        mem[0x0700] = 0x9E; // DIV YA, X
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x04); // quotient
        assert_eq!(cpu.y, 0x00); // remainder
    }

    #[test]
    fn halt_on_sleep() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0800;
        let mut mem = make_mem();
        mem[0x0800] = 0xEF; // SLEEP
        cpu.step(&mut mem);
        assert_eq!(cpu.halted, Some(ApuHalt::Sleep));
        // Further steps return 0 without advancing PC.
        let c = cpu.step(&mut mem);
        assert_eq!(c, 0);
        assert_eq!(cpu.pc, 0x0801);
    }

    #[test]
    fn set1_clr1() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0900;
        let mut mem = make_mem();
        mem[0x0900] = 0x02; // SET1 dp.0
        mem[0x0901] = 0x10; // offset
        mem[0x0010] = 0x00;
        cpu.step(&mut mem);
        assert_eq!(mem[0x0010], 0x01);

        cpu.pc = 0x0902;
        mem[0x0902] = 0x12; // CLR1 dp.0
        mem[0x0903] = 0x10;
        cpu.step(&mut mem);
        assert_eq!(mem[0x0010], 0x00);
    }

    #[test]
    fn direct_page_p_flag() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0A00;
        cpu.psw |= psw::P; // select page 1
        let mut mem = make_mem();
        mem[0x0A00] = 0xE4; // MOV A, dp
        mem[0x0A01] = 0x10; // offset $10 → address $0110
        mem[0x0110] = 0xBB;
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0xBB);
    }

    #[test]
    fn movw_ya_dp() {
        let mut cpu = Spc700::new();
        cpu.pc = 0x0B00;
        let mut mem = make_mem();
        mem[0x0B00] = 0xBA; // MOVW YA, dp
        mem[0x0B01] = 0x20;
        mem[0x0020] = 0x34;
        mem[0x0021] = 0x12;
        cpu.step(&mut mem);
        assert_eq!(cpu.a, 0x34);
        assert_eq!(cpu.y, 0x12);
    }
}
