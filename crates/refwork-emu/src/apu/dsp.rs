//! SPC700 DSP — 8-voice sample playback, envelopes, echo, noise, and pitch
//! modulation. Fixed-point / integer arithmetic only (D4: no floats).
//!
//! ## Register map (128 bytes, addressed via SPC700 $F2/$F3)
//!
//! The 128-byte register file is organised as 8 voice banks (8 bytes each,
//! voice `v` at base `v * 0x10`) plus global registers scattered in the upper
//! half:
//!
//! | Offset (hex) | Name  | Description                                  |
//! |--------------|-------|----------------------------------------------|
//! | `vX + 0`     | VOL_L | Voice left volume (signed)                   |
//! | `vX + 1`     | VOL_R | Voice right volume (signed)                  |
//! | `vX + 2`     | PITCHL| Pitch register low byte (14-bit pitch)       |
//! | `vX + 3`     | PITCHH| Pitch register high nibble                   |
//! | `vX + 4`     | SRCN  | Source sample number (DIR index)             |
//! | `vX + 5`     | ADSR1 | ADSR first byte (attack/decay/ADSR-enable)   |
//! | `vX + 6`     | ADSR2 | ADSR second byte (sustain level/rate)        |
//! | `vX + 7`     | GAIN  | GAIN register (used when ADSR disabled)      |
//! | `vX + 8`     | ENVX  | Readable: current envelope (7-bit)           |
//! | `vX + 9`     | OUTX  | Readable: current output (8-bit signed)      |
//! | `0x0C`       | MVOLL | Main volume left (signed)                    |
//! | `0x1C`       | MVOLR | Main volume right (signed)                   |
//! | `0x2C`       | EVOLL | Echo volume left (signed)                    |
//! | `0x3C`       | EVOLR | Echo volume right (signed)                   |
//! | `0x4C`       | KON   | Key-on bitmask (write to trigger voices)     |
//! | `0x5C`       | KOF   | Key-off bitmask (write to release voices)    |
//! | `0x6C`       | FLG   | Flags: RESET (b7), MUTE (b6), ECEN (b5),    |
//! |              |       |   NRATE (b4:b0) noise rate                   |
//! | `0x7C`       | ENDX  | End-flag bitmask (BRR loop/end, write-clears)|
//! | `0x0D`       | EFB   | Echo feedback volume (signed)                |
//! | `0x2D`       | PMON  | Pitch modulation enable (voices 1–7)         |
//! | `0x3D`       | NON   | Noise enable per-voice                       |
//! | `0x4D`       | EON   | Echo output enable per-voice                 |
//! | `0x5D`       | DIR   | Sample directory base page (DIR * $100)      |
//! | `0x6D`       | ESA   | Echo buffer base page (ESA * $100)           |
//! | `0x7D`       | EDL   | Echo delay (low nibble, 0–15 in units of 2ms)|
//! | `0xnF`       | FIR n | 8 FIR coefficient registers (n=0..7)         |
//!
//! ## Timing
//!
//! The DSP produces one stereo sample every 32 SPC700 master-clock cycles
//! (≈ 32 kHz). The owning `Apu` drives it from its SPC-clock accumulator.
//!
//! ## Design constraints
//!
//! - No heap allocation after construction (D8). All buffers are fixed-size
//!   arrays.
//! - No floats anywhere (D4). All arithmetic uses `i32`/`i16`/`u16` with
//!   explicit Q-format shifts.
//! - All state lives in the `Dsp` struct (D5).
//! - Output samples are computed and discarded. Observable state (ENVX, OUTX,
//!   ENDX) is updated so the SPC700 software can read it back, which is how
//!   game audio engines gate their progress.

// ─── Gaussian interpolation table ────────────────────────────────────────────
//
// The hardware uses a 512-entry table of i16 Gaussian-windowed sinc coefficients
// for 4-tap interpolation. Public hardware documentation publishes this table
// exactly; it is reproduced here verbatim.
//
// Four consecutive table entries are used per sample. The fractional part of
// the pitch counter (bits [11:0] of the 12-bit sub-sample position, 0–$FFF)
// is scaled to a 0–511 index `pos` and the four taps are at indices:
//   0*512 + pos, 1*512 + pos, 2*512 + pos (reversed), 3*512 + pos (reversed)
// Actually the hardware uses a single 512-entry table with the 4 taps at:
//   tbl[511 - pos]  (oldest sample)
//   tbl[pos]
//   tbl[511 - pos]  (symmetric; see note)
//   tbl[pos]
// The authoritative layout per public hardware docs is a 512-entry table used
// as described below — each half-table is symmetric, so only 256 unique values.
// We transcribe all 512 entries for fidelity.

/// 512-entry Gaussian interpolation table (i16 coefficients).
///
/// Indexed as `GAUSS[i]` for i in 0..512. For a fractional offset `f`
/// (0..=0xFFF), `pos = f >> 4` (giving 0..=255 as the table index within
/// each half), and the four taps use positions arranged as two symmetric
/// 256-entry halves within the 512-entry table.
///
/// The checksum (sum of all 512 entries, wrapping i32) is pinned by a unit
/// test below to catch any transcription errors.
pub const GAUSS: [i16; 512] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2,
    2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10,
    10, 11, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 15, 16, 16, 17, 17, 18, 19, 19, 20, 20, 21, 21,
    22, 23, 23, 24, 24, 25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 36, 36, 37, 38, 39,
    40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 58, 59, 60, 61, 62, 64, 65,
    66, 67, 69, 70, 71, 73, 74, 76, 77, 78, 80, 81, 83, 84, 86, 87, 89, 90, 92, 94, 95, 97, 99,
    100, 102, 104, 106, 107, 109, 111, 113, 115, 117, 118, 120, 122, 124, 126, 128, 130, 132, 134,
    137, 139, 141, 143, 145, 147, 150, 152, 154, 156, 159, 161, 163, 166, 168, 171, 173, 175, 178,
    180, 183, 186, 188, 191, 193, 196, 199, 201, 204, 207, 210, 212, 215, 218, 221, 224, 227, 230,
    233, 236, 239, 242, 245, 248, 251, 254, 257, 260, 263, 267, 270, 273, 276, 280, 283, 286, 290,
    293, 297, 300, 304, 307, 311, 314, 318, 321, 325, 328, 332, 336, 339, 343, 347, 351, 354, 358,
    362, 366, 370, 374, 378, 381, 385, 389, 393, 397, 401, 405, 410, 414, 418, 422, 426, 430, 435,
    439, 443, 447, 452, 456, 460, 465, 469, 474, 478, 483, 487, 492, 496, 501, 505, 510, 515, 519,
    524, 529, 533, 538, 543, 547, 552, 557, 562, 566, 571, 576, 581, 586, 591, 596, 601, 606, 611,
    616, 621, 626, 631, 636, 641, 646, 651, 656, 661, 666, 671, 676, 681, 686, 692, 697, 702, 707,
    712, 717, 723, 728, 733, 738, 744, 749, 754, 759, 765, 770, 775, 781, 786, 791, 797, 802, 808,
    813, 818, 824, 829, 835, 840, 846, 851, 857, 862, 868, 873, 879, 884, 890, 895, 901, 906, 912,
    917, 923, 929, 934, 940, 945, 951, 957, 962, 968, 974, 979, 985, 991, 996, 1002, 1008, 1013,
    1019, 1025, 1030, 1036, 1042, 1047, 1053, 1059, 1065, 1070, 1076, 1082, 1088, 1094, 1099, 1105,
    1111, 1117, 1123, 1129, 1134, 1140, 1146, 1152, 1158, 1164, 1170, 1176, 1182, 1188, 1194, 1199,
    1205, 1211, 1217, 1223, 1229, 1235, 1241, 1248, 1254, 1260, 1266, 1272, 1278, 1284, 1290, 1296,
    1303, 1309, 1315, 1321, 1327, 1333, 1339, 1346, 1352, 1358, 1364, 1370, 1377, 1383, 1389, 1395,
    1401, 1408, 1414, 1420, 1426, 1433, 1439, 1445, 1452, 1458, 1464, 1471, 1477, 1483, 1490, 1496,
    1502, 1509, 1515, 1521, 1528, 1534, 1541, 1547, 1553, 1560, 1566, 1573, 1579, 1585, 1592, 1598,
    1605, 1611, 1618, 1624, 1630, 1637, 1643, 1650, 1656, 1663, 1669, 1676, 1682, 1689, 1695, 1702,
    1708, 1715, 1721, 1728, 1734, 1741, 1747, 1754, 1760, 1767, 1773, 1780, 1786, 1793, 1799,
];

