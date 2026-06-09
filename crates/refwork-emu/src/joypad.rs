//! Controller port emulation: platform pad bitmask (API.md §3.4) mapped to
//! the console's native auto-joypad-read order, plus the manual serial
//! interface ($4016/$4017) so games may use either path (D6: the word is
//! latched once per frame by the core; this module only translates).
//!
//! OWNER (implementation): integration agent.
//!
//! Platform bit order (API.md §3.4, canonical): bit 0..11 =
//! A B X Y L R Up Down Left Right Start Select; 12-15 reserved (0).
//!
//! Native order: JOY1L ($4218) bit7..0 = A X L R 0 0 0 0;
//! JOY1H ($4219) bit7..0 = B Y Select Start Up Down Left Right.
//! Manual serial read order (after latch): B, Y, Select, Start, Up, Down,
//! Left, Right, A, X, L, R, then four 0 bits, then 1s.

/// Controller state for port 1 (ports 2-4 read as not-connected).
pub struct Joypad {
    /// Platform-order pad word latched for the current frame.
    pub pad: u16,
    /// Auto-joypad result registers ($4218/$4219), updated at v-blank when
    /// auto-read is enabled.
    pub joy1: u16,
    /// $4016 write bit0 latch state (strobe).
    pub strobe: bool,
    /// Serial shift position for manual $4016 reads.
    pub shift: u8,
}

// Platform bit positions (API.md §3.4):
//   bit 0  = A
//   bit 1  = B
//   bit 2  = X
//   bit 3  = Y
//   bit 4  = L
//   bit 5  = R
//   bit 6  = Up
//   bit 7  = Down
//   bit 8  = Left
//   bit 9  = Right
//   bit 10 = Start
//   bit 11 = Select

impl Joypad {
    pub fn new() -> Joypad {
        Joypad {
            pad: 0,
            joy1: 0,
            strobe: false,
            shift: 0,
        }
    }

    /// Convert the latched platform-order word to native JOY1 order.
    ///
    /// JOY1L ($4218) bit7..0 = A  X  L  R  0  0  0  0
    /// JOY1H ($4219) bit7..0 = B  Y  Sel Sta Up Dn L  R
    ///
    /// Returned as (JOY1H << 8) | JOY1L so that $4218 = low byte, $4219 = high byte.
    pub fn native_word(&self) -> u16 {
        let a = self.pad & 1;
        let b = (self.pad >> 1) & 1;
        let x = (self.pad >> 2) & 1;
        let y = (self.pad >> 3) & 1;
        let l = (self.pad >> 4) & 1;
        let r = (self.pad >> 5) & 1;
        let up = (self.pad >> 6) & 1;
        let down = (self.pad >> 7) & 1;
        let left = (self.pad >> 8) & 1;
        let right = (self.pad >> 9) & 1;
        let start = (self.pad >> 10) & 1;
        let select = (self.pad >> 11) & 1;

        // JOY1L ($4218): A X L R 0 0 0 0
        let lo: u16 = (a << 7) | (x << 6) | (l << 5) | (r << 4);

        // JOY1H ($4219): B Y Select Start Up Down Left Right
        let hi: u16 = (b << 7)
            | (y << 6)
            | (select << 5)
            | (start << 4)
            | (up << 3)
            | (down << 2)
            | (left << 1)
            | right;

        (hi << 8) | lo
    }

    /// Auto-joypad read (v-blank, NMITIMEN bit0): latch `joy1`.
    pub fn auto_read(&mut self) {
        self.joy1 = self.native_word();
        self.shift = 0;
    }

    /// CPU write to $4016 (bit0 = strobe).
    pub fn write_strobe(&mut self, value: u8) {
        let new_strobe = value & 1 != 0;
        // 1→0 transition: latch current pad state and reset shift register.
        if self.strobe && !new_strobe {
            self.joy1 = self.native_word();
            self.shift = 0;
        }
        self.strobe = new_strobe;
    }

