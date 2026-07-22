//! `refwork-emu` — deterministic 16-bit-console emulator core.
//!
//! Implements the emulator determinism contract (reference-workload
//! ARCHITECTURE.md §1, rules D1–D9):
//!
//! - **D1** single-threaded: no threads, no async, anywhere in this crate.
//! - **D2** zero wall-clock reads: no `Instant`, `SystemTime`, sleeps.
//! - **D3** no RNG: uninitialized-RAM contents are a fixed documented
//!   pattern ([`WRAM_INIT_BYTE`]); open bus returns the last bus value.
//! - **D4** no floats: no `f32`/`f64` tokens in this crate (CI-enforced).
//! - **D5** all state in plain memory: every byte of emulator state lives in
//!   ordinary struct fields / owned buffers.
//! - **D8** allocation-stable: everything is allocated in [`Core::new`];
//!   zero allocations per frame thereafter (CI-enforced with a counting
//!   allocator).
//! - **D9** fail loudly: any contract-relevant anomaly is a [`Fault`] and a
//!   halt, never a silent fallback.
//!
//! The crate has exactly one production consumer shape: construct a
//! [`Core`] with a [`Cartridge`] and externally-owned [`RegionBuffers`],
//! then drive `run_one_frame(pad)` / `blit_completed_frame(..)` from the
//! harness frame loop (ARCHITECTURE.md §3).

#![forbid(unsafe_code)]

mod apu;
mod bus;
pub mod cart;
mod core_impl;
mod cpu;
mod dma;
mod fault;
mod joypad;
mod ppu;
mod timing;

pub use cart::Cartridge;
pub use core_impl::{Core, CoreError, RegionBuffers};
pub use fault::{Fault, FrameFlags};
pub use timing::{FB_BYTES, FB_HEIGHT, FB_STRIDE, FB_WIDTH};

/// Fixed WRAM power-on fill pattern (D3): two boots are byte-identical.
pub const WRAM_INIT_BYTE: u8 = 0x55;

/// Emulator core version string, published in the `meta` region by the
/// harness (API.md §3.6) and recorded in determinism reports.
///
/// - 0.2.0 = APU clock epoch (2026-07-16 decision; SPC debt-carry + DSP
///   fidelity fixes).
/// - 0.2.1 = HDMA mid-frame enable (same 2026-07-16 epoch, still
///   pre-re-baseline; disambiguates recordings made under 0.2.0).
/// - 0.2.2 = CGWSEL clip-region + window semantics (same 2026-07-16 epoch).
pub const EMU_VERSION: &str = "refwork-emu 0.2.2";

/// Nominal sample rate, in Hz, of the stream drained by
/// [`Core::take_audio_samples`]. Derived from the DSP clock model
/// (`apu::DSP_CLOCKS_PER_SAMPLE = 32` SPC700 cycles/sample, `apu::SPC_NUM`
/// / `apu::SPC_DEN` ≈ 1.024 MHz nominal SPC700 rate): one DSP sample every
/// 32 SPC cycles at ~1.024 MHz gives ~32,000 Hz. The model's true rate is
/// ≈32,000.4 Hz (0.0013% off this constant, inaudible); frontends must use
/// this constant rather than hardcoding 32000 independently.
#[cfg(feature = "audio")]
pub const AUDIO_SAMPLE_RATE_HZ: u32 = 32_000;

// Test-only exports for the single-step CPU test runner (`xtask cpu-tests`),
// the SPC700 single-step runner (`xtask spc-tests`), and golden-trace tooling.
// Never part of the guest build.
#[cfg(feature = "introspect")]
pub mod introspect {
    pub use crate::apu::spc700::{ApuHalt, Spc700};
    pub use crate::apu::Apu;
    pub use crate::bus::Bus;
    pub use crate::core_impl::DiagSnapshot;
    pub use crate::cpu::Cpu;
}
