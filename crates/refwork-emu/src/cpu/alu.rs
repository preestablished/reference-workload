//! Arithmetic/logic primitives: ADC and SBC (binary and BCD decimal in both
//! 8-bit and 16-bit widths), compares, and the simple bit logic.
//!
//! Decimal-mode flag semantics follow the documented 65C816 behavior:
//! - Carry (C) is the BCD carry out of the top nibble.
//! - Z and N are computed from the final BCD result.
//! - V (overflow) is computed binary-style (from the pre-decimal-adjust
//!   nibble sums, using the sign of the high nibble), matching hardware.

use super::{flags, Cpu};

impl Cpu {
    // ---- ADC ----

    /// Add `operand` to A with carry, width per the M flag. Updates N/V/Z/C
    /// and A.
    pub(crate) fn op_adc(&mut self, operand: u16) {
        if self.p & flags::D != 0 {
            if self.m8() {
                self.adc_bcd8(operand as u8);
            } else {
                self.adc_bcd16(operand);
            }
        } else if self.m8() {
            self.adc_bin8(operand as u8);
        } else {
            self.adc_bin16(operand);
        }
    }

    fn adc_bin8(&mut self, m: u8) {
        let a = self.a as u8;
        let c = (self.p & flags::C) as u16;
        let sum = a as u16 + m as u16 + c;
        let res = sum as u8;
        let overflow = (!(a ^ m) & (a ^ res) & 0x80) != 0;
        self.set_flag(flags::C, sum > 0xFF);
        self.set_flag(flags::V, overflow);
        self.set_nz8(res);
        self.a = (self.a & 0xFF00) | res as u16;
    }

    fn adc_bin16(&mut self, m: u16) {
        let a = self.a;
        let c = (self.p & flags::C) as u32;
        let sum = a as u32 + m as u32 + c;
        let res = sum as u16;
        let overflow = (!(a ^ m) & (a ^ res) & 0x8000) != 0;
        self.set_flag(flags::C, sum > 0xFFFF);
        self.set_flag(flags::V, overflow);
        self.set_nz16(res);
        self.a = res;
    }

    fn adc_bcd8(&mut self, m: u8) {
        let a = self.a as u8;
        let c = (self.p & flags::C) as u16;
        // Low nibble.
        let mut lo = (a & 0x0F) as u16 + (m & 0x0F) as u16 + c;
        // Binary-style overflow uses the pre-adjust high-nibble sum.
        let mut hi = (a >> 4) as u16 + (m >> 4) as u16;
        if lo > 9 {
            lo += 6;
        }
        if lo > 0x0F {
            hi += 1;
        }
        // Compute V from the binary sign of the (decimal-adjusted-low)
        // high-nibble sum before the high decimal adjust.
        let bin_hi = (hi << 4) as u8;
        let overflow = (!(a ^ m) & (a ^ bin_hi) & 0x80) != 0;
        if hi > 9 {
            hi += 6;
        }
        let carry = hi > 0x0F;
        let res = (((hi << 4) | (lo & 0x0F)) & 0xFF) as u8;
        self.set_flag(flags::C, carry);
        self.set_flag(flags::V, overflow);
        self.set_nz8(res);
        self.a = (self.a & 0xFF00) | res as u16;
    }

    fn adc_bcd16(&mut self, m: u16) {
        let a = self.a;
        let c = (self.p & flags::C) as u32;
        let mut d0 = (a & 0x000F) as u32 + (m & 0x000F) as u32 + c;
        let mut d1 = ((a >> 4) & 0x000F) as u32 + ((m >> 4) & 0x000F) as u32;
        let mut d2 = ((a >> 8) & 0x000F) as u32 + ((m >> 8) & 0x000F) as u32;
        let mut d3 = ((a >> 12) & 0x000F) as u32 + ((m >> 12) & 0x000F) as u32;
        if d0 > 9 {
            d0 += 6;
        }
        if d0 > 0x0F {
            d1 += 1;
        }
        if d1 > 9 {
            d1 += 6;
        }
        if d1 > 0x0F {
            d2 += 1;
        }
        if d2 > 9 {
            d2 += 6;
        }
        if d2 > 0x0F {
            d3 += 1;
        }
        let bin_hi = (d3 << 12) as u16;
        let overflow = (!(a ^ m) & (a ^ bin_hi) & 0x8000) != 0;
        if d3 > 9 {
            d3 += 6;
        }
        let carry = d3 > 0x0F;
        let res =
            (((d3 & 0x0F) << 12) | ((d2 & 0x0F) << 8) | ((d1 & 0x0F) << 4) | (d0 & 0x0F)) as u16;
        self.set_flag(flags::C, carry);
        self.set_flag(flags::V, overflow);
        self.set_nz16(res);
        self.a = res;
    }