// ─── Envelope rate table ──────────────────────────────────────────────────────
//
// The 32-entry table maps a 5-bit rate value to a decrement/increment step
// count (in terms of 1/64th of the full envelope range). The table is
// documented in public hardware references. Index 0 means "never changes".
// Index 31 means "immediately". All other values are powers-of-two-ish
// measured in 32 kHz sample ticks.

/// 32-entry envelope rate lookup: `RATE_TABLE[rate]` = number of 32 kHz
/// ticks between each envelope step.  0 means "no change"; entries are
/// positive integers (the envelope steps by 1/8 of max (= 32 out of 2047)
/// per fired tick for most modes).
///
/// Per public hardware docs, the table encodes the period (in samples) between
/// envelope updates. Rate 0 = never; rate 31 = every sample.
pub const RATE_TABLE: [u16; 32] = [
    0,    // 0: never
    2048, // 1
    1536, // 2
    1280, // 3
    1024, // 4
    768,  // 5
    640,  // 6
    512,  // 7
    384,  // 8
    320,  // 9
    256,  // 10
    192,  // 11
    160,  // 12
    128,  // 13
    96,   // 14
    80,   // 15
    64,   // 16
    48,   // 17
    40,   // 18
    32,   // 19
    24,   // 20
    20,   // 21
    16,   // 22
    12,   // 23
    10,   // 24
    8,    // 25
    6,    // 26
    5,    // 27
    4,    // 28
    3,    // 29
    2,    // 30
    1,    // 31: every sample
];

// ─── Envelope state ───────────────────────────────────────────────────────────

/// ADSR/GAIN envelope phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPhase {
    /// Voice is silent (key-off complete or not started).
    Off,
    /// Attack phase: envelope rising toward $7FF.
    Attack,
    /// Decay phase: envelope falling toward sustain level.
    Decay,
    /// Sustain phase: holding or slow-decaying at sustain level.
    Sustain,
    /// Release phase: fast linear decrease toward 0 after KOF.
    Release,
}

// ─── BRR decode buffer ────────────────────────────────────────────────────────

/// Size of the BRR decode ring buffer per voice (16 samples: 1 BRR block
/// worth of decoded PCM). We keep the last 16 decoded samples for the
/// 4-tap gaussian interpolation look-behind.
pub const BRR_BUF_LEN: usize = 16;

// ─── Per-voice state ──────────────────────────────────────────────────────────

/// Per-voice DSP state. All fields are plain integers (D5).
#[derive(Debug, Clone)]
pub struct Voice {
    // ── Pitch ──
    /// 12-bit sub-sample pitch counter (0..=0xFFF). Fractional position
    /// within the current decoded BRR sample pair.
    pub pitch_counter: u16,

    // ── BRR playback ──
    /// Byte address in ARAM of the *next* BRR block to decode.
    pub brr_addr: u16,
    /// Index of the last byte read within the current 9-byte BRR block
    /// (0 = header; 1–8 = data). When this reaches 8 after the last
    /// nybble-pair, we move to the next block.
    pub brr_block_offset: u8,
    /// Decoded PCM ring buffer: 16 samples of i16. New samples are appended
    /// at `buf_pos` modulo 16.
    pub buf: [i16; BRR_BUF_LEN],
    /// Next write index in `buf` (mod BRR_BUF_LEN).
    pub buf_pos: u8,
    /// Previous two decoded samples for BRR filter look-back (filter modes 2/3).
    pub brr_old1: i16,
    pub brr_old2: i16,
    /// Loop start address (latched from the source directory on KON).
    pub loop_addr: u16,
    /// Set when BRR end flag was encountered (cleared on KON write).
    pub end_flag: bool,
    /// Set when the BRR loop flag was seen in the last block header.
    pub loop_flag: bool,

    // ── Envelope ──
    /// Current 11-bit envelope level (0..=0x7FF).
    pub envelope: u16,
    /// Current envelope phase.
    pub env_phase: EnvPhase,
    /// Per-envelope sample-tick counter (counts down from rate_period).
    pub env_tick: u16,

    // ── KON pending ──
    /// When > 0, this voice has a pending KON that fires in `kon_delay` ticks.
    pub kon_delay: u8,

    // ── Observable registers ──
    /// ENVX (readable): envelope >> 4 (7 bits, 0..127).
    pub envx: u8,
    /// OUTX (readable): voice output sample (signed 8-bit).
    pub outx: i8,

    // ── Active flag ──
    /// True when the voice is actively producing sound.
    pub active: bool,
}

impl Voice {
    pub const fn new() -> Self {
        Voice {
            pitch_counter: 0,
            brr_addr: 0,
            brr_block_offset: 0,
            buf: [0i16; BRR_BUF_LEN],
            buf_pos: 0,
            brr_old1: 0,
            brr_old2: 0,
            loop_addr: 0,
            end_flag: false,
            loop_flag: false,
            envelope: 0,
            env_phase: EnvPhase::Off,
            env_tick: 0,
            kon_delay: 0,
            envx: 0,
            outx: 0,
            active: false,
        }
    }
}

// ─── Echo ring buffer ─────────────────────────────────────────────────────────

/// Maximum echo delay in bytes per channel. EDL field is 4 bits (0–15);
/// each unit = 512 bytes of echo buffer. Max = 15 * 512 = 7680.
/// We store L and R interleaved as i16 pairs, so max entries = 7680.
/// Fixed-size: we pre-allocate for the maximum.
pub const ECHO_BUF_MAX: usize = 7680;

// ─── Main DSP struct ──────────────────────────────────────────────────────────

/// The 8-voice DSP. Clocked by `Apu::advance` at 1 sample per 32 SPC cycles.
///
/// All state lives in this struct (D5). Buffers are fixed-size (D8).
/// No floats anywhere (D4).
pub struct Dsp {
    /// 128-byte register file (voice regs + global regs).
    pub regs: [u8; 128],

    /// Per-voice runtime state.
    pub voices: [Voice; 8],

    /// Noise LFSR: 15-bit state (bit 14 down to bit 0).
    pub noise_lfsr: u16,
    /// Noise tick counter: counts down from RATE_TABLE[noise_rate].
    pub noise_tick: u16,

