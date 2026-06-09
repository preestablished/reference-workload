//! Unit tests for the 65C816-class CPU core, driven against a flat 16 MiB
//! test bus (no SysBus, no other modules' `todo!()` stubs are touched).

use super::{flags, Cpu};
use crate::bus::Bus;
use crate::fault::Fault;

/// Flat 16 MiB memory bus: every address maps to RAM; idle is a no-op; no
/// interrupts asserted. `fault` panics so a misbehaving test fails loudly.
struct FlatBus {
    mem: Vec<u8>,
    nmi: bool,
    irq: bool,
}

impl FlatBus {
    fn new() -> FlatBus {
        FlatBus {
            mem: vec![0u8; 0x100_0000],
            nmi: false,
            irq: false,
        }
    }

    fn load(&mut self, addr: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.mem[((addr + i as u32) & 0xFF_FFFF) as usize] = *b;
        }
    }

    fn set8(&mut self, addr: u32, v: u8) {
        self.mem[(addr & 0xFF_FFFF) as usize] = v;
    }

    fn get8(&self, addr: u32) -> u8 {
        self.mem[(addr & 0xFF_FFFF) as usize]
    }
}

impl Bus for FlatBus {
    fn read(&mut self, addr: u32) -> u8 {
        self.mem[(addr & 0xFF_FFFF) as usize]
    }
    fn write(&mut self, addr: u32, value: u8) {
        self.mem[(addr & 0xFF_FFFF) as usize] = value;
    }
    fn idle(&mut self) {}
    fn take_nmi(&mut self) -> bool {
        let n = self.nmi;
        self.nmi = false;
        n
    }
    fn irq_line(&self) -> bool {
        self.irq
    }
    fn fault(&mut self, fault: Fault) {
        panic!("unexpected fault in test: {:?}", fault);
    }
}

/// Build a CPU in native mode with both M and X clear (16-bit A/X/Y) and a
/// known stack, plus a flat bus. PC starts at $00:8000.
fn native16() -> (Cpu, FlatBus) {
    let mut cpu = Cpu::new();
    cpu.e = false;
    cpu.p = 0; // M=0, X=0 -> 16-bit
    cpu.s = 0x1FFF;
    cpu.d = 0;
    cpu.dbr = 0;
    cpu.pbr = 0;
    cpu.pc = 0x8000;
    (cpu, FlatBus::new())
}

/// Native mode with 8-bit A/X/Y (M=X=1).
fn native8() -> (Cpu, FlatBus) {
    let mut cpu = Cpu::new();
    cpu.e = false;
    cpu.p = flags::M | flags::X;
    cpu.s = 0x1FFF;
    cpu.pc = 0x8000;
    (cpu, FlatBus::new())
}

fn run_one(cpu: &mut Cpu, bus: &mut FlatBus, prog: &[u8]) {
    bus.load(((cpu.pbr as u32) << 16) | cpu.pc as u32, prog);
    cpu.step(bus);
}

// ---------------------------------------------------------------------------
// ADC / SBC binary
// ---------------------------------------------------------------------------

#[test]
fn adc_binary_8bit() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x10;
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0x22]); // ADC #$22
    assert_eq!(cpu.a as u8, 0x32);
    assert_eq!(cpu.p & flags::C, 0);
    assert_eq!(cpu.p & flags::Z, 0);
}

#[test]
fn adc_binary_8bit_carry_and_overflow() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x50;
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0x50]); // 0x50+0x50 = 0xA0
    assert_eq!(cpu.a as u8, 0xA0);
    assert_ne!(cpu.p & flags::V, 0, "signed overflow");
    assert_ne!(cpu.p & flags::N, 0);
    assert_eq!(cpu.p & flags::C, 0);
}

#[test]
fn adc_binary_16bit() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0x1234;
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0xCD, 0xAB]); // ADC #$ABCD
    assert_eq!(cpu.a, 0xBE01);
    assert_eq!(cpu.p & flags::C, 0);
}

