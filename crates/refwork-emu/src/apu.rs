//! Stub APU (M1): deterministic canned handshake responses on the four
//! CPU↔APU I/O ports ($2140-$2143). Flagged via `FrameFlags`; replaced by
//! the full audio CPU + DSP in M2 (D4: fixed-point only, no floats).
//!
//! OWNER (implementation): integration agent.
//!
//! Behavior: models the boot-ROM upload handshake deterministically —
//! ready signature (`$AA`/`$BB` on ports 0/1) out of reset, then the
//! counter-echo transfer protocol (each data byte written to port 1 is
//! acknowledged by echoing the index byte written to port 0), accepting
//! "next block" and "start execution" transitions. After the start-execution
//! kick, every port reads back the last value the CPU wrote to it (pure
//! echo) — enough for boot code that spin-waits on its own bytes, useless
//! for real drivers (hence the flag).

/// See module docs.
pub struct ApuStub {
    /// Last value the CPU wrote to each port.
    pub from_cpu: [u8; 4],
    /// Value each port presents to CPU reads.
    pub to_cpu: [u8; 4],
    /// Handshake state machine (integration agent defines variants).
    pub state: ApuState,
    /// Set when any port was accessed since the last frame-flag harvest.
    pub accessed: bool,
    /// Set when the handshake state machine advanced since the last harvest.
    pub handshake_activity: bool,

    // Internal tracking for the Transfer state:
    // Last index byte acknowledged on port 0.
    last_index: u8,
}

/// Boot-handshake protocol states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuState {
    /// Presenting the $AA/$BB ready signature.
    Ready,
    /// Transfer loop: echoing index bytes written to port 0.
    Transfer,
    /// Post-kick echo mode.
    Echo,
}

impl ApuStub {
    /// Power-on: ready signature presented.
    pub fn new() -> ApuStub {
        ApuStub {
            from_cpu: [0; 4],
            // Power-on: ports 0/1 show $AA/$BB per documented boot-ROM ready signature.
            to_cpu: [0xAA, 0xBB, 0, 0],
            state: ApuState::Ready,
            accessed: false,
            handshake_activity: false,
            last_index: 0,
        }
    }

    /// CPU read of port 0..=3 ($2140+port; mirrors handled by the bus).
    pub fn read(&mut self, port: u8) -> u8 {
        self.accessed = true;
        self.to_cpu[port as usize & 3]
    }

    /// CPU write of port 0..=3.
    pub fn write(&mut self, port: u8, value: u8) {
        let port = port as usize & 3;
        self.accessed = true;
        self.from_cpu[port] = value;

        match self.state {
            ApuState::Ready => {
                // Documented boot kick:
                // CPU writes $CC to port 0 (with port 1 nonzero and port 2/3
                // carrying the load address) → acknowledge by echoing $CC on
                // port 0 and transition to Transfer.
                if port == 0 && value == 0xCC {
                    self.to_cpu[0] = 0xCC;
                    self.last_index = 0;
                    self.state = ApuState::Transfer;
                    self.handshake_activity = true;
                }
            }
            ApuState::Transfer => {
                // Transfer per-byte protocol:
                // CPU writes data to port 1, then writes the running index to
                // port 0 as a trigger.  We echo the index back on port 0.
                //
                // Start-execution detection — a boot-ROM-only approximation
                // (M1 stub): a port 0 write where the new index jumps by ≥2
                // from the last acknowledged index while port 1 == 0 is
                // treated as the "start execution" command. M2's real audio
                // unit replaces this; M2 acceptance must reject runs that
                // carry APU_STUB_HANDSHAKE frames.
                if port == 0 {
                    let new_index = value;
                    let delta = new_index.wrapping_sub(self.last_index);
                    if delta >= 2 && self.from_cpu[1] == 0 {
                        // Start execution kick — enter Echo mode.
                        self.state = ApuState::Echo;
                        self.handshake_activity = true;
                        // In Echo mode all ports reflect what the CPU last wrote.
                        for i in 0..4 {
                            self.to_cpu[i] = self.from_cpu[i];
                        }
                    } else {
                        // Normal per-byte ack: echo the index byte on port 0.
                        self.to_cpu[0] = new_index;
                        self.last_index = new_index;
                        self.handshake_activity = true;
                    }
                }
            }
            ApuState::Echo => {
                // Post-kick: every CPU write is immediately echoed back.
                self.to_cpu[port] = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_on_signature() {
        let apu = ApuStub::new();
        assert_eq!(apu.to_cpu[0], 0xAA);
        assert_eq!(apu.to_cpu[1], 0xBB);
        assert_eq!(apu.state, ApuState::Ready);
    }

    #[test]
    fn boot_handshake_kick() {
        let mut apu = ApuStub::new();
        // Write non-zero to port1 and address to port2/3.
        apu.write(1, 0x01);
        apu.write(2, 0x00);
        apu.write(3, 0x02);
        // Write $CC to port0 → should acknowledge.
        apu.write(0, 0xCC);
        assert_eq!(apu.state, ApuState::Transfer);
        assert_eq!(apu.to_cpu[0], 0xCC);
        assert!(apu.handshake_activity);
    }

    #[test]
    fn transfer_per_byte_ack() {
        let mut apu = ApuStub::new();
        apu.write(1, 0x01);
        apu.write(0, 0xCC);
        assert_eq!(apu.state, ApuState::Transfer);
        apu.handshake_activity = false;

        // Write data byte to port 1, then index 1 to port 0.
        apu.write(1, 0xDE);
        apu.write(0, 0x01);
        assert_eq!(apu.to_cpu[0], 0x01); // echoed index
        assert!(apu.handshake_activity);
    }

    #[test]
    fn start_execution_kick() {
        let mut apu = ApuStub::new();
        apu.write(1, 0x01);
        apu.write(0, 0xCC);
        // Send one block (index 1).
        apu.write(1, 0xDE);
        apu.write(0, 0x01);
        apu.handshake_activity = false;

        // Start execution: port1=0, index jumps by ≥2 from last ack (was 1).
        apu.write(1, 0x00);
        apu.write(0, 0x10); // delta = 15 >= 2
        assert_eq!(apu.state, ApuState::Echo);
        assert!(apu.handshake_activity);
    }

    #[test]
    fn echo_mode_reflects_writes() {
        let mut apu = ApuStub::new();
        apu.write(1, 0x01);
        apu.write(0, 0xCC);
        apu.write(1, 0x00);
        apu.write(0, 0xFF); // start execution
        assert_eq!(apu.state, ApuState::Echo);

        apu.write(2, 0xAB);
        assert_eq!(apu.read(2), 0xAB);
    }
}