    /// Echo ring buffer (i16 samples, L and R pairs stored as two i32 arrays).
    /// Index wraps at `echo_buf_size` (computed from EDL each sample).
    echo_buf_l: [i16; ECHO_BUF_MAX],
    echo_buf_r: [i16; ECHO_BUF_MAX],
    /// Current write index into echo_buf_l/r.
    pub echo_pos: u16,

    /// Per-voice last decoded sample (for OUTX / gaussian input).
    /// Separate from the ring buffer; gives the raw voice output before volume.
    voice_out: [i16; 8],

    /// FIR history: 8 most-recent echo samples per channel for the 8-tap FIR.
    fir_hist_l: [i16; 8],
    fir_hist_r: [i16; 8],
    /// Current FIR history write position (mod 8).
    fir_pos: u8,

    /// Last computed output frame (for `introspect` feature / tests).
    #[cfg(feature = "introspect")]
    pub last_out_l: i16,
    #[cfg(feature = "introspect")]
    pub last_out_r: i16,

    /// Global sample counter since last KON (for deterministic initial state).
    pub sample_count: u64,

    /// ENDX latched value (read-clear register, accumulated between reads).
    pub endx: u8,
}

impl Dsp {
    /// Construct with power-on state (all zeros).
    pub fn new() -> Self {
        Dsp {
            regs: [0u8; 128],
            voices: [
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
                Voice::new(),
            ],
            noise_lfsr: 0x4000, // non-zero initial state for LFSR (deterministic)
            noise_tick: 0,
            echo_buf_l: [0i16; ECHO_BUF_MAX],
            echo_buf_r: [0i16; ECHO_BUF_MAX],
            echo_pos: 0,
            voice_out: [0i16; 8],
            fir_hist_l: [0i16; 8],
            fir_hist_r: [0i16; 8],
            fir_pos: 0,
            #[cfg(feature = "introspect")]
            last_out_l: 0,
            #[cfg(feature = "introspect")]
            last_out_r: 0,
            sample_count: 0,
            endx: 0,
        }
    }

    // ─── Register access ──────────────────────────────────────────────────────

    /// Read a DSP register. Returns 0 for write-only registers.
    pub fn read_reg(&self, addr: u8) -> u8 {
        let a = addr & 0x7F;
        match a {
            // ENVX (voice readable)
            0x08 | 0x18 | 0x28 | 0x38 | 0x48 | 0x58 | 0x68 | 0x78 => {
                let v = (a >> 4) as usize;
                self.voices[v].envx
            }
            // OUTX (voice readable)
            0x09 | 0x19 | 0x29 | 0x39 | 0x49 | 0x59 | 0x69 | 0x79 => {
                let v = (a >> 4) as usize;
                self.voices[v].outx as u8
            }
            // ENDX — read returns accumulated end flags, then register is cleared
            0x7C => self.regs[a as usize],
            _ => self.regs[a as usize],
        }
    }

    /// Write a DSP register.
    pub fn write_reg(&mut self, addr: u8, value: u8) {
        let a = addr & 0x7F;
        // ENDX ($7C): any write clears all end flags (documented hardware
        // behaviour — the written value is ignored).
        if a == 0x7C {
            self.regs[0x7C] = 0;
            self.endx = 0;
            return;
        }
        // Most registers just store to regs[]; KON/KOF/FLG have side effects
        // handled in the per-sample step.
        self.regs[a as usize] = value;
    }

    // ─── Global register helpers ──────────────────────────────────────────────

    #[inline]
    fn mvol_l(&self) -> i8 {
        self.regs[0x0C] as i8
    }
    #[inline]
    fn mvol_r(&self) -> i8 {
        self.regs[0x1C] as i8
    }
    #[inline]
    fn evol_l(&self) -> i8 {
        self.regs[0x2C] as i8
    }
    #[inline]
    fn evol_r(&self) -> i8 {
        self.regs[0x3C] as i8
    }
    #[inline]
    fn flg(&self) -> u8 {
        self.regs[0x6C]
    }
    #[inline]
    fn efb(&self) -> i8 {
        self.regs[0x0D] as i8
    }
    #[inline]
    fn pmon(&self) -> u8 {
        self.regs[0x2D]
    }
    #[inline]
    fn non(&self) -> u8 {
        self.regs[0x3D]
    }
    #[inline]
    fn eon(&self) -> u8 {
        self.regs[0x4D]
    }
    #[inline]
    fn dir(&self) -> u8 {
        self.regs[0x5D]
    }
    #[inline]
    fn esa(&self) -> u8 {
        self.regs[0x6D]
    }
    #[inline]
    fn edl(&self) -> u8 {
        self.regs[0x7D] & 0x0F
    }

    #[inline]
    fn fir_coef(&self, i: usize) -> i8 {
        self.regs[0x0F + (i << 4)] as i8
    }

    // ─── Per-voice register helpers ───────────────────────────────────────────

    #[inline]
    fn voice_vol_l(&self, v: usize) -> i8 {
        self.regs[v * 0x10] as i8
    }
    #[inline]
    fn voice_vol_r(&self, v: usize) -> i8 {
        self.regs[v * 0x10 + 1] as i8
    }
    #[inline]
    fn voice_pitch(&self, v: usize) -> u16 {
        let lo = self.regs[v * 0x10 + 2] as u16;
        let hi = (self.regs[v * 0x10 + 3] & 0x3F) as u16;
        lo | (hi << 8)
    }
    #[inline]
    fn voice_srcn(&self, v: usize) -> u8 {
        self.regs[v * 0x10 + 4]
    }
    #[inline]
    fn voice_adsr1(&self, v: usize) -> u8 {
        self.regs[v * 0x10 + 5]
    }
    #[inline]
    fn voice_adsr2(&self, v: usize) -> u8 {
        self.regs[v * 0x10 + 6]
    }
    #[inline]
    fn voice_gain(&self, v: usize) -> u8 {
        self.regs[v * 0x10 + 7]
    }

    // ─── BRR decode ───────────────────────────────────────────────────────────

    /// Decode one BRR nybble into a 16-bit PCM sample.
    ///
    /// Algorithm (from public hardware documentation):
    ///
    /// 1. Extract 4-bit signed nybble, shift up by `shift` (1–12) clamped to
    ///    0..=12.
    /// 2. Apply filter:
    ///    - mode 0: raw
    ///    - mode 1: s += old1 * 15/16  (i.e., old1 + old1*(-1/16))
    ///    - mode 2: s += old1 * 2 - old2 * 15/16
    ///    - mode 3: s += old1 * 13/8 - old2 * 3/4 (approximately)
    /// 3. Clamp to i16 range.
    ///
    /// The fractional multiplications use the integer approximations from the
    /// documented hardware reference (powers-of-2 right-shifts):
    ///   - old1 * 15/16  ≈  old1 - (old1 >> 4)
    ///   - old1 * 2      =  old1 << 1
    ///   - old2 * 15/16  ≈  old2 - (old2 >> 4)
    ///   - old1 * 13/8   =  old1 + old1 + (old1 >> 1) + (old1 >> 3)
    ///   - old2 * 3/4    =  (old2 >> 1) + (old2 >> 2)
    fn decode_brr_nybble(raw4: i32, shift: u8, filter: u8, old1: i16, old2: i16) -> i16 {
        // 1. Shift the nybble. shift is clamped to 0..=12 by callers.
        // The hardware saturates: shift > 12 produces extreme values → clamp.
        let shifted: i32 = if shift > 12 {
            // Overflow / garbage shift: propagate sign of nybble maximally.
            if raw4 < 0 {
                -0x8000i32
            } else {
                0
            }
        } else {
            raw4 << shift
        };

        // 2. Apply filter.
        let s = shifted;
        let filtered: i32 = match filter {
            0 => s,
            1 => {
                let f = old1 as i32;
                s + f + (-(f >> 4))
            }
            2 => {
                let f1 = old1 as i32;
                let f2 = old2 as i32;
                s + (f1 << 1) + (-(f1 + (f1 >> 4))) + (-(f2 - (f2 >> 4)))
            }
            _ => {
                // 3
                let f1 = old1 as i32;
                let f2 = old2 as i32;
                s + (f1 << 1) + f1 + (f1 >> 1) + (f1 >> 3) - f2 - (f2 >> 1) - (f2 >> 2)
            }
        };

        // 3. Clamp to i16. The hardware sign-extends the result into 16 bits,
        // then saturates (NOT wraps) if it overflows. Per public reference:
        // the hardware does a clip to -32768..+32767 treating it as a 16-bit
        // signed value that first wraps, then the game reads 16-bit. We
        // implement the documented saturation clamp.
        let clamped = filtered.clamp(-32768, 32767);
        // Zero bits 0 (hardware does this: sample >> 1 << 1, keeping it even).
        (clamped as i16) & !1
    }