#[test]
fn sbc_binary_8bit() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x50;
    cpu.set_flag(flags::C, true); // no borrow
    run_one(&mut cpu, &mut bus, &[0xE9, 0x30]); // SBC #$30
    assert_eq!(cpu.a as u8, 0x20);
    assert_ne!(cpu.p & flags::C, 0, "no borrow -> C set");
}

#[test]
fn sbc_binary_16bit_borrow() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0x0000;
    cpu.set_flag(flags::C, true);
    run_one(&mut cpu, &mut bus, &[0xE9, 0x01, 0x00]); // SBC #$0001
    assert_eq!(cpu.a, 0xFFFF);
    assert_eq!(cpu.p & flags::C, 0, "borrow -> C clear");
    assert_ne!(cpu.p & flags::N, 0);
}

// ---------------------------------------------------------------------------
// ADC / SBC decimal (BCD)
// ---------------------------------------------------------------------------

#[test]
fn adc_bcd_8bit_99_plus_01() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x99;
    cpu.set_flag(flags::D, true);
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0x01]); // 99 + 01 = 00, carry
    assert_eq!(cpu.a as u8, 0x00);
    assert_ne!(cpu.p & flags::C, 0);
    assert_ne!(cpu.p & flags::Z, 0);
}

#[test]
fn adc_bcd_8bit_simple() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x25;
    cpu.set_flag(flags::D, true);
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0x48]); // 25 + 48 = 73
    assert_eq!(cpu.a as u8, 0x73);
    assert_eq!(cpu.p & flags::C, 0);
}

#[test]
fn sbc_bcd_8bit_00_minus_01() {
    let (mut cpu, mut bus) = native8();
    cpu.a = 0x00;
    cpu.set_flag(flags::D, true);
    cpu.set_flag(flags::C, true); // no borrow in
    run_one(&mut cpu, &mut bus, &[0xE9, 0x01]); // 00 - 01 = 99, borrow
    assert_eq!(cpu.a as u8, 0x99);
    assert_eq!(cpu.p & flags::C, 0, "borrow occurred");
}

#[test]
fn adc_bcd_16bit() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0x1234;
    cpu.set_flag(flags::D, true);
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0x69, 0x66, 0x87]); // 1234 + 8766 = 0000 c
    assert_eq!(cpu.a, 0x0000);
    assert_ne!(cpu.p & flags::C, 0);
    assert_ne!(cpu.p & flags::Z, 0);
}

#[test]
fn sbc_bcd_16bit() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0x1000;
    cpu.set_flag(flags::D, true);
    cpu.set_flag(flags::C, true);
    run_one(&mut cpu, &mut bus, &[0xE9, 0x01, 0x00]); // 1000 - 0001 = 0999
    assert_eq!(cpu.a, 0x0999);
    assert_ne!(cpu.p & flags::C, 0);
}

// ---------------------------------------------------------------------------
// REP / SEP width switching, X-flag high-byte truncation
// ---------------------------------------------------------------------------

#[test]
fn sep_rep_width_switch() {
    let (mut cpu, mut bus) = native16();
    run_one(&mut cpu, &mut bus, &[0xE2, 0x30]);
    assert!(cpu.m8());
    assert!(cpu.x8());
    cpu.pc = 0x8000;
    run_one(&mut cpu, &mut bus, &[0xC2, 0x30]);
    assert!(!cpu.m8());
    assert!(!cpu.x8());
}

#[test]
fn sep_x_truncates_index_high_byte() {
    let (mut cpu, mut bus) = native16();
    cpu.x = 0xABCD;
    cpu.y = 0x1234;
    run_one(&mut cpu, &mut bus, &[0xE2, 0x10]); // SEP #$10 -> X flag set
    assert_eq!(cpu.x, 0x00CD);
    assert_eq!(cpu.y, 0x0034);
}

// ---------------------------------------------------------------------------
// XCE transitions
// ---------------------------------------------------------------------------

#[test]
fn xce_native_to_emulation() {
    let (mut cpu, mut bus) = native16();
    cpu.set_flag(flags::C, true);
    run_one(&mut cpu, &mut bus, &[0xFB]); // XCE
    assert!(cpu.e);
    assert!(cpu.m8());
    assert!(cpu.x8());
    assert_eq!(cpu.s & 0xFF00, 0x0100, "S forced to page 1");
}

