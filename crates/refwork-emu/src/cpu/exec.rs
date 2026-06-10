//! Opcode decode and execution for the full 65C816-class instruction set.
//!
//! The dispatch is a flat `match` over the 256 opcodes. Addressing-mode
//! resolution lives in [`super::addressing`]; arithmetic in [`super::alu`].
//! Each handler performs exactly the documented bus accesses (one
//! `read`/`write` per byte, one `idle` per internal cycle).

use super::addressing::Ea;
use super::{flags, Cpu, IntKind};
use crate::bus::Bus;

impl Cpu {
    /// Execute a single already-fetched opcode.
    pub(crate) fn execute<B: Bus>(&mut self, bus: &mut B, opcode: u8) {
        match opcode {
            // ---- ADC ----
            0x69 => {
                let v = self.imm_m(bus);
                self.op_adc(v);
            }
            0x6D => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x6F => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x65 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x72 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x67 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x7D => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x7F => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x79 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x75 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x61 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x71 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x77 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x63 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }
            0x73 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.op_adc(v);
            }

            // ---- SBC ----
            0xE9 => {
                let v = self.imm_m(bus);
                self.op_sbc(v);
            }
            0xED => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xEF => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xE5 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF2 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xE7 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xFD => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xFF => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF9 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF5 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xE1 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF1 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF7 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xE3 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }
            0xF3 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.op_sbc(v);
            }

            // ---- AND ----
            0x29 => {
                let v = self.imm_m(bus);
                self.op_and(v);
            }
            0x2D => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x2F => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x25 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x32 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x27 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x3D => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x3F => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x39 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x35 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x21 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x31 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x37 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x23 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }
            0x33 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.op_and(v);
            }

            // ---- ORA ----
            0x09 => {
                let v = self.imm_m(bus);
                self.op_ora(v);
            }
            0x0D => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x0F => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x05 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x12 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x07 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x1D => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x1F => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x19 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x15 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x01 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x11 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x17 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x03 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }
            0x13 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.op_ora(v);
            }

            // ---- EOR ----
            0x49 => {
                let v = self.imm_m(bus);
                self.op_eor(v);
            }
            0x4D => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x4F => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x45 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x52 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x47 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x5D => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x5F => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x59 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x55 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x41 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x51 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x57 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x43 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }
            0x53 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.op_eor(v);
            }

            // ---- CMP ----
            0xC9 => {
                let v = self.imm_m(bus);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xCD => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xCF => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xC5 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD2 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xC7 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xDD => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xDF => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD9 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD5 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xC1 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD1 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD7 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xC3 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }
            0xD3 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                let a = self.a;
                self.op_compare(a, v, self.m8());
            }

            // ---- CPX ----
            0xE0 => {
                let v = self.imm_x(bus);
                let x = self.x;
                self.op_compare(x, v, self.x8());
            }
            0xEC => {
                let ea = self.ea_absolute(bus);
                let v = self.read_x(bus, ea);
                let x = self.x;
                self.op_compare(x, v, self.x8());
            }
            0xE4 => {
                let ea = self.ea_direct(bus);
                let v = self.read_x(bus, ea);
                let x = self.x;
                self.op_compare(x, v, self.x8());
            }

            // ---- CPY ----
            0xC0 => {
                let v = self.imm_x(bus);
                let y = self.y;
                self.op_compare(y, v, self.x8());
            }
            0xCC => {
                let ea = self.ea_absolute(bus);
                let v = self.read_x(bus, ea);
                let y = self.y;
                self.op_compare(y, v, self.x8());
            }
            0xC4 => {
                let ea = self.ea_direct(bus);
                let v = self.read_x(bus, ea);
                let y = self.y;
                self.op_compare(y, v, self.x8());
            }

            // ---- LDA ----
            0xA9 => {
                let v = self.imm_m(bus);
                self.set_a_m(v);
            }
            0xAD => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xAF => {
                let ea = self.ea_long(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xA5 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB2 => {
                let ea = self.ea_indirect(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xA7 => {
                let ea = self.ea_indirect_long(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xBD => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xBF => {
                let ea = self.ea_long_x(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB9 => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB5 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xA1 => {
                let ea = self.ea_indirect_x(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB1 => {
                let ea = self.ea_indirect_y(bus, false);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB7 => {
                let ea = self.ea_indirect_long_y(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xA3 => {
                let ea = self.ea_stack_rel(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }
            0xB3 => {
                let ea = self.ea_stack_rel_y(bus);
                let v = self.read_m(bus, ea);
                self.set_a_m(v);
            }

            // ---- LDX ----
            0xA2 => {
                let v = self.imm_x(bus);
                self.set_x_reg(v);
            }
            0xAE => {
                let ea = self.ea_absolute(bus);
                let v = self.read_x(bus, ea);
                self.set_x_reg(v);
            }
            0xA6 => {
                let ea = self.ea_direct(bus);
                let v = self.read_x(bus, ea);
                self.set_x_reg(v);
            }
            0xBE => {
                let ea = self.ea_absolute_y(bus, false);
                let v = self.read_x(bus, ea);
                self.set_x_reg(v);
            }
            0xB6 => {
                let ea = self.ea_direct_y(bus);
                let v = self.read_x(bus, ea);
                self.set_x_reg(v);
            }

            // ---- LDY ----
            0xA0 => {
                let v = self.imm_x(bus);
                self.set_y_reg(v);
            }
            0xAC => {
                let ea = self.ea_absolute(bus);
                let v = self.read_x(bus, ea);
                self.set_y_reg(v);
            }
            0xA4 => {
                let ea = self.ea_direct(bus);
                let v = self.read_x(bus, ea);
                self.set_y_reg(v);
            }
            0xBC => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_x(bus, ea);
                self.set_y_reg(v);
            }
            0xB4 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_x(bus, ea);
                self.set_y_reg(v);
            }

            // ---- STA ----
            0x8D => {
                let ea = self.ea_absolute(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x8F => {
                let ea = self.ea_long(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x85 => {
                let ea = self.ea_direct(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x92 => {
                let ea = self.ea_indirect(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x87 => {
                let ea = self.ea_indirect_long(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x9D => {
                let ea = self.ea_absolute_x(bus, true);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x9F => {
                let ea = self.ea_long_x(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x99 => {
                let ea = self.ea_absolute_y(bus, true);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x95 => {
                let ea = self.ea_direct_x(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x81 => {
                let ea = self.ea_indirect_x(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x91 => {
                let ea = self.ea_indirect_y(bus, true);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x97 => {
                let ea = self.ea_indirect_long_y(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x83 => {
                let ea = self.ea_stack_rel(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }
            0x93 => {
                let ea = self.ea_stack_rel_y(bus);
                let a = self.a;
                self.write_m(bus, ea, a);
            }

            // ---- STX ----
            0x8E => {
                let ea = self.ea_absolute(bus);
                let x = self.x;
                self.write_x(bus, ea, x);
            }
            0x86 => {
                let ea = self.ea_direct(bus);
                let x = self.x;
                self.write_x(bus, ea, x);
            }
            0x96 => {
                let ea = self.ea_direct_y(bus);
                let x = self.x;
                self.write_x(bus, ea, x);
            }

            // ---- STY ----
            0x8C => {
                let ea = self.ea_absolute(bus);
                let y = self.y;
                self.write_x(bus, ea, y);
            }
            0x84 => {
                let ea = self.ea_direct(bus);
                let y = self.y;
                self.write_x(bus, ea, y);
            }
            0x94 => {
                let ea = self.ea_direct_x(bus);
                let y = self.y;
                self.write_x(bus, ea, y);
            }

            // ---- STZ ----
            0x9C => {
                let ea = self.ea_absolute(bus);
                self.write_m(bus, ea, 0);
            }
            0x64 => {
                let ea = self.ea_direct(bus);
                self.write_m(bus, ea, 0);
            }
            0x9E => {
                let ea = self.ea_absolute_x(bus, true);
                self.write_m(bus, ea, 0);
            }
            0x74 => {
                let ea = self.ea_direct_x(bus);
                self.write_m(bus, ea, 0);
            }

            // ---- BIT ----
            0x89 => {
                // Immediate BIT only affects Z (no N/V).
                let v = self.imm_m(bus);
                let a = self.a;
                let r = if self.m8() {
                    (a as u8 & v as u8) as u16
                } else {
                    a & v
                };
                self.set_flag(flags::Z, if self.m8() { r as u8 == 0 } else { r == 0 });
            }
            0x2C => {
                let ea = self.ea_absolute(bus);
                let v = self.read_m(bus, ea);
                self.op_bit(v);
            }
            0x24 => {
                let ea = self.ea_direct(bus);
                let v = self.read_m(bus, ea);
                self.op_bit(v);
            }
            0x3C => {
                let ea = self.ea_absolute_x(bus, false);
                let v = self.read_m(bus, ea);
                self.op_bit(v);
            }
            0x34 => {
                let ea = self.ea_direct_x(bus);
                let v = self.read_m(bus, ea);
                self.op_bit(v);
            }

            // ---- TRB / TSB ----
            0x1C => {
                let ea = self.ea_absolute(bus);
                self.op_trb(bus, ea);
            }
            0x14 => {
                let ea = self.ea_direct(bus);
                self.op_trb(bus, ea);
            }
            0x0C => {
                let ea = self.ea_absolute(bus);
                self.op_tsb(bus, ea);
            }
            0x04 => {
                let ea = self.ea_direct(bus);
                self.op_tsb(bus, ea);
            }

            // ---- INC / DEC (accumulator) ----
            0x1A => self.inc_a(),
            0x3A => self.dec_a(),

            // ---- INC / DEC (memory) ----
            0xEE => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_inc(v));
            }
            0xE6 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_inc(v));
            }
            0xFE => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_inc(v));
            }
            0xF6 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_inc(v));
            }
            0xCE => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_dec(v));
            }
            0xC6 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_dec(v));
            }
            0xDE => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_dec(v));
            }
            0xD6 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_dec(v));
            }

            // ---- INX / DEX / INY / DEY ----
            0xE8 => self.inx(),
            0xCA => self.dex(),
            0xC8 => self.iny(),
            0x88 => self.dey(),

            // ---- ASL ----
            0x0A => self.asl_a(),
            0x0E => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_asl(v));
            }
            0x06 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_asl(v));
            }
            0x1E => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_asl(v));
            }
            0x16 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_asl(v));
            }

            // ---- LSR ----
            0x4A => self.lsr_a(),
            0x4E => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_lsr(v));
            }
            0x46 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_lsr(v));
            }
            0x5E => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_lsr(v));
            }
            0x56 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_lsr(v));
            }

            // ---- ROL ----
            0x2A => self.rol_a(),
            0x2E => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_rol(v));
            }
            0x26 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_rol(v));
            }
            0x3E => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_rol(v));
            }
            0x36 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_rol(v));
            }

            // ---- ROR ----
            0x6A => self.ror_a(),
            0x6E => {
                let ea = self.ea_absolute(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_ror(v));
            }
            0x66 => {
                let ea = self.ea_direct(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_ror(v));
            }
            0x7E => {
                let ea = self.ea_absolute_x(bus, true);
                self.rmw_m(bus, ea, |c, v| c.alu_ror(v));
            }
            0x76 => {
                let ea = self.ea_direct_x(bus);
                self.rmw_m(bus, ea, |c, v| c.alu_ror(v));
            }

            // ---- branches ----
            0x90 => self.branch(bus, self.p & flags::C == 0), // BCC
            0xB0 => self.branch(bus, self.p & flags::C != 0), // BCS
            0xF0 => self.branch(bus, self.p & flags::Z != 0), // BEQ
            0xD0 => self.branch(bus, self.p & flags::Z == 0), // BNE
            0x30 => self.branch(bus, self.p & flags::N != 0), // BMI
            0x10 => self.branch(bus, self.p & flags::N == 0), // BPL
            0x50 => self.branch(bus, self.p & flags::V == 0), // BVC
            0x70 => self.branch(bus, self.p & flags::V != 0), // BVS
            0x80 => self.branch(bus, true),                   // BRA
            0x82 => self.branch_long(bus),                    // BRL

            // ---- jumps / calls ----
            0x4C => {
                // JMP absolute
                let addr = self.fetch16(bus);
                self.pc = addr;
            }
            0x5C => {
                // JMP absolute long (JML)
                let lo = self.fetch16(bus);
                let bank = self.fetch8(bus);
                self.pc = lo;
                self.pbr = bank;
            }
            0x6C => {
                // JMP (absolute) — pointer in bank $00.
                let ptr = self.fetch16(bus);
                let lo = bus.read(ptr as u32) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u32) as u16;
                self.pc = lo | (hi << 8);
            }
            0x7C => {
                // JMP (absolute,X) — pointer in the program bank.
                let base = self.fetch16(bus);
                bus.idle();
                let ptr = base.wrapping_add(self.x);
                let pa = ((self.pbr as u32) << 16) | ptr as u32;
                let pa1 = ((self.pbr as u32) << 16) | ptr.wrapping_add(1) as u32;
                let lo = bus.read(pa) as u16;
                let hi = bus.read(pa1) as u16;
                self.pc = lo | (hi << 8);
            }
            0xDC => {
                // JMP [absolute] — 24-bit pointer in bank $00 (JML).
                let ptr = self.fetch16(bus);
                let lo = bus.read(ptr as u32) as u16;
                let hi = bus.read(ptr.wrapping_add(1) as u32) as u16;
                let bank = bus.read(ptr.wrapping_add(2) as u32);
                self.pc = lo | (hi << 8);
                self.pbr = bank;
            }
            0x20 => {
                // JSR absolute
                let addr = self.fetch16(bus);
                bus.idle();
                let ret = self.pc.wrapping_sub(1);
                self.push16(bus, ret);
                self.pc = addr;
            }
            0x22 => {
                // JSL absolute long ("new" instruction: unforced stack)
                let lo = self.fetch16(bus);
                self.push8_n(bus, self.pbr);
                bus.idle();
                let bank = self.fetch8(bus);
                let ret = self.pc.wrapping_sub(1);
                self.push16_n(bus, ret);
                self.pbr = bank;
                self.pc = lo;
            }
            0xFC => {
                // JSR (absolute,X) — push return, pointer in program bank.
                let lo = self.fetch8(bus) as u16;
                let ret = self.pc; // points at the high operand byte
                self.push16(bus, ret);
                let hi = self.fetch8(bus) as u16;
                let base = lo | (hi << 8);
                bus.idle();
                let ptr = base.wrapping_add(self.x);
                let pa = ((self.pbr as u32) << 16) | ptr as u32;
                let pa1 = ((self.pbr as u32) << 16) | ptr.wrapping_add(1) as u32;
                let plo = bus.read(pa) as u16;
                let phi = bus.read(pa1) as u16;
                self.pc = plo | (phi << 8);
            }
            0x60 => {
                // RTS
                bus.idle();
                bus.idle();
                let ret = self.pull16(bus);
                bus.idle();
                self.pc = ret.wrapping_add(1);
            }
            0x6B => {
                // RTL ("new" instruction: unforced stack)
                bus.idle();
                bus.idle();
                let ret = self.pull16_n(bus);
                let bank = self.pull8_n(bus);
                self.pc = ret.wrapping_add(1);
                self.pbr = bank;
            }
            0x40 => self.op_rti(bus),

            // ---- BRK / COP ----
            0x00 => {
                // BRK: signature byte consumed, then vector through BRK.
                let _sig = self.fetch8(bus);
                self.service_interrupt(bus, IntKind::Brk);
            }
            0x02 => {
                let _sig = self.fetch8(bus);
                self.service_interrupt(bus, IntKind::Cop);
            }

            // ---- transfers ----
            0xAA => {
                // TAX
                let v = self.a;
                self.set_x_reg_keepwidth(v);
            }
            0xA8 => {
                // TAY
                let v = self.a;
                self.set_y_reg_keepwidth(v);
            }
            0x8A => {
                // TXA
                if self.m8() {
                    self.a = (self.a & 0xFF00) | (self.x & 0x00FF);
                } else {
                    self.a = self.x;
                }
                self.set_nz_m(self.a);
            }
            0x98 => {
                // TYA
                if self.m8() {
                    self.a = (self.a & 0xFF00) | (self.y & 0x00FF);
                } else {
                    self.a = self.y;
                }
                self.set_nz_m(self.a);
            }
            0x9B => {
                // TXY
                let v = self.x;
                self.set_y_reg_keepwidth(v);
            }
            0xBB => {
                // TYX
                let v = self.y;
                self.set_x_reg_keepwidth(v);
            }
            0xBA => {
                // TSX
                if self.x8() {
                    self.x = self.s & 0x00FF;
                    self.set_nz8(self.x as u8);
                } else {
                    self.x = self.s;
                    self.set_nz16(self.x);
                }
            }
            0x9A => {
                // TXS — no flags. Emulation forces high byte $01.
                if self.e {
                    self.s = 0x0100 | (self.x & 0x00FF);
                } else {
                    self.s = self.x;
                }
            }
            0x5B => {
                // TCD — full 16-bit, sets N/Z.
                self.d = self.a;
                self.set_nz16(self.d);
            }
            0x7B => {
                // TDC
                self.a = self.d;
                self.set_nz16(self.a);
            }
            0x1B => {
                // TCS — no flags. Emulation forces high byte $01.
                if self.e {
                    self.s = 0x0100 | (self.a & 0x00FF);
                } else {
                    self.s = self.a;
                }
            }
            0x3B => {
                // TSC
                self.a = self.s;
                self.set_nz16(self.a);
            }

            // ---- stack push/pull ----
            0x48 => {
                // PHA
                if self.m8() {
                    let v = self.a as u8;
                    self.push8(bus, v);
                } else {
                    let v = self.a;
                    self.push16(bus, v);
                }
            }
            0x68 => {
                // PLA
                let v = if self.m8() {
                    self.pull8(bus) as u16
                } else {
                    self.pull16(bus)
                };
                self.set_a_m(v);
            }
            0xDA => {
                // PHX
                if self.x8() {
                    let v = self.x as u8;
                    self.push8(bus, v);
                } else {
                    let v = self.x;
                    self.push16(bus, v);
                }
            }
            0xFA => {
                // PLX
                let v = if self.x8() {
                    self.pull8(bus) as u16
                } else {
                    self.pull16(bus)
                };
                self.set_x_reg(v);
            }
            0x5A => {
                // PHY
                if self.x8() {
                    let v = self.y as u8;
                    self.push8(bus, v);
                } else {
                    let v = self.y;
                    self.push16(bus, v);
                }
            }
            0x7A => {
                // PLY
                let v = if self.x8() {
                    self.pull8(bus) as u16
                } else {
                    self.pull16(bus)
                };
                self.set_y_reg(v);
            }
            0x08 => {
                // PHP
                let p = self.p;
                self.push8(bus, p);
            }
            0x28 => {
                // PLP
                let p = self.pull8(bus);
                self.p = p;
                if self.e {
                    self.p |= flags::M | flags::X;
                }
                self.normalize_widths();
            }
            0x8B => {
                // PHB ("new" instruction: unforced stack)
                let v = self.dbr;
                self.push8_n(bus, v);
            }
            0xAB => {
                // PLB ("new" instruction: unforced stack)
                let v = self.pull8_n(bus);
                self.dbr = v;
                self.set_nz8(v);
            }
            0x0B => {
                // PHD — full 16-bit ("new" instruction: unforced stack).
                let v = self.d;
                self.push16_n(bus, v);
            }
            0x2B => {
                // PLD ("new" instruction: unforced stack)
                let v = self.pull16_n(bus);
                self.d = v;
                self.set_nz16(v);
            }
            0x4B => {
                // PHK
                let v = self.pbr;
                self.push8(bus, v);
            }
            0xF4 => {
                // PEA — push a 16-bit immediate ("new": unforced stack).
                let v = self.fetch16(bus);
                self.push16_n(bus, v);
            }
            0xD4 => {
                // PEI — push the 16-bit pointer word read from the direct
                // page (not a dereference of it).
                let v = self.read_dp_word(bus);
                self.push16_n(bus, v); // PEI is a "new" instruction
            }
            0x62 => {
                // PER — push PC + signed 16-bit displacement.
                let disp = self.fetch16(bus);
                bus.idle();
                let target = self.pc.wrapping_add(disp);
                self.push16_n(bus, target); // PER is a "new" instruction
            }

            // ---- flag ops ----
            0x18 => self.set_flag(flags::C, false), // CLC
            0x38 => self.set_flag(flags::C, true),  // SEC
            0x58 => self.set_flag(flags::I, false), // CLI
            0x78 => self.set_flag(flags::I, true),  // SEI
            0xB8 => self.set_flag(flags::V, false), // CLV
            0xD8 => self.set_flag(flags::D, false), // CLD
            0xF8 => self.set_flag(flags::D, true),  // SED

            0xC2 => {
                // REP — clear P bits.
                let mask = self.fetch8(bus);
                bus.idle();
                self.p &= !mask;
                if self.e {
                    self.p |= flags::M | flags::X;
                }
                self.normalize_widths();
            }
            0xE2 => {
                // SEP — set P bits.
                let mask = self.fetch8(bus);
                bus.idle();
                self.p |= mask;
                self.normalize_widths();
            }
            0xFB => {
                // XCE — exchange C and E.
                let old_c = self.p & flags::C != 0;
                let old_e = self.e;
                self.set_flag(flags::C, old_e);
                self.e = old_c;
                if self.e {
                    self.p |= flags::M | flags::X;
                }
                self.normalize_widths();
            }
            0xEB => {
                // XBA — swap the two bytes of A; flags on the new low byte.
                let lo = self.a & 0x00FF;
                let hi = (self.a >> 8) & 0x00FF;
                self.a = (lo << 8) | hi;
                bus.idle();
                self.set_nz8(self.a as u8);
            }

            // ---- block moves ----
            0x54 => self.op_mvn(bus),
            0x44 => self.op_mvp(bus),

            // ---- WAI / STP ----
            0xCB => {
                self.waiting = true;
                bus.idle();
            }
            0xDB => {
                self.stopped = true;
                bus.idle();
            }

            // ---- NOP / WDM / reserved ----
            0xEA => {
                // NOP
            }
            0x42 => {
                // WDM — 2-byte NOP (consumes the signature byte).
                let _ = self.fetch8(bus);
            }
        }
    }

    // ---- immediate operand width helpers ----

    fn imm_m<B: Bus>(&mut self, bus: &mut B) -> u16 {
        if self.m8() {
            self.fetch8(bus) as u16
        } else {
            self.fetch16(bus)
        }
    }

    fn imm_x<B: Bus>(&mut self, bus: &mut B) -> u16 {
        if self.x8() {
            self.fetch8(bus) as u16
        } else {
            self.fetch16(bus)
        }
    }

    fn read_x<B: Bus>(&mut self, bus: &mut B, ea: Ea) -> u16 {
        if self.x8() {
            self.read8(bus, ea) as u16
        } else {
            self.read16(bus, ea)
        }
    }

    fn write_x<B: Bus>(&mut self, bus: &mut B, ea: Ea, v: u16) {
        if self.x8() {
            self.write8(bus, ea, v as u8);
        } else {
            self.write16(bus, ea, v);
        }
    }

    // ---- register width-aware setters ----

    fn set_a_m(&mut self, v: u16) {
        if self.m8() {
            self.a = (self.a & 0xFF00) | (v & 0x00FF);
            self.set_nz8(v as u8);
        } else {
            self.a = v;
            self.set_nz16(v);
        }
    }

    fn set_x_reg(&mut self, v: u16) {
        if self.x8() {
            self.x = v & 0x00FF;
            self.set_nz8(v as u8);
        } else {
            self.x = v;
            self.set_nz16(v);
        }
    }

    fn set_y_reg(&mut self, v: u16) {
        if self.x8() {
            self.y = v & 0x00FF;
            self.set_nz8(v as u8);
        } else {
            self.y = v;
            self.set_nz16(v);
        }
    }

    /// Transfer into X taking the source width from the X flag (transfers
    /// truncate the source to the destination width).
    fn set_x_reg_keepwidth(&mut self, v: u16) {
        if self.x8() {
            self.x = v & 0x00FF;
            self.set_nz8(self.x as u8);
        } else {
            self.x = v;
            self.set_nz16(self.x);
        }
    }

    fn set_y_reg_keepwidth(&mut self, v: u16) {
        if self.x8() {
            self.y = v & 0x00FF;
            self.set_nz8(self.y as u8);
        } else {
            self.y = v;
            self.set_nz16(self.y);
        }
    }

    // ---- simple logic ops ----

    pub(crate) fn op_and(&mut self, m: u16) {
        if self.m8() {
            let r = (self.a as u8) & (m as u8);
            self.a = (self.a & 0xFF00) | r as u16;
            self.set_nz8(r);
        } else {
            self.a &= m;
            self.set_nz16(self.a);
        }
    }

    pub(crate) fn op_ora(&mut self, m: u16) {
        if self.m8() {
            let r = (self.a as u8) | (m as u8);
            self.a = (self.a & 0xFF00) | r as u16;
            self.set_nz8(r);
        } else {
            self.a |= m;
            self.set_nz16(self.a);
        }
    }

    pub(crate) fn op_eor(&mut self, m: u16) {
        if self.m8() {
            let r = (self.a as u8) ^ (m as u8);
            self.a = (self.a & 0xFF00) | r as u16;
            self.set_nz8(r);
        } else {
            self.a ^= m;
            self.set_nz16(self.a);
        }
    }

    fn op_bit(&mut self, m: u16) {
        if self.m8() {
            let r = (self.a as u8) & (m as u8);
            self.set_flag(flags::Z, r == 0);
            self.set_flag(flags::N, m & 0x80 != 0);
            self.set_flag(flags::V, m & 0x40 != 0);
        } else {
            let r = self.a & m;
            self.set_flag(flags::Z, r == 0);
            self.set_flag(flags::N, m & 0x8000 != 0);
            self.set_flag(flags::V, m & 0x4000 != 0);
        }
    }

    fn op_trb<B: Bus>(&mut self, bus: &mut B, ea: Ea) {
        let m = self.read_m(bus, ea);
        bus.idle();
        if self.m8() {
            let a = self.a as u8;
            self.set_flag(flags::Z, (a & m as u8) == 0);
            let r = m as u8 & !a;
            self.write8(bus, ea, r);
        } else {
            self.set_flag(flags::Z, (self.a & m) == 0);
            let r = m & !self.a;
            self.write16(bus, ea, r);
        }
    }

    fn op_tsb<B: Bus>(&mut self, bus: &mut B, ea: Ea) {
        let m = self.read_m(bus, ea);
        bus.idle();
        if self.m8() {
            let a = self.a as u8;
            self.set_flag(flags::Z, (a & m as u8) == 0);
            let r = m as u8 | a;
            self.write8(bus, ea, r);
        } else {
            self.set_flag(flags::Z, (self.a & m) == 0);
            let r = m | self.a;
            self.write16(bus, ea, r);
        }
    }

    // ---- accumulator inc/dec ----

    fn inc_a(&mut self) {
        if self.m8() {
            let r = (self.a as u8).wrapping_add(1);
            self.a = (self.a & 0xFF00) | r as u16;
            self.set_nz8(r);
        } else {
            self.a = self.a.wrapping_add(1);
            self.set_nz16(self.a);
        }
    }

    fn dec_a(&mut self) {
        if self.m8() {
            let r = (self.a as u8).wrapping_sub(1);
            self.a = (self.a & 0xFF00) | r as u16;
            self.set_nz8(r);
        } else {
            self.a = self.a.wrapping_sub(1);
            self.set_nz16(self.a);
        }
    }

    fn inx(&mut self) {
        if self.x8() {
            self.x = (self.x as u8).wrapping_add(1) as u16;
            self.set_nz8(self.x as u8);
        } else {
            self.x = self.x.wrapping_add(1);
            self.set_nz16(self.x);
        }
    }

    fn dex(&mut self) {
        if self.x8() {
            self.x = (self.x as u8).wrapping_sub(1) as u16;
            self.set_nz8(self.x as u8);
        } else {
            self.x = self.x.wrapping_sub(1);
            self.set_nz16(self.x);
        }
    }

    fn iny(&mut self) {
        if self.x8() {
            self.y = (self.y as u8).wrapping_add(1) as u16;
            self.set_nz8(self.y as u8);
        } else {
            self.y = self.y.wrapping_add(1);
            self.set_nz16(self.y);
        }
    }

    fn dey(&mut self) {
        if self.x8() {
            self.y = (self.y as u8).wrapping_sub(1) as u16;
            self.set_nz8(self.y as u8);
        } else {
            self.y = self.y.wrapping_sub(1);
            self.set_nz16(self.y);
        }
    }

    // ---- shift/rotate primitives (width per M), returning the result ----

    pub(crate) fn alu_inc(&mut self, v: u16) -> u16 {
        if self.m8() {
            let r = (v as u8).wrapping_add(1);
            self.set_nz8(r);
            r as u16
        } else {
            let r = v.wrapping_add(1);
            self.set_nz16(r);
            r
        }
    }

    pub(crate) fn alu_dec(&mut self, v: u16) -> u16 {
        if self.m8() {
            let r = (v as u8).wrapping_sub(1);
            self.set_nz8(r);
            r as u16
        } else {
            let r = v.wrapping_sub(1);
            self.set_nz16(r);
            r
        }
    }

    pub(crate) fn alu_asl(&mut self, v: u16) -> u16 {
        if self.m8() {
            let r = (v as u8) << 1;
            self.set_flag(flags::C, v & 0x80 != 0);
            self.set_nz8(r);
            r as u16
        } else {
            let r = v << 1;
            self.set_flag(flags::C, v & 0x8000 != 0);
            self.set_nz16(r);
            r
        }
    }

    pub(crate) fn alu_lsr(&mut self, v: u16) -> u16 {
        if self.m8() {
            let r = (v as u8) >> 1;
            self.set_flag(flags::C, v & 0x01 != 0);
            self.set_nz8(r);
            r as u16
        } else {
            let r = v >> 1;
            self.set_flag(flags::C, v & 0x01 != 0);
            self.set_nz16(r);
            r
        }
    }

    pub(crate) fn alu_rol(&mut self, v: u16) -> u16 {
        let cin = (self.p & flags::C) as u16;
        if self.m8() {
            let r = ((v as u8) << 1) | cin as u8;
            self.set_flag(flags::C, v & 0x80 != 0);
            self.set_nz8(r);
            r as u16
        } else {
            let r = (v << 1) | cin;
            self.set_flag(flags::C, v & 0x8000 != 0);
            self.set_nz16(r);
            r
        }
    }

    pub(crate) fn alu_ror(&mut self, v: u16) -> u16 {
        let cin = (self.p & flags::C) as u16;
        if self.m8() {
            let r = ((v as u8) >> 1) | ((cin as u8) << 7);
            self.set_flag(flags::C, v & 0x01 != 0);
            self.set_nz8(r);
            r as u16
        } else {
            let r = (v >> 1) | (cin << 15);
            self.set_flag(flags::C, v & 0x01 != 0);
            self.set_nz16(r);
            r
        }
    }

    fn asl_a(&mut self) {
        let r = self.alu_asl(self.a);
        self.store_a_m(r);
    }
    fn lsr_a(&mut self) {
        let r = self.alu_lsr(self.a);
        self.store_a_m(r);
    }
    fn rol_a(&mut self) {
        let r = self.alu_rol(self.a);
        self.store_a_m(r);
    }
    fn ror_a(&mut self) {
        let r = self.alu_ror(self.a);
        self.store_a_m(r);
    }

    /// Store an M-width result into A without touching flags (flags already
    /// set by the alu_* primitive).
    fn store_a_m(&mut self, r: u16) {
        if self.m8() {
            self.a = (self.a & 0xFF00) | (r & 0x00FF);
        } else {
            self.a = r;
        }
    }

    /// Read-modify-write at an effective address, applying `f`. Cycle order
    /// follows the 65C816: read all operand bytes, one internal cycle, then
    /// write back.
    fn rmw_m<B: Bus, F>(&mut self, bus: &mut B, ea: Ea, f: F)
    where
        F: FnOnce(&mut Cpu, u16) -> u16,
    {
        let v = self.read_m(bus, ea);
        bus.idle();
        let r = f(self, v);
        self.write_m(bus, ea, r);
    }

    // ---- branches ----

    fn branch<B: Bus>(&mut self, bus: &mut B, take: bool) {
        let off = self.fetch8(bus) as i8 as i16;
        if take {
            bus.idle();
            let old = self.pc;
            let new = (self.pc as i16).wrapping_add(off) as u16;
            // Emulation-mode page-cross adds one more internal cycle.
            if self.e && (old & 0xFF00) != (new & 0xFF00) {
                bus.idle();
            }
            self.pc = new;
        }
    }

    fn branch_long<B: Bus>(&mut self, bus: &mut B) {
        let off = self.fetch16(bus) as i16;
        bus.idle();
        self.pc = (self.pc as i16).wrapping_add(off) as u16;
    }

    // ---- RTI ----

    fn op_rti<B: Bus>(&mut self, bus: &mut B) {
        bus.idle();
        bus.idle();
        if self.e {
            let p = self.pull8(bus);
            self.p = p | flags::M | flags::X;
            let lo = self.pull8(bus) as u16;
            let hi = self.pull8(bus) as u16;
            self.pc = lo | (hi << 8);
        } else {
            let p = self.pull8(bus);
            self.p = p;
            let lo = self.pull8(bus) as u16;
            let hi = self.pull8(bus) as u16;
            let bank = self.pull8(bus);
            self.pc = lo | (hi << 8);
            self.pbr = bank;
        }
        self.normalize_widths();
    }

    // ---- block moves: one byte per step, rewinding PC until A wraps ----

    fn op_mvn<B: Bus>(&mut self, bus: &mut B) {
        // Operands: dest bank, src bank (in that fetch order for MVN).
        let dst_bank = self.fetch8(bus) as u32;
        let src_bank = self.fetch8(bus) as u32;
        self.dbr = dst_bank as u8;
        let src = (src_bank << 16) | self.x as u32;
        let dst = (dst_bank << 16) | self.y as u32;
        let b = bus.read(src);
        bus.write(dst, b);
        bus.idle();
        bus.idle();
        // MVN increments X and Y.
        self.step_index_inc();
        let done = self.a == 0x0000;
        self.a = self.a.wrapping_sub(1);
        if !done {
            // Rewind PC to re-execute this instruction on the next step.
            self.pc = self.pc.wrapping_sub(3);
        }
    }

    fn op_mvp<B: Bus>(&mut self, bus: &mut B) {
        let dst_bank = self.fetch8(bus) as u32;
        let src_bank = self.fetch8(bus) as u32;
        self.dbr = dst_bank as u8;
        let src = (src_bank << 16) | self.x as u32;
        let dst = (dst_bank << 16) | self.y as u32;
        let b = bus.read(src);
        bus.write(dst, b);
        bus.idle();
        bus.idle();
        // MVP decrements X and Y.
        self.step_index_dec();
        let done = self.a == 0x0000;
        self.a = self.a.wrapping_sub(1);
        if !done {
            self.pc = self.pc.wrapping_sub(3);
        }
    }

    fn step_index_inc(&mut self) {
        if self.x8() {
            self.x = (self.x as u8).wrapping_add(1) as u16;
            self.y = (self.y as u8).wrapping_add(1) as u16;
        } else {
            self.x = self.x.wrapping_add(1);
            self.y = self.y.wrapping_add(1);
        }
    }

    fn step_index_dec(&mut self) {
        if self.x8() {
            self.x = (self.x as u8).wrapping_sub(1) as u16;
            self.y = (self.y as u8).wrapping_sub(1) as u16;
        } else {
            self.x = self.x.wrapping_sub(1);
            self.y = self.y.wrapping_sub(1);
        }
    }
}