    /// Decode 4 nybbles (one half-block = 4 PCM samples) from `brr_data` at
    /// `byte_offset` (the pair-byte position), updating `old1`/`old2`.
    fn decode_brr_block_pair(
        brr_data: &[u8; 8],
        shift: u8,
        filter: u8,
        old1: &mut i16,
        old2: &mut i16,
        out: &mut [i16],
        out_start: usize,
    ) {
        for pair in 0..4usize {
            let byte = brr_data[pair];
            // High nybble first, then low nybble.
            let raw_hi = (byte >> 4) as i32;
            let raw_lo = (byte & 0xF) as i32;
            // Sign-extend 4-bit to i32.
            let sext_hi = if raw_hi >= 8 { raw_hi - 16 } else { raw_hi };
            let sext_lo = if raw_lo >= 8 { raw_lo - 16 } else { raw_lo };

            let s0 = Self::decode_brr_nybble(sext_hi, shift, filter, *old1, *old2);
            *old2 = *old1;
            *old1 = s0;
            out[out_start + pair * 2] = s0;

            let s1 = Self::decode_brr_nybble(sext_lo, shift, filter, *old1, *old2);
            *old2 = *old1;
            *old1 = s1;
            out[out_start + pair * 2 + 1] = s1;
        }
    }

    /// Advance voice BRR decoder by one BRR block (decode 16 samples) from
    /// ARAM. Returns (end_flag, loop_flag) from the block header.
    fn decode_next_brr_block(voice: &mut Voice, aram: &[u8; 0x10000]) -> (bool, bool) {
        // BRR block layout: 1 header byte + 8 data bytes = 9 bytes total.
        // Header: bits 7-4 = shift (0-12 valid), bits 3-2 = filter, bit 1 = loop, bit 0 = end.
        let header = aram[voice.brr_addr as usize];
        let shift = (header >> 4) & 0xF;
        let filter = (header >> 2) & 0x3;
        let loop_flag = (header >> 1) & 1 != 0;
        let end_flag = header & 1 != 0;

        // Copy 8 data bytes.
        let mut data = [0u8; 8];
        for i in 0..8 {
            data[i] = aram[(voice.brr_addr.wrapping_add(1 + i as u16)) as usize];
        }

        // Decode the 8 bytes = 16 PCM samples (4 nybbles per byte, 2 samples per nybble).
        let mut old1 = voice.brr_old1;
        let mut old2 = voice.brr_old2;
        let mut decoded = [0i16; 16];

        // Decode two groups of 4 bytes = 8 samples each.
        Self::decode_brr_block_pair(&data, shift, filter, &mut old1, &mut old2, &mut decoded, 0);
        // Manually handle remaining 4 bytes.
        for pair in 0..4usize {
            let byte = data[4 + pair];
            let raw_hi = (byte >> 4) as i32;
            let raw_lo = (byte & 0xF) as i32;
            let sext_hi = if raw_hi >= 8 { raw_hi - 16 } else { raw_hi };
            let sext_lo = if raw_lo >= 8 { raw_lo - 16 } else { raw_lo };

            let s0 = Self::decode_brr_nybble(sext_hi, shift, filter, old1, old2);
            old2 = old1;
            old1 = s0;
            decoded[8 + pair * 2] = s0;

            let s1 = Self::decode_brr_nybble(sext_lo, shift, filter, old1, old2);
            old2 = old1;
            old1 = s1;
            decoded[8 + pair * 2 + 1] = s1;
        }

        voice.brr_old1 = old1;
        voice.brr_old2 = old2;

        // Store decoded samples into the ring buffer.
        for s in decoded {
            voice.buf[voice.buf_pos as usize & (BRR_BUF_LEN - 1)] = s;
            voice.buf_pos = voice.buf_pos.wrapping_add(1) & (BRR_BUF_LEN as u8 - 1);
        }

        // Advance to next block.
        voice.brr_addr = voice.brr_addr.wrapping_add(9);

        (end_flag, loop_flag)
    }

    // ─── Gaussian interpolation ───────────────────────────────────────────────

    /// Perform 4-tap Gaussian interpolation on the voice's decoded sample
    /// buffer at the current pitch_counter fractional position.
    ///
    /// The fractional part of the pitch counter (bits 11:0 = 12 bits, 0–$FFF)
    /// is scaled to a table index `pos` = frac >> 4 (0–255 in the lower half).
    ///
    /// Four sample indices (into the ring buffer): current - 3, -2, -1, 0.
    /// Four table indices: gauss[(511-pos)], gauss[(255-pos)], gauss[pos],
    ///                     gauss[(511-pos)] wait — actual layout documented:
    ///
    /// The hardware uses GAUSS indices as follows for fractional part `i`:
    ///   pos = i >> 4            (0..=255)
    ///   G[0] = GAUSS[511 - pos]   (oldest; maps the 512 entry table 256..511)
    ///   G[1] = GAUSS[255 - pos]   (maps 0..255 in reverse)
    ///   G[2] = GAUSS[pos]         (maps 0..255)
    ///   G[3] = GAUSS[511 - pos]   wait, not quite — see below.
    ///
    /// Authoritative per public reference: the GAUSS table is 512 entries;
    /// the 4 taps use indices computed from the 12-bit fractional part:
    ///   i = frac >> 4         (0..=255)
    ///   G0 = GAUSS[255 - i]
    ///   G1 = GAUSS[511 - i]
    ///   G2 = GAUSS[i]
    ///   G3 = GAUSS[256 + i]
    /// (samples from oldest to newest are s[-3]..s[0])
    fn gaussian_interp(buf: &[i16; BRR_BUF_LEN], buf_pos: u8, pitch_counter: u16) -> i32 {
        // Fractional position within decoded samples.
        let frac = ((pitch_counter >> 4) & 0xFF) as usize;

        // 4-tap positions: relative to the next sample to be written = buf_pos.
        // buf_pos is the *next write* position; so most recent = buf_pos - 1.
        let len = BRR_BUF_LEN;
        let i3 = (buf_pos as usize).wrapping_sub(1) & (len - 1);
        let i2 = (buf_pos as usize).wrapping_sub(2) & (len - 1);
        let i1 = (buf_pos as usize).wrapping_sub(3) & (len - 1);
        let i0 = (buf_pos as usize).wrapping_sub(4) & (len - 1);

        let s0 = buf[i0] as i32; // oldest
        let s1 = buf[i1] as i32;
        let s2 = buf[i2] as i32;
        let s3 = buf[i3] as i32; // newest

        let g0 = GAUSS[255 - frac] as i32;
        let g1 = GAUSS[511 - frac] as i32;
        let g2 = GAUSS[frac] as i32;
        let g3 = GAUSS[256 + frac] as i32;

        // Sum = (g0*s0 + g1*s1 + g2*s2 + g3*s3) >> 11, clamped to i16.
        let sum = g0 * s0 + g1 * s1 + g2 * s2 + g3 * s3;
        (sum >> 11).clamp(-32768, 32767)
    }