    // ---- SBC ----

    /// Subtract `operand` from A with borrow (C clear = borrow), width per M.
    pub(crate) fn op_sbc(&mut self, operand: u16) {
        if self.p & flags::D != 0 {
            if self.m8() {
                self.sbc_bcd8(operand as u8);
            } else {
                self.sbc_bcd16(operand);
            }
        } else if self.m8() {
            self.sbc_bin8(operand as u8);
        } else {
            self.sbc_bin16(operand);
        }
    }

    fn sbc_bin8(&mut self, m: u8) {
        let a = self.a as u8;
        let c = (self.p & flags::C) as i32;
        let diff = a as i32 - m as i32 - (1 - c);
        let res = diff as u8;
        // Overflow: signs of A and ~M agree, but result sign differs.
        let overflow = ((a ^ m) & (a ^ res) & 0x80) != 0;
        self.set_flag(flags::C, diff >= 0);
        self.set_flag(flags::V, overflow);
        self.set_nz8(res);
        self.a = (self.a & 0xFF00) | res as u16;
    }

    fn sbc_bin16(&mut self, m: u16) {
        let a = self.a;
        let c = (self.p & flags::C) as i32;
        let diff = a as i32 - m as i32 - (1 - c);
        let res = diff as u16;
        let overflow = ((a ^ m) & (a ^ res) & 0x8000) != 0;
        self.set_flag(flags::C, diff >= 0);
        self.set_flag(flags::V, overflow);
        self.set_nz16(res);
        self.a = res;
    }

    fn sbc_bcd8(&mut self, m: u8) {
        let a = self.a as u8;
        let c = (self.p & flags::C) as i32;
        // Binary diff first (drives C and V like binary subtraction).
        let bin = a as i32 - m as i32 - (1 - c);
        let res_bin = bin as u8;
        let overflow = ((a ^ m) & (a ^ res_bin) & 0x80) != 0;
        // Decimal adjust per nibble.
        let mut lo = (a as i32 & 0x0F) - (m as i32 & 0x0F) - (1 - c);
        let mut hi = (a as i32 >> 4) - (m as i32 >> 4);
        if lo < 0 {
            lo -= 6;
            hi -= 1;
        }
        if hi < 0 {
            hi -= 6;
        }
        let res = (((hi << 4) | (lo & 0x0F)) & 0xFF) as u8;
        self.set_flag(flags::C, bin >= 0);
        self.set_flag(flags::V, overflow);
        self.set_nz8(res);
        self.a = (self.a & 0xFF00) | res as u16;
    }

    fn sbc_bcd16(&mut self, m: u16) {
        let a = self.a;
        let c = (self.p & flags::C) as i32;
        let bin = a as i32 - m as i32 - (1 - c);
        let res_bin = bin as u16;
        let overflow = ((a ^ m) & (a ^ res_bin) & 0x8000) != 0;
        let mut d0 = (a as i32 & 0x0F) - (m as i32 & 0x0F) - (1 - c);
        let mut d1 = ((a as i32 >> 4) & 0x0F) - ((m as i32 >> 4) & 0x0F);
        let mut d2 = ((a as i32 >> 8) & 0x0F) - ((m as i32 >> 8) & 0x0F);
        let mut d3 = ((a as i32 >> 12) & 0x0F) - ((m as i32 >> 12) & 0x0F);
        if d0 < 0 {
            d0 -= 6;
            d1 -= 1;
        }
        if d1 < 0 {
            d1 -= 6;
            d2 -= 1;
        }
        if d2 < 0 {
            d2 -= 6;
            d3 -= 1;
        }
        if d3 < 0 {
            d3 -= 6;
        }
        let res =
            (((d3 & 0x0F) << 12) | ((d2 & 0x0F) << 8) | ((d1 & 0x0F) << 4) | (d0 & 0x0F)) as u16;
        self.set_flag(flags::C, bin >= 0);
        self.set_flag(flags::V, overflow);
        self.set_nz16(res);
        self.a = res;
    }

    // ---- compares ----

    /// Compare `reg` against `m` at the given width, setting N/Z/C.
    pub(crate) fn op_compare(&mut self, reg: u16, m: u16, eight: bool) {
        if eight {
            let r = reg as u8;
            let v = m as u8;
            let diff = r.wrapping_sub(v);
            self.set_flag(flags::C, r >= v);
            self.set_nz8(diff);
        } else {
            let diff = reg.wrapping_sub(m);
            self.set_flag(flags::C, reg >= m);
            self.set_nz16(diff);
        }
    }
}
