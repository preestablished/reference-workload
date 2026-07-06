//! Clean-room 64-byte IPL boot shim for SPC700 corpus tests.
//!
//! Production execution uses `Apu::step_ipl_hle` for the public SPC IPL upload
//! protocol. This compact byte program is retained for flat-memory SPC700 corpus
//! tests and for preserving the IPL ROM overlay shape at `$FFC0-$FFFF`; it uses
//! a deliberately small upload protocol instead of trying to reproduce the
//! manufacturer ROM bytes.
//!
//! ## Compact test protocol
//!
//! 1. **Ready**: APU presents `$AA`/`$BB` on ports 0/1. Host polls until it
//!    sees the signature, then writes `$CC` to port 0 to begin transfer.
//! 2. **Address latch**: APU reads the 16-bit load address from ports 2 (lo)
//!    and 3 (hi), stores it internally, and acknowledges by presenting `$CC`
//!    on port 0.
//! 3. **Transfer loop**: host writes a data byte to port 1, then writes a
//!    monotonically increasing 1-based index byte to port 0. APU stores the
//!    data byte to the current pointer, increments the pointer, and echoes the
//!    index on port 0 so the host knows to advance.
//! 4. **Kick**: host writes strobe **0** to port 0 (port 1 value is ignored).
//!    APU jumps to the load address.
//!
//! ## Kick detection
//!
//! Kick is detected when the host writes strobe 0 to port 0.  Normal transfer
//! strobes are 1-based and monotonically increasing, so strobe 0 is unambiguously
//! out-of-band.  This approach allows the host to send arbitrary byte values
//! (including $00) as data without triggering a premature kick.
//!
//! ## Clean-room boundary
//!
//! These bytes were assembled by hand from the documented protocol above using
//! only the public SPC700 instruction set reference. No manufacturer ROM bytes
//! nor any third-party re-implementation were consulted.
//!
//! ## Assembly listing (base = $FFC0)
//!
//! ```text
//! ; DP scratch:
//! ;   $00       = last_strobe (saved so xfer_top can detect new index and
//! ;               ack can echo it on port 0)
//! ;   $02/$03   = write pointer (ptr_lo / ptr_hi) — incremented each byte
//! ;   $04/$05   = original load address (lo/hi) — saved once at latch time;
//! ;               used by do_kick so the jump goes to load address, not to
//! ;               wherever the pointer currently sits after uploading.
//! ;
//! ; X is zeroed (MOV X,#$00) before the kick check, so it is always 0 when
//! ; we reach do_kick (JMP [!$0004+0] = orig load addr), and also 0 when
//! ; we reach the C7 indirect store (correct: DP[$02+0] = [$02:$03] = ptr).
//! ;
//! ; Kick detection: strobe 0 is the kick signal.  Transfer strobes are
//! ; 1-based and strictly positive, so strobe 0 is unambiguously out-of-band.
//! ; This lets the host send $00 data bytes without triggering an early kick.
//! ;
//! ; After saving the new strobe and zeroing X, we use CMP X, dp[$00] to
//! ; test if the saved strobe is 0 (kick) vs. non-zero (normal transfer).
//! ; CMP X ($00) compares X=0 against DP[$00]=new_strobe; Z=1 iff new_strobe=0.
//! ; This preserves X=0 for both the C7 store and the do_kick JMP path.
//! ;
//! ; Pointer wraparound (ptr_lo $FF→$00) is intentionally not handled; programs
//! ; up to 256 bytes fit without wraparound.
//!
//! FFC0: E8 AA     MOV  A, #$AA        ; A = ready byte 0
//! FFC2: C4 F4     MOV  $F4, A         ; port 0 = $AA
//! FFC4: E8 BB     MOV  A, #$BB        ; A = ready byte 1
//! FFC6: C4 F5     MOV  $F5, A         ; port 1 = $BB
//!
//! ;--- poll until host writes $CC to port 0 ---
//! FFC8: E4 F4     MOV  A, $F4         ; poll_cc: A = port 0
//! FFCA: 68 CC     CMP  A, #$CC        ; is it $CC?
//! FFCC: D0 FA     BNE  poll_cc        ; no → loop (−6)
//!
//! ;--- latch load address from ports 2/3; save original for do_kick ---
//! FFCE: E4 F6     MOV  A, $F6         ; A = addr_lo (port 2)
//! FFD0: C4 02     MOV  $02, A         ; ptr_lo = addr_lo
//! FFD2: C4 04     MOV  $04, A         ; orig_lo = addr_lo  (saved for kick)
//! FFD4: E4 F7     MOV  A, $F7         ; A = addr_hi (port 3)
//! FFD6: C4 03     MOV  $03, A         ; ptr_hi = addr_hi
//! FFD8: C4 05     MOV  $05, A         ; orig_hi = addr_hi  (saved for kick)
//!
//! ;--- acknowledge with $CC on port 0; save as last_strobe ---
//! FFDA: E8 CC     MOV  A, #$CC
//! FFDC: C4 F4     MOV  $F4, A         ; port 0 = $CC  (ACK)
//! FFDE: C4 00     MOV  $00, A         ; DP[$00] = $CC (init last_strobe)
//!
//! ;--- transfer loop ---
//! FFE0: E4 F4     MOV  A, $F4         ; xfer_top: A = port 0
//! FFE2: 64 00     CMP  A, $00         ; A == last_strobe?  (CMP A, dp)
//! FFE4: F0 FA     BEQ  xfer_top       ; equal → wait (−6; from FFE6 → FFE0)
//! FFE6: C4 00     MOV  $00, A         ; save new strobe to DP[$00]
//! FFE8: CD 00     MOV  X, #$00        ; zero X (for C7 store and for JMP kick path)
//! FFEA: 3E 00     CMP  X, $00         ; CMP X(=0), DP[$00](=new_strobe): Z=1 iff new_strobe=0
//! FFEC: F0 0C     BEQ  do_kick        ; new_strobe == 0 → kick (+12 → $FFFA)
//! FFEE: E4 F5     MOV  A, $F5         ; A = port 1 (data byte)
//! FFF0: C7 02     MOV  ($02+X), A     ; store data at ptr (X=0 → correct)
//! FFF2: AB 02     INC  $02            ; ptr_lo++
//! FFF4: E4 00     MOV  A, $00         ; ack: A = saved strobe (from DP[$00])
//! FFF6: C4 F4     MOV  $F4, A         ; echo strobe on port 0
//! FFF8: 2F E6     BRA  xfer_top       ; → $FFE0 (−26; from FFFA → FFE0)
//!
//! ;--- kick: jump to original load address via [$04:$05] ---
//! FFFA: 1F 04 00  JMP  [!$0004+X]     ; do_kick: X=0 → jump through DP[$04:$05] = orig addr
//! FFFD: 00        pad
//! FFFE: C0 FF     reset vector → $FFC0
//! ```
//!
//! Offset of `do_kick` from `$FFEC+2` = `$FFFA − $FFEE` = +12 = 0x0C. ✓
//! Offset of `xfer_top` from `$FFF8+2` = `$FFE0 − $FFFA` = −26 = 0xE6. ✓
//! Offset of `poll_cc` from `$FFCC+2` = `$FFC8 − $FFCE` = −6 = 0xFA. ✓
//! Offset of `xfer_top` from `$FFE4+2` = `$FFE0 − $FFE6` = −6 = 0xFA. ✓