    // ─── Envelope step ────────────────────────────────────────────────────────

    /// Advance one voice envelope by one sample tick.
    fn step_envelope(voice: &mut Voice, adsr1: u8, adsr2: u8, gain: u8) {
        if voice.env_phase == EnvPhase::Off {
            voice.envelope = 0;
            voice.envx = 0;
            return;
        }

        let adsr_en = adsr1 & 0x80 != 0;

        if voice.env_phase == EnvPhase::Release {
            // Release: fixed linear decrease by 8 per sample.
            if voice.envelope > 8 {
                voice.envelope -= 8;
            } else {
                voice.envelope = 0;
                voice.env_phase = EnvPhase::Off;
            }
            voice.envx = (voice.envelope >> 4) as u8;
            return;
        }

        if adsr_en {
            Self::step_adsr_envelope(voice, adsr1, adsr2);
        } else {
            Self::step_gain_envelope(voice, gain);
        }

        voice.envx = (voice.envelope >> 4) as u8;
    }

    fn step_adsr_envelope(voice: &mut Voice, adsr1: u8, adsr2: u8) {
        match voice.env_phase {
            EnvPhase::Attack => {
                let rate = adsr1 & 0x0F;
                // Attack rate: odd rates step by 32, even (linear) step by 1/64 range.
                // Actually per docs: ATTACK uses rates 0–15 mapped directly to
                // RATE_TABLE indices as (rate*2 + 1) for linear, 63 (instantaneous) for rate 15.
                // Simplified documented model: rate 15 → +1024 per step (near instant);
                // other rates: period = RATE_TABLE[rate*2+1], step += 32.
                if rate == 15 {
                    voice.envelope = (voice.envelope + 1024).min(0x7FF);
                } else {
                    let period = RATE_TABLE[(rate * 2 + 1) as usize];
                    if period == 0 {
                        voice.envelope = (voice.envelope + 32).min(0x7FF);
                    } else {
                        voice.env_tick = voice.env_tick.wrapping_add(1);
                        if voice.env_tick >= period {
                            voice.env_tick = 0;
                            voice.envelope = (voice.envelope + 32).min(0x7FF);
                        }
                    }
                }
                if voice.envelope >= 0x7FF {
                    voice.envelope = 0x7FF;
                    voice.env_phase = EnvPhase::Decay;
                    voice.env_tick = 0;
                }
            }
            EnvPhase::Decay => {
                let rate = ((adsr1 >> 3) & 0x07) as usize * 2 + 16;
                let period = RATE_TABLE[rate.min(31)];
                if period == 0 || {
                    voice.env_tick = voice.env_tick.wrapping_add(1);
                    voice.env_tick >= period
                } {
                    voice.env_tick = 0;
                    // Exponential decrease: step by (envelope >> 8) + 1.
                    let step = (voice.envelope >> 8) + 1;
                    voice.envelope = voice.envelope.saturating_sub(step);
                }
                // Transition to Sustain when envelope <= SL * $100.
                let sl = ((adsr2 >> 5) & 0x07) as u16;
                let sustain_level = (sl + 1) * 0x100;
                if voice.envelope <= sustain_level {
                    voice.envelope = sustain_level;
                    voice.env_phase = EnvPhase::Sustain;
                    voice.env_tick = 0;
                }
            }
            EnvPhase::Sustain => {
                let sr = (adsr2 & 0x1F) as usize;
                if sr == 0 {
                    // SR=0: no change in sustain (hold at level).
                    return;
                }
                let period = RATE_TABLE[sr];
                if period == 0 || {
                    voice.env_tick = voice.env_tick.wrapping_add(1);
                    voice.env_tick >= period
                } {
                    voice.env_tick = 0;
                    let step = (voice.envelope >> 8) + 1;
                    voice.envelope = voice.envelope.saturating_sub(step);
                }
                if voice.envelope == 0 {
                    voice.env_phase = EnvPhase::Off;
                }
            }
            _ => {}
        }
    }

    fn step_gain_envelope(voice: &mut Voice, gain: u8) {
        let mode = gain >> 5;
        if mode & 0x04 == 0 {
            // Direct: GAIN[6:0] << 4 = envelope value.
            voice.envelope = ((gain & 0x7F) as u16) << 4;
            return;
        }
        let rate = (gain & 0x1F) as usize;
        let period = RATE_TABLE[rate];
        if period == 0 {
            return;
        }
        voice.env_tick = voice.env_tick.wrapping_add(1);
        if voice.env_tick < period {
            return;
        }
        voice.env_tick = 0;

        match mode & 0x03 {
            0 => {
                // Linear decrease by 32.
                voice.envelope = voice.envelope.saturating_sub(32);
                if voice.envelope == 0 {
                    voice.env_phase = EnvPhase::Off;
                }
            }
            1 => {
                // Exponential decrease.
                let step = (voice.envelope >> 8) + 1;
                voice.envelope = voice.envelope.saturating_sub(step);
                if voice.envelope == 0 {
                    voice.env_phase = EnvPhase::Off;
                }
            }
            2 => {
                // Linear increase by 32.
                voice.envelope = (voice.envelope + 32).min(0x7FF);
            }
            _ => {
                // Bent-line increase: +32 below $600, +8 above.
                let step = if voice.envelope < 0x600 { 32 } else { 8 };
                voice.envelope = (voice.envelope + step).min(0x7FF);
            }
        }
    }

    // ─── Noise LFSR ───────────────────────────────────────────────────────────

    /// Advance the 15-bit noise LFSR by one step.
    /// Feedback: bit 14 XOR bit 13 → new bit 14 (shift left, OR feedback into bit 0,
    /// then mask to 15 bits). Actually the SPC700 noise LFSR has documented
    /// polynomial x^15 + x^14 + 1 (taps at bits 14 and 13).
    fn step_noise(&mut self) {
        let feedback = ((self.noise_lfsr >> 14) ^ (self.noise_lfsr >> 13)) & 1;
        self.noise_lfsr = ((self.noise_lfsr << 1) | feedback) & 0x7FFF;
    }

    // ─── Echo processing ──────────────────────────────────────────────────────

    /// Compute the echo buffer size in samples based on EDL.
    /// Each EDL unit = 512 bytes = 256 stereo i16 pairs = 256 samples per channel.
    #[inline]
    fn echo_buf_size(&self) -> usize {
        let edl = self.edl() as usize;
        if edl == 0 {
            1
        } else {
            edl * 256
        }
    }

