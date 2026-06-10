//! SPC700 hardware timers.
//!
//! The SPC700 has three hardware timers:
//!
//! - Timer 0 ($FA target, $FD output): divides the SPC clock by 128 (~8 kHz
//!   when the SPC runs at ~1.024 MHz).
//! - Timer 1 ($FB target, $FE output): same divisor of 128.
//! - Timer 2 ($FC target, $FF output): divides by 16 (~64 kHz).
//!
//! Each timer has:
//!
//! - An 8-bit **target register** ($FA–$FC), writable. A target of `0` is
//!   treated as 256.
//! - An 8-bit internal **stage counter** (not directly readable), which
//!   increments at the divided rate while the timer is enabled.
//! - A 4-bit **output counter** ($FD–$FF), which increments when the stage
//!   reaches the target, and clears (to 0) when read.
//!
//! Enable semantics ($F1 bits 0/1/2):
//!
//! - While disabled, the stage counter does not increment.
//! - On a 0 → 1 transition of the enable bit, **both** the stage counter and
//!   the output counter are cleared.
//! - On a 1 → 1 (already enabled) write, counters are NOT cleared.
//!
//! The output counter wraps at 16 (4-bit).
//!
//! ## Clock units
//!
//! Callers pass a `ticks` argument representing the number of SPC700 master
//! clock ticks elapsed. The timers divide internally:
//!
//! - Timers 0/1: fire every 128 ticks (divider constant `DIVIDER_01`).
//! - Timer 2: fires every 16 ticks (divider constant `DIVIDER_2`).
//!
//! This is package-01 scope; cycle-accurate scheduling against the 65C816
//! master clock is package-02's job.

/// Clock divider for timers 0 and 1 (~8 kHz from the ~1.024 MHz SPC clock).
pub const DIVIDER_01: u32 = 128;

/// Clock divider for timer 2 (~64 kHz from the ~1.024 MHz SPC clock).
pub const DIVIDER_2: u32 = 16;

/// One SPC700 hardware timer.
///
/// The divider (clock subdivision) and the target/output registers are all
/// plain integer fields — no floating-point involved (D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timer {
    /// Counts ticks since the last stage increment; fires when it reaches
    /// `divider`.
    tick_accum: u32,
    /// Clock divisor: 128 for timers 0/1, 16 for timer 2.
    pub divider: u32,
    /// 8-bit target register ($FA/$FB/$FC). A value of 0 is treated as 256.
    pub target: u8,
    /// 8-bit internal stage counter (up-counter, not directly readable by
    /// software).
    pub stage: u8,
    /// 4-bit output counter ($FD/$FE/$FF). Cleared on read; wraps at 16.
    output: u8,
    /// Whether this timer is currently enabled (bit set in $F1).
    pub enabled: bool,
}

impl Timer {
    /// Create a new timer with the given clock divisor.
    pub fn new(divider: u32) -> Self {
        Timer {
            tick_accum: 0,
            divider,
            target: 0,
            stage: 0,
            output: 0,
            enabled: false,
        }
    }

    /// Enable or disable this timer. A 0 → 1 transition clears both the
    /// stage and output counters per the documented hardware behaviour.
    pub fn set_enabled(&mut self, enable: bool) {
        if enable && !self.enabled {
            // Rising edge: clear both counters.
            self.stage = 0;
            self.output = 0;
            self.tick_accum = 0;
        }
        self.enabled = enable;
    }

    /// Advance the timer by `ticks` SPC700 master-clock cycles. Returns the
    /// number of times the stage counter overflowed (≥ 0). The caller may
    /// ignore this return value; it is provided for test purposes.
    pub fn advance(&mut self, ticks: u32) -> u32 {
        if !self.enabled {
            return 0;
        }
        self.tick_accum += ticks;
        let mut overflows = 0u32;
        while self.tick_accum >= self.divider {
            self.tick_accum -= self.divider;
            // Stage increments; check against target (0 treated as 256).
            // Stage increments; when it reaches the target (0 means 256), it
            // resets and the output counter ticks. The stage is an 8-bit counter,
            // so target=0 fires naturally when stage wraps from 255 back to 0.
            let prev_stage = self.stage;
            self.stage = self.stage.wrapping_add(1);
            let effective_target = self.target as u32;
            // Fire condition: stage just reached the target value.
            //   target != 0: fire when stage == target.
            //   target == 0: fire when stage wraps (prev==255, stage==0 → i.e., stage as
            //                u32 == 0 after wrapping past 255 = effective 256).
            let fired = if self.target == 0 {
                // Stage wrapped at 256.
                prev_stage == 0xFF && self.stage == 0
            } else {
                self.stage as u32 >= effective_target
            };
            if fired {
                self.stage = 0;
                self.output = self.output.wrapping_add(1) & 0x0F;
                overflows += 1;
            }
        }
        overflows
    }