#[test]
fn xce_emulation_to_native() {
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000;
    let mut bus = FlatBus::new();
    cpu.set_flag(flags::C, false);
    run_one(&mut cpu, &mut bus, &[0xFB]);
    assert!(!cpu.e, "now native");
    assert_ne!(cpu.p & flags::C, 0, "old E (1) moved into C");
}

// ---------------------------------------------------------------------------
// MVN block move
// ---------------------------------------------------------------------------

#[test]
fn mvn_moves_block() {
    let (mut cpu, mut bus) = native16();
    bus.load(0x01_0000, &[0xDE, 0xAD, 0xBE, 0xEF]);
    cpu.x = 0x0000;
    cpu.y = 0x0000;
    cpu.a = 0x0003;
    let prog = [0x54, 0x02, 0x01];
    bus.load(0x8000, &prog);
    for _ in 0..4 {
        cpu.step(&mut bus);
    }
    assert_eq!(bus.get8(0x02_0000), 0xDE);
    assert_eq!(bus.get8(0x02_0001), 0xAD);
    assert_eq!(bus.get8(0x02_0002), 0xBE);
    assert_eq!(bus.get8(0x02_0003), 0xEF);
    assert_eq!(cpu.a, 0xFFFF);
    assert_eq!(cpu.dbr, 0x02);
    assert_eq!(cpu.pc, 0x8003, "PC past the instruction when done");
}

// ---------------------------------------------------------------------------
// JSR/RTS and JSL/RTL round trips
// ---------------------------------------------------------------------------

#[test]
fn jsr_rts_roundtrip() {
    let (mut cpu, mut bus) = native16();
    bus.load(0x8000, &[0x20, 0x00, 0x90]);
    bus.load(0x9000, &[0x60]);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x9000);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8003, "returns past the JSR operand");
}

#[test]
fn jsl_rtl_roundtrip() {
    let (mut cpu, mut bus) = native16();
    bus.load(0x8000, &[0x22, 0x00, 0x90, 0x01]);
    bus.load(0x01_9000, &[0x6B]);
    cpu.step(&mut bus);
    assert_eq!(cpu.pbr, 0x01);
    assert_eq!(cpu.pc, 0x9000);
    cpu.step(&mut bus);
    assert_eq!(cpu.pbr, 0x00);
    assert_eq!(cpu.pc, 0x8004, "returns past the 4-byte JSL");
}

// ---------------------------------------------------------------------------
// Stack-relative addressing
// ---------------------------------------------------------------------------

#[test]
fn stack_relative_load() {
    let (mut cpu, mut bus) = native8();
    cpu.s = 0x1F00;
    bus.set8(0x1F02, 0x7E);
    run_one(&mut cpu, &mut bus, &[0xA3, 0x02]); // LDA $02,S
    assert_eq!(cpu.a as u8, 0x7E);
}

// ---------------------------------------------------------------------------
// BRK entry / RTI in native mode, IRQ masking
// ---------------------------------------------------------------------------

#[test]
fn brk_rti_native() {
    let (mut cpu, mut bus) = native16();
    cpu.p = flags::M | flags::X;
    cpu.set_flag(flags::D, true);
    bus.load(0x00FFE6, &[0x00, 0xA0]);
    bus.load(0x8000, &[0x00, 0xEA]);
    bus.load(0xA000, &[0x40]);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0xA000);
    assert_ne!(cpu.p & flags::I, 0, "I set on entry");
    assert_eq!(cpu.p & flags::D, 0, "D cleared on entry");
    assert_eq!(cpu.pbr, 0x00);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8002);
    assert_ne!(cpu.p & flags::D, 0, "D restored by RTI");
}

#[test]
fn irq_masked_by_i_flag() {
    let (mut cpu, mut bus) = native16();
    cpu.set_flag(flags::I, true);
    bus.irq = true;
    bus.load(0x00FFEE, &[0x00, 0xB0]);
    bus.load(0x8000, &[0xEA]);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0x8001);
}