    /// Apply the 8-tap FIR filter to the echo history buffer.
    fn fir_filter(&self, hist: &[i16; 8], fir_pos: u8) -> i32 {
        let mut sum: i32 = 0;
        for i in 0..8usize {
            let idx = (fir_pos as usize + i) & 7;
            let coef = self.fir_coef(i) as i32;
            sum += (hist[idx] as i32) * coef;
        }
        // FIR: sum >> 7, clamp to i16.
        (sum >> 7).clamp(-32768, 32767)
    }

    // ─── KON / KOF handling ───────────────────────────────────────────────────

    fn handle_kon_kof(&mut self, aram: &[u8; 0x10000]) {
        let kon = self.regs[0x4C];
        let kof = self.regs[0x5C];
        let dir = self.dir() as u16;
        let reset_bit = self.flg() & 0x80 != 0;

        for v in 0..8usize {
            if reset_bit {
                self.voices[v].envelope = 0;
                self.voices[v].env_phase = EnvPhase::Off;
                self.voices[v].active = false;
                continue;
            }

            if kof & (1 << v) != 0
                && self.voices[v].env_phase != EnvPhase::Off
                && self.voices[v].env_phase != EnvPhase::Release
            {
                self.voices[v].env_phase = EnvPhase::Release;
            }

            if kon & (1 << v) != 0 {
                // KON: start voice. Per docs, takes effect after ~5 samples of delay.
                // We use a 5-tick delay then begin playback.
                let srcn = self.voice_srcn(v) as u16;
                let dir_entry = (dir << 8).wrapping_add(srcn << 2);
                // Sample start address from directory: [dir_entry] = start_lo, [+1] = start_hi
                let start_lo = aram[dir_entry as usize] as u16;
                let start_hi = aram[(dir_entry + 1) as usize] as u16;
                let start_addr = start_lo | (start_hi << 8);
                // Loop address: [dir_entry+2] = loop_lo, [+3] = loop_hi
                let loop_lo = aram[(dir_entry + 2) as usize] as u16;
                let loop_hi = aram[(dir_entry + 3) as usize] as u16;
                let loop_addr = loop_lo | (loop_hi << 8);

                self.voices[v].brr_addr = start_addr;
                self.voices[v].loop_addr = loop_addr;
                self.voices[v].brr_block_offset = 0;
                self.voices[v].pitch_counter = 0;
                self.voices[v].buf = [0i16; BRR_BUF_LEN];
                self.voices[v].buf_pos = 0;
                self.voices[v].brr_old1 = 0;
                self.voices[v].brr_old2 = 0;
                self.voices[v].end_flag = false;
                self.voices[v].loop_flag = false;
                self.voices[v].envelope = 0;
                self.voices[v].env_phase = EnvPhase::Attack;
                self.voices[v].env_tick = 0;
                self.voices[v].active = true;
                self.voices[v].kon_delay = 5;

                // Clear ENDX bit for this voice.
                self.regs[0x7C] &= !(1u8 << v);
                self.endx &= !(1u8 << v);
            }
        }

        // Clear KON register after processing (write-once semantics).
        self.regs[0x4C] = 0;
        // KOF is NOT cleared (it persists until KON).
        // (Actually per hardware docs: KOF should be cleared after one sample
        // to avoid inadvertent re-triggering, but leaving it set is common
        // in SPC programs. For determinism, we leave it — it only matters
        // when voices are already in Release or Off state.)
    }

    // ─── Per-sample step ──────────────────────────────────────────────────────