    /// CPU read of $4016 (port 1 serial; bit0 = data, returns 1s after 16
    /// bits). $4017 reads as not-connected per the bus.
    ///
    /// Serial read order: B, Y, Select, Start, Up, Down, Left, Right, A, X, L, R,
    /// then four 0 bits (bits 12..15), then 1s.
    pub fn read_serial(&mut self) -> u8 {
        // While strobe is held high, reads return the first bit (B) repeatedly.
        if self.strobe {
            // B is bit 7 of JOY1H which is bit 15 of joy1.
            return ((self.joy1 >> 15) & 1) as u8;
        }

        if self.shift >= 16 {
            // Past the 16-bit word: return 1 (open bus / inactive).
            return 1;
        }

        // Serial bit order matches the auto-joypad order starting from MSB of joy1.
        // joy1 = (JOY1H << 8) | JOY1L
        // Bit 15..8 of joy1 = JOY1H = B Y Sel Sta Up Dn L R
        // Bit 7..0  of joy1 = JOY1L = A X L R 0 0 0 0
        //
        // Serial order: B(15) Y(14) Sel(13) Sta(12) Up(11) Dn(10) L(9) R(8)
        //               A(7)  X(6)  L(5)   R(4)    0(3)   0(2)   0(1) 0(0)
        // Shift 0 → bit 15, shift 1 → bit 14, ..., shift 15 → bit 0.
        let bit_pos = 15u8.saturating_sub(self.shift);
        let data = ((self.joy1 >> bit_pos) & 1) as u8;
        self.shift += 1;
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_word_all_released() {
        let j = Joypad::new();
        assert_eq!(j.native_word(), 0x0000);
    }

    #[test]
    fn native_word_all_pressed() {
        let mut j = Joypad::new();
        // All 12 buttons pressed.
        j.pad = 0x0FFF;
        let w = j.native_word();
        // JOY1L: A(7) X(6) L(5) R(4) = 0xF0
        assert_eq!(w & 0xFF, 0xF0, "JOY1L mismatch: {:#04x}", w & 0xFF);
        // JOY1H: B(7) Y(6) Sel(5) Sta(4) Up(3) Dn(2) L(1) R(0) = 0xFF
        assert_eq!(
            (w >> 8) & 0xFF,
            0xFF,
            "JOY1H mismatch: {:#04x}",
            (w >> 8) & 0xFF
        );
    }

    #[test]
    fn native_word_a_only() {
        let mut j = Joypad::new();
        j.pad = 1 << 0; // A only
        let w = j.native_word();
        // A → JOY1L bit 7
        assert_eq!(w & 0xFF, 0x80);
        assert_eq!((w >> 8) & 0xFF, 0x00);
    }

    #[test]
    fn native_word_b_only() {
        let mut j = Joypad::new();
        j.pad = 1 << 1; // B only
        let w = j.native_word();
        // B → JOY1H bit 7
        assert_eq!(w & 0xFF, 0x00);
        assert_eq!((w >> 8) & 0xFF, 0x80);
    }

    #[test]
    fn serial_read_order() {
        let mut j = Joypad::new();
        // Press B only: joy1 bit 15 set.
        j.pad = 1 << 1; // B
        j.write_strobe(1); // strobe high
        j.write_strobe(0); // 1→0 → latch
                           // Serial order: B first.
        assert_eq!(j.read_serial(), 1, "bit 0 (B) should be 1");
        // Remaining bits 1..15 should be 0.
        for i in 1..16 {
            assert_eq!(j.read_serial(), 0, "bit {} should be 0", i);
        }
        // After 16 reads: returns 1.
        assert_eq!(j.read_serial(), 1);
    }

    #[test]
    fn serial_strobe_holds_first_bit() {
        let mut j = Joypad::new();
        j.pad = 0x0FFF; // all pressed
        j.write_strobe(1); // hold strobe high → latch and hold
                           // B is in JOY1H bit 7 = joy1 bit 15.
        j.joy1 = j.native_word();
        // While strobe held, always return first bit (B = 1).
        assert_eq!(j.read_serial(), 1);
        assert_eq!(j.read_serial(), 1);
    }

    #[test]
    fn auto_read_latches() {
        let mut j = Joypad::new();
        j.pad = 1 << 0; // A
        j.auto_read();
        // JOY1L bit 7 should be set (A).
        assert_eq!(j.joy1 & 0x0080, 0x0080);
    }
}