#[test]
fn irq_serviced_when_unmasked() {
    let (mut cpu, mut bus) = native16();
    cpu.set_flag(flags::I, false);
    bus.irq = true;
    bus.load(0x00FFEE, &[0x00, 0xB0]);
    cpu.step(&mut bus);
    assert_eq!(cpu.pc, 0xB000);
    assert_ne!(cpu.p & flags::I, 0);
}

// ---------------------------------------------------------------------------
// WAI release on NMI
// ---------------------------------------------------------------------------

#[test]
fn wai_releases_on_nmi() {
    let (mut cpu, mut bus) = native16();
    cpu.waiting = true;
    bus.nmi = true;
    bus.load(0x00FFEA, &[0x00, 0xC0]);
    cpu.step(&mut bus);
    assert!(!cpu.waiting);
    assert_eq!(cpu.pc, 0xC000);
}

#[test]
fn wai_idles_without_interrupt() {
    let (mut cpu, mut bus) = native16();
    cpu.waiting = true;
    cpu.pc = 0x8000;
    cpu.step(&mut bus);
    assert!(cpu.waiting, "still waiting");
    assert_eq!(cpu.pc, 0x8000, "PC unchanged while waiting");
}

// ---------------------------------------------------------------------------
// Direct-page wrap with E=1, DL=0
// ---------------------------------------------------------------------------

#[test]
fn dp_indexed_wrap_emulation() {
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000;
    cpu.d = 0x0000;
    cpu.x = 0xFF;
    let mut bus = FlatBus::new();
    bus.set8(0x0004, 0x42);
    bus.set8(0x0104, 0x99);
    run_one(&mut cpu, &mut bus, &[0xB5, 0x05]); // LDA $05,X
    assert_eq!(cpu.a as u8, 0x42, "direct page wrapped within page 0");
}

#[test]
fn dp_no_wrap_when_dl_nonzero() {
    let mut cpu = Cpu::new();
    cpu.pc = 0x8000;
    cpu.d = 0x0010;
    cpu.x = 0xFF;
    let mut bus = FlatBus::new();
    bus.set8(0x0114, 0x77);
    run_one(&mut cpu, &mut bus, &[0xB5, 0x05]);
    assert_eq!(cpu.a as u8, 0x77);
}

// ---------------------------------------------------------------------------
// Absolute indexed page-cross path executes correctly
// ---------------------------------------------------------------------------

#[test]
fn absolute_x_page_cross() {
    let (mut cpu, mut bus) = native8();
    cpu.x = 0x05;
    cpu.dbr = 0x00;
    bus.set8(0x2104, 0x5A);
    run_one(&mut cpu, &mut bus, &[0xBD, 0xFF, 0x20]);
    assert_eq!(cpu.a as u8, 0x5A);
}

// ---------------------------------------------------------------------------
// Misc: XBA, PHA/PLA round trip, reset
// ---------------------------------------------------------------------------

#[test]
fn xba_swaps_bytes() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0x12FF;
    run_one(&mut cpu, &mut bus, &[0xEB]); // XBA
    assert_eq!(cpu.a, 0xFF12);
    assert_eq!(cpu.p & flags::N, 0);
    assert_eq!(cpu.p & flags::Z, 0);
}

#[test]
fn pha_pla_16bit() {
    let (mut cpu, mut bus) = native16();
    cpu.a = 0xCAFE;
    bus.load(0x8000, &[0x48]);
    cpu.step(&mut bus);
    cpu.a = 0x0000;
    bus.load(cpu.pc as u32, &[0x68]);
    cpu.step(&mut bus);
    assert_eq!(cpu.a, 0xCAFE);
}

#[test]
fn reset_loads_vector() {
    let mut cpu = Cpu::new();
    let mut bus = FlatBus::new();
    bus.load(0x00FFFC, &[0x34, 0x12]);
    cpu.reset(&mut bus);
    assert_eq!(cpu.pc, 0x1234);
    assert!(cpu.e);
    assert_ne!(cpu.p & flags::I, 0);
    assert_eq!(cpu.p & flags::D, 0);
}