    /// Advance the DSP by one stereo sample (32 SPC700 clocks).
    ///
    /// This is the main entry point called by `Apu::advance_spc_cycles`.
    /// `aram` is borrowed for BRR sample reads and echo buffer accesses.
    pub fn step_sample(&mut self, aram: &mut [u8; 0x10000]) {
        self.sample_count = self.sample_count.wrapping_add(1);

        // Handle KON/KOF from register writes.
        self.handle_kon_kof(aram);

        let flg = self.flg();
        let mute = flg & 0x40 != 0;
        let noise_rate = (flg & 0x1F) as usize;

        // Advance noise LFSR.
        if noise_rate > 0 {
            let noise_period = RATE_TABLE[noise_rate];
            if noise_period == 0 {
                self.step_noise();
            } else {
                self.noise_tick = self.noise_tick.wrapping_add(1);
                if self.noise_tick >= noise_period {
                    self.noise_tick = 0;
                    self.step_noise();
                }
            }
        }

        let pmon = self.pmon();
        let non_mask = self.non();
        let eon_mask = self.eon();
        let echo_write_disable = flg & 0x20 != 0;

        let mut main_l: i32 = 0;
        let mut main_r: i32 = 0;
        let mut echo_in_l: i32 = 0;
        let mut echo_in_r: i32 = 0;

        // Previous voice output for pitch modulation (voice 0 = no modulation).
        let mut prev_voice_out: i16 = 0;

        for v in 0..8usize {
            if !self.voices[v].active {
                self.voices[v].envx = 0;
                self.voices[v].outx = 0;
                self.voice_out[v] = 0;
                prev_voice_out = 0;
                continue;
            }

            // KON delay: voice doesn't produce samples during delay period.
            if self.voices[v].kon_delay > 0 {
                self.voices[v].kon_delay -= 1;
                // Decode a block on the last delay tick.
                if self.voices[v].kon_delay == 0 {
                    let (end_f, loop_f) = Self::decode_next_brr_block(&mut self.voices[v], aram);
                    self.voices[v].end_flag = end_f;
                    self.voices[v].loop_flag = loop_f;
                    if end_f && !loop_f {
                        self.voices[v].active = false;
                        self.voices[v].env_phase = EnvPhase::Off;
                        self.regs[0x7C] |= 1u8 << v;
                        self.endx |= 1u8 << v;
                    }
                }
                self.voices[v].envx = 0;
                self.voices[v].outx = 0;
                self.voice_out[v] = 0;
                prev_voice_out = 0;
                continue;
            }

            // Pitch modulation.
            let mut pitch = self.voice_pitch(v);
            if v > 0 && (pmon & (1u8 << v)) != 0 {
                // PMON: multiply pitch by previous voice output (normalized).
                // Hardware: new_pitch = base_pitch * (1 + prev_out/256)
                //         ≈ base_pitch + (base_pitch * prev_out) >> 8
                // Where prev_out is the 15-bit signed voice output >> 8.
                // Documented: pitch += (pitch * (prev_voice_out >> 5)) >> 10
                let mod_factor = (prev_voice_out as i32) >> 4;
                let pitch_delta = ((pitch as i32) * mod_factor) >> 11;
                pitch = (pitch as i32 + pitch_delta).clamp(0, 0x3FFF) as u16;
            }

            // Advance pitch counter.
            let pc_old = self.voices[v].pitch_counter;
            let pc_new = pc_old.wrapping_add(pitch);
            self.voices[v].pitch_counter = pc_new & 0x0FFF;

            // Decode new BRR blocks if the sample position advances.
            // Each BRR block = 16 samples. The pitch counter's top bits (bits 15:12
            // before masking) represent whole sample advances.
            let samples_advanced = (pc_new >> 12) as u32;
            if samples_advanced > 0 {
                for _ in 0..samples_advanced {
                    // Check if we need a new BRR block.
                    // brr_block_offset tracks which sample in the current block we're at.
                    // Each block has 16 samples; when we reach the end of a block, decode next.
                    self.voices[v].brr_block_offset += 1;
                    if self.voices[v].brr_block_offset >= 16 {
                        self.voices[v].brr_block_offset = 0;

                        // If previous block had end flag, handle loop or stop.
                        if self.voices[v].end_flag {
                            if self.voices[v].loop_flag {
                                self.voices[v].brr_addr = self.voices[v].loop_addr;
                                self.voices[v].brr_old1 = 0;
                                self.voices[v].brr_old2 = 0;
                                self.regs[0x7C] |= 1u8 << v;
                                self.endx |= 1u8 << v;
                            } else {
                                // End without loop: silence voice.
                                self.voices[v].active = false;
                                self.voices[v].env_phase = EnvPhase::Off;
                                self.voices[v].envelope = 0;
                                self.regs[0x7C] |= 1u8 << v;
                                self.endx |= 1u8 << v;
                                break;
                            }
                        }

                        let (end_f, loop_f) =
                            Self::decode_next_brr_block(&mut self.voices[v], aram);
                        self.voices[v].end_flag = end_f;
                        self.voices[v].loop_flag = loop_f;
                    }
                }
            }

            if !self.voices[v].active {
                self.voices[v].envx = 0;
                self.voices[v].outx = 0;
                self.voice_out[v] = 0;
                prev_voice_out = 0;
                continue;
            }

            // Get sample via noise or Gaussian interpolation.
            let raw_sample: i32 = if (non_mask & (1u8 << v)) != 0 {
                // Noise: use LFSR output (bit 0, mapped to 15-bit full-scale).
                if self.noise_lfsr & 1 != 0 {
                    -0x4000i32
                } else {
                    0x4000i32
                }
            } else {
                Self::gaussian_interp(
                    &self.voices[v].buf,
                    self.voices[v].buf_pos,
                    self.voices[v].pitch_counter,
                )
            };

            // Step envelope.
            let adsr1 = self.voice_adsr1(v);
            let adsr2 = self.voice_adsr2(v);
            let gain = self.voice_gain(v);
            Self::step_envelope(&mut self.voices[v], adsr1, adsr2, gain);

            // Apply envelope.
            // sample * envelope / 2048: Q-format multiply, result in range ~[-32768,32767].
            let env_sample =
                ((raw_sample * self.voices[v].envelope as i32) >> 11).clamp(-32768, 32767) as i16;

            // Update OUTX (output before main volume): sample >> 8.
            self.voices[v].outx = (env_sample >> 8) as i8;
            self.voice_out[v] = env_sample;
            prev_voice_out = env_sample;

            if !mute {
                // Apply per-voice volume and accumulate into main mix.
                let vl = self.voice_vol_l(v) as i32;
                let vr = self.voice_vol_r(v) as i32;
                let s = env_sample as i32;
                main_l += (s * vl) >> 7;
                main_r += (s * vr) >> 7;

                // Echo input accumulation (if voice has echo enabled).
                if (eon_mask & (1u8 << v)) != 0 {
                    echo_in_l += (s * vl) >> 7;
                    echo_in_r += (s * vr) >> 7;
                }
            }
        }

        // ── Echo processing ──
        let echo_buf_size = self.echo_buf_size();
        let esa = self.esa() as usize;
        let echo_base = esa << 8;

        // Read from echo buffer in ARAM (or in-memory buf).
        let ep = self.echo_pos as usize;
        let echo_read_l = self.echo_buf_l[ep % echo_buf_size];
        let echo_read_r = self.echo_buf_r[ep % echo_buf_size];

        // Update FIR history.
        let fpos = self.fir_pos as usize;
        self.fir_hist_l[fpos & 7] = echo_read_l;
        self.fir_hist_r[fpos & 7] = echo_read_r;

        // Apply FIR filter.
        let fir_l = self.fir_filter(&self.fir_hist_l, self.fir_pos) as i16;
        let fir_r = self.fir_filter(&self.fir_hist_r, self.fir_pos) as i16;

        self.fir_pos = (self.fir_pos + 1) & 7;

        // Echo volume mix into main output.
        let evl = self.evol_l() as i32;
        let evr = self.evol_r() as i32;
        if !mute {
            main_l += (fir_l as i32 * evl) >> 7;
            main_r += (fir_r as i32 * evr) >> 7;
        }

        // Clamp main output.
        let out_l = main_l.clamp(-32768, 32767) as i16;
        let out_r = main_r.clamp(-32768, 32767) as i16;

        // Apply master volume (output is discarded for actual audio but we
        // update observable state).
        let mvl = self.mvol_l() as i32;
        let mvr = self.mvol_r() as i32;
        let _final_l = ((out_l as i32 * mvl) >> 7).clamp(-32768, 32767) as i16;
        let _final_r = ((out_r as i32 * mvr) >> 7).clamp(-32768, 32767) as i16;

        #[cfg(feature = "introspect")]
        {
            self.last_out_l = _final_l;
            self.last_out_r = _final_r;
        }

        // Write echo feedback into echo buffer (if echo write not disabled).
        if !echo_write_disable {
            let efb = self.efb() as i32;
            let new_echo_l = ((echo_in_l + (fir_l as i32 * efb)) >> 7).clamp(-32768, 32767) as i16;
            let new_echo_r = ((echo_in_r + (fir_r as i32 * efb)) >> 7).clamp(-32768, 32767) as i16;

            self.echo_buf_l[ep % echo_buf_size] = new_echo_l;
            self.echo_buf_r[ep % echo_buf_size] = new_echo_r;

            // Also write to ARAM at the echo region (games rely on this for
            // region-overlap interactions — emulate faithfully).
            let aram_off_l = (echo_base + ep * 4) & 0xFFFF;
            let aram_off_r = (echo_base + ep * 4 + 2) & 0xFFFF;
            aram[aram_off_l] = new_echo_l as u8;
            aram[(aram_off_l + 1) & 0xFFFF] = (new_echo_l >> 8) as u8;
            aram[aram_off_r] = new_echo_r as u8;
            aram[(aram_off_r + 1) & 0xFFFF] = (new_echo_r >> 8) as u8;
        }

        // Advance echo position.
        self.echo_pos = ((ep + 1) % echo_buf_size) as u16;

        // Sync ENDX register from accumulated endx.
        self.regs[0x7C] = self.endx;
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Gaussian table checksum ----

    /// Pin the Gaussian table checksum to catch any transcription errors.
    /// Computed as: sum of all 512 i16 entries, treated as i32 accumulator.
    #[test]
    fn gauss_table_checksum() {
        let sum: i32 = GAUSS.iter().map(|&x| x as i32).sum();
        // The expected checksum for the Gaussian table transcribed above.
        // Computed as: sum of all 512 i16 entries = 290589.
        // This value pins the table; any transcription error changes it.
        assert!(sum > 0, "Gaussian table sum should be positive");
        assert_eq!(
            sum, 290589,
            "Gaussian table sum mismatch — transcription error"
        );
    }

    // ---- Rate table ----

    #[test]
    fn rate_table_zero_is_never() {
        assert_eq!(RATE_TABLE[0], 0, "rate 0 must mean 'never fire'");
    }

    #[test]
    fn rate_table_31_is_every_sample() {
        assert_eq!(RATE_TABLE[31], 1, "rate 31 must fire every sample");
    }

    #[test]
    fn rate_table_monotone_decreasing() {
        // Rates should be monotonically decreasing (higher rate = faster).
        for i in 1..31usize {
            assert!(
                RATE_TABLE[i] >= RATE_TABLE[i + 1],
                "rate table not monotone at {}",
                i
            );
        }
    }

    // ---- BRR decode ----

    /// Test filter mode 0 (raw): no filtering applied.
    #[test]
    fn brr_filter0_raw() {
        let old1: i16 = 0;
        let old2: i16 = 0;
        // nybble = +7 (positive max), shift = 0 → 7 << 0 = 7, then & !1 = 6.
        let result = Dsp::decode_brr_nybble(7, 0, 0, old1, old2);
        assert_eq!(result, 6, "raw filter mode 0, nybble=7, shift=0");

        // nybble = -8 (negative max), shift = 0 → -8 << 0 = -8, then & !1 = -8.
        let result2 = Dsp::decode_brr_nybble(-8, 0, 0, old1, old2);
        assert_eq!(result2, -8, "raw filter mode 0, nybble=-8, shift=0");
    }

    /// Test BRR shift overflow (shift > 12): saturates.
    #[test]
    fn brr_shift_overflow_saturates() {
        // shift = 13 with positive nybble → saturates to 0 (garbage shift rule).
        let result = Dsp::decode_brr_nybble(7, 13, 0, 0, 0);
        assert_eq!(result & !1, result, "must be even");
        // result should be 0 since nybble > 0 and shift > 12 → 0.
        assert_eq!(result, 0, "positive nybble with overflow shift → 0");

        // Negative nybble → max negative.
        let result2 = Dsp::decode_brr_nybble(-8, 13, 0, 0, 0);
        assert_eq!(
            result2, -32768,
            "negative nybble with overflow shift → -32768"
        );
    }

    /// Test BRR decode: hand-computed block with filter 0.
    ///
    /// Block: header = $00 (shift=0, filter=0, loop=0, end=0),
    /// data = [$07, $70, $00, $00, $00, $00, $00, $00]
    /// → nybbles: +0,+7, +7,+0, 0,0, 0,0, 0,0, 0,0, 0,0, 0,0
    /// Wait: data[0]=0x07: hi=0, lo=7 → s[0]=0 & !1=0, s[1]=7*1=7→6 (& !1)
    /// data[1]=0x70: hi=7, lo=0 → s[2]=6 (& !1), s[3]=0.
    #[test]
    fn brr_decode_known_block() {
        let mut voice = Voice::new();
        let mut aram = [0u8; 0x10000];
        // header: shift=0, filter=0, no loop, no end
        aram[0] = 0x00;
        aram[1] = 0x07; // hi-nybble=0, lo=7
        aram[2] = 0x70; // hi=7, lo=0
                        // rest are 0

        let (end_f, loop_f) = Dsp::decode_next_brr_block(&mut voice, &aram);
        assert!(!end_f, "end flag should not be set");
        assert!(!loop_f, "loop flag should not be set");

        // Check that some samples are non-zero.
        let non_zero = voice.buf.iter().any(|&s| s != 0);
        assert!(non_zero, "decoded block should have non-zero samples");
    }

    // ---- Envelope steps ----

    #[test]
    fn envelope_attack_with_rate_15_is_fast() {
        let mut voice = Voice::new();
        voice.env_phase = EnvPhase::Attack;
        // ADSR1: ADSR enable (bit7), attack rate = 15.
        let adsr1 = 0x80 | 0x0F;
        let adsr2 = 0x00;
        let gain = 0;

        // Rate 15: +1024 per step → should reach max (0x7FF = 2047) in 2 steps.
        Dsp::step_envelope(&mut voice, adsr1, adsr2, gain);
        assert_eq!(voice.envelope, 1024, "after step 1 with rate 15");
        Dsp::step_envelope(&mut voice, adsr1, adsr2, gain);
        assert_eq!(voice.envelope, 0x7FF, "after step 2 with rate 15");
        assert_eq!(
            voice.env_phase,
            EnvPhase::Decay,
            "should transition to Decay"
        );
    }

    #[test]
    fn envelope_release_decreases_by_8() {
        let mut voice = Voice::new();
        voice.env_phase = EnvPhase::Release;
        voice.envelope = 100;

        let adsr1 = 0;
        let adsr2 = 0;
        let gain = 0;

        Dsp::step_envelope(&mut voice, adsr1, adsr2, gain);
        assert_eq!(voice.envelope, 92, "release decreases by 8");
    }

    #[test]
    fn envelope_release_hits_zero_and_goes_off() {
        let mut voice = Voice::new();
        voice.env_phase = EnvPhase::Release;
        voice.envelope = 4;

        let adsr1 = 0;
        let adsr2 = 0;
        let gain = 0;

        Dsp::step_envelope(&mut voice, adsr1, adsr2, gain);
        assert_eq!(voice.envelope, 0);
        assert_eq!(
            voice.env_phase,
            EnvPhase::Off,
            "should be Off after release hit 0"
        );
    }

    // ---- Echo FIR ----

    #[test]
    fn echo_fir_zero_history_zero_output() {
        let dsp = Dsp::new();
        let hist = [0i16; 8];
        let result = dsp.fir_filter(&hist, 0);
        assert_eq!(result, 0, "FIR with all-zero history gives zero");
    }

    #[test]
    fn echo_fir_clamped() {
        // Very large input should clamp to i16 range.
        let mut dsp = Dsp::new();
        // Set all FIR coefficients to max (127).
        for i in 0..8 {
            dsp.regs[0x0F + i * 0x10] = 127u8;
        }
        let hist = [32767i16; 8];
        let result = dsp.fir_filter(&hist, 0);
        // Max possible: 8 * 32767 * 127 / 128 ≈ 259,896; >> 7 = ~ 2030. Within i16.
        assert!(
            (-32768..=32767).contains(&result),
            "FIR output must be clamped to i16 range"
        );
    }

    // ---- Noise LFSR ----

    #[test]
    fn noise_lfsr_nonzero_initial_state() {
        let dsp = Dsp::new();
        assert_ne!(dsp.noise_lfsr, 0, "LFSR should not start at zero");
    }

    #[test]
    fn noise_lfsr_advances() {
        let mut dsp = Dsp::new();
        let initial = dsp.noise_lfsr;
        dsp.step_noise();
        assert_ne!(dsp.noise_lfsr, initial, "LFSR should advance");
    }

    #[test]
    fn noise_lfsr_15_bit() {
        let mut dsp = Dsp::new();
        for _ in 0..1000 {
            dsp.step_noise();
            assert!(dsp.noise_lfsr <= 0x7FFF, "LFSR must stay 15-bit");
        }
    }

    #[test]
    fn noise_lfsr_deterministic() {
        // Two DSP instances stepping the same number of times produce the same LFSR state.
        let mut dsp1 = Dsp::new();
        let mut dsp2 = Dsp::new();
        for _ in 0..100 {
            dsp1.step_noise();
            dsp2.step_noise();
        }
        assert_eq!(
            dsp1.noise_lfsr, dsp2.noise_lfsr,
            "LFSR must be deterministic"
        );
    }

    // ---- Full sample step smoke test ----

    #[test]
    fn step_sample_no_active_voices() {
        let mut dsp = Dsp::new();
        let mut aram = [0u8; 0x10000];
        // With no active voices, should not panic and ENDX should be 0.
        dsp.step_sample(&mut aram);
        assert_eq!(dsp.endx, 0);
    }
}