/// The 64-byte IPL boot program mapped at $FFC0–$FFFF.
pub const IPL_ROM: [u8; 64] = [
    // $FFC0 +00: MOV A, #$AA
    0xE8, 0xAA, // $FFC2 +02: MOV $F4, A  (port 0 = $AA)
    0xC4, 0xF4, // $FFC4 +04: MOV A, #$BB
    0xE8, 0xBB, // $FFC6 +06: MOV $F5, A  (port 1 = $BB)
    0xC4, 0xF5, // $FFC8 +08: poll_cc: MOV A, $F4
    0xE4, 0xF4, // $FFCA +0A: CMP A, #$CC
    0x68, 0xCC, // $FFCC +0C: BNE poll_cc  (offset = -6 = 0xFA)
    0xD0, 0xFA, // $FFCE +0E: MOV A, $F6  (addr_lo from port 2)
    0xE4, 0xF6, // $FFD0 +10: MOV $02, A  (ptr_lo = addr_lo)
    0xC4, 0x02, // $FFD2 +12: MOV $04, A  (orig_lo = addr_lo; saved for do_kick jump)
    0xC4, 0x04, // $FFD4 +14: MOV A, $F7  (addr_hi from port 3)
    0xE4, 0xF7, // $FFD6 +16: MOV $03, A  (ptr_hi = addr_hi)
    0xC4, 0x03, // $FFD8 +18: MOV $05, A  (orig_hi = addr_hi; saved for do_kick jump)
    0xC4, 0x05, // $FFDA +1A: MOV A, #$CC
    0xE8, 0xCC, // $FFDC +1C: MOV $F4, A  (ACK: port 0 = $CC)
    0xC4, 0xF4,
    // $FFDE +1E: MOV $00, A  (DP[$00] = $CC; init last_strobe for xfer_top compare)
    0xC4, 0x00, // $FFE0 +20: xfer_top: MOV A, $F4
    0xE4, 0xF4,
    // $FFE2 +22: CMP A, $00  (CMP A, dp; compare port 0 read with last_strobe)
    0x64, 0x00, // $FFE4 +24: BEQ xfer_top  (offset = -6 = 0xFA; target $FFE0)
    0xF0, 0xFA, // $FFE6 +26: MOV $00, A  (save new strobe to DP[$00])
    0xC4, 0x00, // $FFE8 +28: MOV X, #$00  (zero X for C7 store path and JMP kick path)
    0xCD, 0x00,
    // $FFEA +2A: CMP X, $00  (compare X=0 with DP[$00]=new_strobe; Z=1 iff new_strobe=0)
    0x3E, 0x00,
    // $FFEC +2C: BEQ do_kick  (new_strobe == 0 → kick; offset = +12 = 0x0C; target $FFFA)
    0xF0, 0x0C, // $FFEE +2E: MOV A, $F5  (A = port 1 data)
    0xE4, 0xF5, // $FFF0 +30: MOV ($02+X), A  (X=0 → store data at ptr in DP[$02:$03])
    0xC7, 0x02, // $FFF2 +32: INC $02  (ptr_lo++)
    0xAB, 0x02, // $FFF4 +34: ack: MOV A, $00  (A = saved strobe from DP[$00])
    0xE4, 0x00, // $FFF6 +36: MOV $F4, A  (echo strobe on port 0)
    0xC4, 0xF4, // $FFF8 +38: BRA xfer_top  (offset = -26 = 0xE6; target $FFE0)
    0x2F, 0xE6,
    // $FFFA +3A: do_kick: JMP [!$0004+X]  (X=0 → jump through DP[$04:$05] = orig addr)
    // JMP [!abs+X] opcode = 0x1F, followed by 16-bit absolute address (LE)
    0x1F, 0x04, 0x00, // $FFFD +3D: pad
    0x00, // $FFFE +3E: reset vector = $FFC0 (lo=$C0, hi=$FF)
    0xC0, 0xFF,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipl_rom_size() {
        assert_eq!(IPL_ROM.len(), 64, "IPL ROM must be exactly 64 bytes");
    }

    #[test]
    fn reset_vector_at_fffe() {
        // Bytes at offset 62–63 (addresses $FFFE/$FFFF) form the reset vector.
        // Should point to $FFC0: lo=$C0, hi=$FF.
        assert_eq!(IPL_ROM[62], 0xC0, "reset vector lo should be $C0 (→ $FFC0)");
        assert_eq!(IPL_ROM[63], 0xFF, "reset vector hi should be $FF (→ $FFC0)");
    }

    #[test]
    fn ready_signature_setup() {
        // Opcode at offset 0: MOV A, #imm = 0xE8, imm = $AA.
        assert_eq!(IPL_ROM[0], 0xE8, "first byte: MOV A, #imm");
        assert_eq!(IPL_ROM[1], 0xAA, "immediate operand: $AA");
    }
}
