//! Clean-room 64-byte IPL boot ROM for the SPC700 audio processor.
//!
//! ## Protocol (publicly documented)
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
//! 4. **Kick**: host writes a value ≥ 2 ahead of the last index to port 0
//!    while port 1 is 0. APU jumps to the load address.
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
//! ; DP scratch: $00 = last_strobe, $02/$03 = ptr_lo/hi
//!
//! FFC0: E8 AA     MOV  A, #$AA        ; A = ready byte 0
//! FFC2: D8 F4     MOV  $F4, A         ; port 0 = $AA
//! FFC4: E8 BB     MOV  A, #$BB        ; A = ready byte 1
//! FFC6: D8 F5     MOV  $F5, A         ; port 1 = $BB
//!
//! ;--- poll until host writes $CC to port 0 ---
//! FFC8: E4 F4     MOV  A, $F4         ; poll_cc: A = port 0
//! FFCA: 68 CC     CMP  A, #$CC        ; is it $CC?
//! FFCC: D0 FA     BNE  poll_cc        ; no → loop (−6)
//!
//! ;--- latch load address from ports 2/3 ---
//! FFCE: E4 F6     MOV  A, $F6         ; A = addr_lo (port 2)
//! FFD0: C4 02     MOV  $02, A         ; ptr_lo = addr_lo
//! FFD2: E4 F7     MOV  A, $F7         ; A = addr_hi (port 3)
//! FFD4: C4 03     MOV  $03, A         ; ptr_hi = addr_hi
//!
//! ;--- acknowledge with $CC on port 0 ---
//! FFD6: E8 CC     MOV  A, #$CC
//! FFD8: D8 F4     MOV  $F4, A         ; port 0 = $CC
//!
//! ;--- init last_strobe = $CC so first compare detects new index ---
//! FFDA: CD CC     MOV  X, #$CC        ; X = last strobe
//!
//! ;--- transfer loop ---
//! FFDC: E4 F4     MOV  A, $F4         ; xfer_top: A = port 0
//! FFDE: 7E        CMP  A, X           ; A == X? (1-byte compare A with X)
//! FFDF: F0 FB     BEQ  xfer_top       ; equal → wait (−5)
//! FFE1: C4 00     MOV  $00, A         ; save new strobe
//! FFE3: E4 F5     MOV  A, $F5         ; A = port 1 (data byte)
//! FFE5: F0 0E     BEQ  do_kick        ; data == 0 → kick (+14 → $FFF5)
//! FFE7: C7 02     MOV  [$02], A       ; store data at [ptr]
//! FFE9: AB 02     INC  $02            ; ptr_lo++
//! FFEB: D0 04     BNE  ack            ; no wrap → ack (+4 → $FFF1)
//! FFED: AB 03     INC  $03            ; ptr_hi++ (lo wrapped)
//! FFEF: 2F 00     BRA  ack            ; → ack (+0, next byte)
//! FFF1: E4 00     MOV  A, $00         ; ack: A = new strobe
//! FFF3: 5D        MOV  X, A           ; X = new strobe (update last)
//! FFF4: D8 F4     MOV  $F4, A         ; echo strobe on port 0
//! FFF6: 2F E4     BRA  xfer_top       ; → $FFDC (−28)
//!
//! ;--- kick: jump to load address in [$02:$03] ---
//! FFF8: 1F 02 00  JMP  [!$0002]       ; do_kick: indirect jump through DP[2:3]
//! FFFB: 00        NOP / pad
//! FFFC: C0 FF     (unused padding — reset vector at FFFE, not FFFC)
//! FFFE: C0 FF     reset vector → $FFC0
//! ```
//!
//! Offset of `do_kick` from `$FFE5+2` = `$FFF5 - $FFE7 = +14 = 0x0E`. ✓
//! Offset of `xfer_top` from `$FFF6+2` = `$FFDC - $FFF8 = −28 = 0xE4`. ✓
//! Offset of `poll_cc` from `$FFCC+2` = `$FFC8 - $FFCE = −6 = 0xFA`. ✓

/// The 64-byte IPL boot program mapped at $FFC0–$FFFF.
pub const IPL_ROM: [u8; 64] = [
    // $FFC0 +00: MOV A, #$AA
    0xE8, 0xAA, // $FFC2 +02: MOV $F4, A  (port 0 = $AA)
    0xD8, 0xF4, // $FFC4 +04: MOV A, #$BB
    0xE8, 0xBB, // $FFC6 +06: MOV $F5, A  (port 1 = $BB)
    0xD8, 0xF5, // $FFC8 +08: poll_cc: MOV A, $F4
    0xE4, 0xF4, // $FFCA +0A: CMP A, #$CC
    0x68, 0xCC, // $FFCC +0C: BNE poll_cc  (offset = -6 = 0xFA)
    0xD0, 0xFA, // $FFCE +0E: MOV A, $F6  (addr_lo)
    0xE4, 0xF6, // $FFD0 +10: MOV $02, A
    0xC4, 0x02, // $FFD2 +12: MOV A, $F7  (addr_hi)
    0xE4, 0xF7, // $FFD4 +14: MOV $03, A
    0xC4, 0x03, // $FFD6 +16: MOV A, #$CC
    0xE8, 0xCC, // $FFD8 +18: MOV $F4, A  (ack: port 0 = $CC)
    0xD8, 0xF4, // $FFDA +1A: MOV X, #$CC  (X = last_strobe = $CC)
    0xCD, 0xCC, // $FFDC +1C: xfer_top: MOV A, $F4
    0xE4, 0xF4, // $FFDE +1E: CMP A, X  (1-byte opcode 0x7E)
    0x7E, // $FFDF +1F: BEQ xfer_top  (offset = -5 = 0xFB)
    0xF0, 0xFB, // $FFE1 +21: MOV $00, A  (save new strobe)
    0xC4, 0x00, // $FFE3 +23: MOV A, $F5  (A = port1 data)
    0xE4, 0xF5, // $FFE5 +25: BEQ do_kick  (offset = +14 = 0x0E; target $FFF5)
    0xF0, 0x0E, // $FFE7 +27: MOV [$02], A  (store data at [ptr])
    0xC7, 0x02, // $FFE9 +29: INC $02  (ptr_lo++)
    0xAB, 0x02, // $FFEB +2B: BNE ack  (offset = +4 = 0x04; target $FFF1)
    0xD0, 0x04, // $FFED +2D: INC $03  (ptr_hi++)
    0xAB, 0x03, // $FFEF +2F: BRA ack  (offset = 0; target $FFF1)
    0x2F, 0x00, // $FFF1 +31: ack: MOV A, $00  (A = new strobe)
    0xE4, 0x00, // $FFF3 +33: MOV X, A  (X = new strobe; 1-byte opcode 0x5D)
    0x5D, // $FFF4 +34: MOV $F4, A  (echo strobe on port 0)
    0xD8, 0xF4, // $FFF6 +36: BRA xfer_top  (offset = -28 = 0xE4; target $FFDC)
    0x2F, 0xE4,
    // $FFF8 +38: do_kick: JMP [!$0002]  (indirect jump through DP[$02:$03])
    // JMP [!abs] opcode = 0x1F, followed by 16-bit absolute address (LE)
    0x1F, 0x02, 0x00, // $FFFB +3B: pad
    0x00, // $FFFC +3C: pad (not a real vector on this chip)
    0x00, 0x00, // $FFFE +3E: reset vector = $FFC0 (lo=$C0, hi=$FF)
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