    /// Read the 4-bit output counter and clear it (documented hardware
    /// behaviour: reading $FD–$FF returns the counter and resets it to 0).
    pub fn read_output(&mut self) -> u8 {
        let v = self.output & 0x0F;
        self.output = 0;
        v
    }

    /// Write the 8-bit target register ($FA–$FC). Does NOT restart the timer.
    #[inline]
    pub fn write_target(&mut self, value: u8) {
        self.target = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Timer 0/1 (divider = 128) ----

    #[test]
    fn timer_disabled_does_not_advance() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1);
        // No enable → advance should be a no-op.
        t.advance(1000);
        assert_eq!(t.read_output(), 0);
        assert_eq!(t.stage, 0);
    }

    #[test]
    fn timer_fires_after_128_ticks() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1); // target=1 → fires after every stage increment
        t.set_enabled(true);
        // 127 ticks: should not have fired yet.
        t.advance(127);
        assert_eq!(t.read_output(), 0);
        // 1 more tick: should fire once.
        t.advance(1);
        assert_eq!(t.read_output(), 1);
    }

    #[test]
    fn output_cleared_on_read() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1);
        t.set_enabled(true);
        t.advance(128 * 3); // 3 overflows
        let out = t.read_output();
        assert_eq!(out, 3);
        // Second read must return 0.
        assert_eq!(t.read_output(), 0);
    }

    #[test]
    fn output_wraps_at_16() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1);
        t.set_enabled(true);
        // 17 overflows → counter wraps: 17 & 0x0F = 1
        t.advance(128 * 17);
        assert_eq!(t.read_output(), 17 & 0x0F);
    }

    #[test]
    fn target_zero_treated_as_256() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(0); // 0 means 256
        t.set_enabled(true);
        // 256 stage increments → 1 output overflow.
        t.advance(128 * 255);
        assert_eq!(t.read_output(), 0, "should not have fired yet");
        t.advance(128);
        assert_eq!(t.read_output(), 1, "should fire after 256 increments");
    }

    #[test]
    fn enable_rising_edge_clears_counters() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1);
        t.set_enabled(true);
        t.advance(128 * 5); // build up some output
                            // Disable then re-enable: should clear.
        t.set_enabled(false);
        t.set_enabled(true);
        assert_eq!(t.read_output(), 0, "output cleared on rising edge");
        assert_eq!(t.stage, 0, "stage cleared on rising edge");
    }

    #[test]
    fn already_enabled_write_does_not_clear() {
        let mut t = Timer::new(DIVIDER_01);
        t.write_target(1);
        t.set_enabled(true);
        t.advance(128 * 3); // output = 3
                            // Write enabled again while already enabled: should NOT clear output.
        t.set_enabled(true);
        assert_eq!(t.read_output(), 3, "output preserved on non-rising edge");
    }

    // ---- Timer 2 (divider = 16) ----

    #[test]
    fn timer2_fires_after_16_ticks() {
        let mut t = Timer::new(DIVIDER_2);
        t.write_target(1);
        t.set_enabled(true);
        t.advance(15);
        assert_eq!(t.read_output(), 0);
        t.advance(1);
        assert_eq!(t.read_output(), 1);
    }

    #[test]
    fn timer2_target_multiple_fires() {
        let mut t = Timer::new(DIVIDER_2);
        t.write_target(4); // fires every 4 stage increments = every 64 ticks
        t.set_enabled(true);
        t.advance(64);
        assert_eq!(t.read_output(), 1);
        t.advance(64 * 2);
        assert_eq!(t.read_output(), 2);
    }
}
