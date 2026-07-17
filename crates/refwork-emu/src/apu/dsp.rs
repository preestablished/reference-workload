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
// The hardware uses a 512-entry table of i16 Gaussian-windowed sinc
// coefficients for 4-tap interpolation (D3: transcribed verbatim from the
// published S-DSP table; the previous table in this file was not the
// hardware table — see the fixed-D3 note below).
//
// The fractional part of the pitch counter (bits 11:4 of the 12-bit
// sub-sample position, giving `frac` in 0..=255) selects four taps from the
// four samples `s0..s3` (oldest..newest) via (D4-fixed assignment):
//   s0: GAUSS[255 - frac]
//   s1: GAUSS[511 - frac]   (dominant tap at frac=0: the table's near-max)
//   s2: GAUSS[256 + frac]
//   s3: GAUSS[frac]         (near-zero at frac=0: GAUSS[0] == 0)
// Every 4-tap kernel (the four values above, for a given `frac`) sums to
// ~2048 (unity gain after the `>>11` normalization); this is what the
// `fidelity_gauss_table_matches_published_values` test checks.

/// 512-entry Gaussian interpolation table (i16 coefficients).
///
/// Indexed as `GAUSS[i]` for i in 0..512. For a fractional offset `f`
/// (0..=0xFFF), `frac = (f >> 4) & 0xFF` (0..=255) selects the four taps
/// per the layout documented above.
///
/// The checksum (sum of all 512 entries, wrapping i32) is pinned by a unit
/// test below to catch any transcription errors.
pub const GAUSS: [i16; 512] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2,
    2, 2, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5, 5,
    6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10,
    11, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 15, 16, 16, 17, 17,
    18, 19, 19, 20, 20, 21, 21, 22, 23, 23, 24, 24, 25, 26, 27, 27,
    28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 36, 36, 37, 38, 39, 40,
    41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56,
    58, 59, 60, 61, 62, 64, 65, 66, 67, 69, 70, 71, 73, 74, 76, 77,
    78, 80, 81, 83, 84, 86, 87, 89, 90, 92, 94, 95, 97, 99, 100, 102,
    104, 106, 107, 109, 111, 113, 115, 117, 118, 120, 122, 124, 126, 128, 130, 132,
    134, 137, 139, 141, 143, 145, 147, 150, 152, 154, 156, 159, 161, 163, 166, 168,
    171, 173, 175, 178, 180, 183, 186, 188, 191, 193, 196, 199, 201, 204, 207, 210,
    212, 215, 218, 221, 224, 227, 230, 233, 236, 239, 242, 245, 248, 251, 254, 257,
    260, 263, 267, 270, 273, 276, 280, 283, 286, 290, 293, 297, 300, 304, 307, 311,
    314, 318, 321, 325, 328, 332, 336, 339, 343, 347, 351, 354, 358, 362, 366, 370,
    374, 378, 381, 385, 389, 393, 397, 401, 405, 410, 414, 418, 422, 426, 430, 434,
    439, 443, 447, 451, 456, 460, 464, 469, 473, 477, 482, 486, 491, 495, 499, 504,
    508, 513, 517, 522, 527, 531, 536, 540, 545, 550, 554, 559, 563, 568, 573, 577,
    582, 587, 592, 596, 601, 606, 611, 615, 620, 625, 630, 635, 640, 644, 649, 654,
    659, 664, 669, 674, 678, 683, 688, 693, 698, 703, 708, 713, 718, 723, 728, 732,
    737, 742, 747, 752, 757, 762, 767, 772, 777, 782, 787, 792, 797, 802, 806, 811,
    816, 821, 826, 831, 836, 841, 846, 851, 855, 860, 865, 870, 875, 880, 884, 889,
    894, 899, 904, 908, 913, 918, 923, 927, 932, 937, 941, 946, 951, 955, 960, 965,
    969, 974, 978, 983, 988, 992, 997, 1001, 1005, 1010, 1014, 1019, 1023, 1027, 1032, 1036,
    1040, 1045, 1049, 1053, 1057, 1061, 1066, 1070, 1074, 1078, 1082, 1086, 1090, 1094, 1098, 1102,
    1106, 1109, 1113, 1117, 1121, 1125, 1128, 1132, 1136, 1139, 1143, 1146, 1150, 1153, 1157, 1160,
    1164, 1167, 1170, 1174, 1177, 1180, 1183, 1186, 1190, 1193, 1196, 1199, 1202, 1205, 1207, 1210,
    1213, 1216, 1219, 1221, 1224, 1227, 1229, 1232, 1234, 1237, 1239, 1241, 1244, 1246, 1248, 1251,
    1253, 1255, 1257, 1259, 1261, 1263, 1265, 1267, 1269, 1270, 1272, 1274, 1275, 1277, 1279, 1280,
    1282, 1283, 1284, 1286, 1287, 1288, 1290, 1291, 1292, 1293, 1294, 1295, 1296, 1297, 1297, 1298,
    1299, 1300, 1300, 1301, 1302, 1302, 1303, 1303, 1303, 1304, 1304, 1304, 1304, 1304, 1305, 1305,
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

/// Size of the BRR decode ring buffer per voice (32 samples: the current
/// *and* previous BRR block, 16 samples each). D1: a single 16-sample block
/// isn't enough — the 4-tap gaussian interpolator's 3-sample look-behind
/// must be able to read across a block boundary into the tail of the
/// previously decoded block, not just within the block currently playing.
pub const BRR_BUF_LEN: usize = 32;

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
    /// Current playback position within the currently-decoded 16-sample BRR
    /// block (0..=15). Advanced by one for each whole-sample pitch-counter
    /// carry; when it would reach 16, it wraps to 0 and the next block is
    /// decoded (D1: this is also the tap-selection input to
    /// [`Dsp::gaussian_interp`], so the interpolator tracks playback
    /// position instead of freezing at the block's start).
    pub brr_block_offset: u8,
    /// Decoded PCM ring buffer: the current and previous 16-sample BRR
    /// blocks (32 samples total). New samples are appended at `buf_pos`
    /// modulo `BRR_BUF_LEN`; the previous block's tail survives the
    /// boundary so gaussian interpolation can look behind it (D1).
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

// ─── Audio capture ring (feature "audio" only) ────────────────────────────────

/// Capacity of the host-facing audio capture ring, in stereo pairs. 4096
/// pairs (~128 ms at 32 kHz) is comfortably above the ~532 pairs/frame a
/// 60 fps drain sees (357,368 master cycles/frame × 1024/21477 ÷ 32 ≈ 532.5).
#[cfg(feature = "audio")]
const AUDIO_RING_CAP: usize = 4096;

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

    /// Host-facing audio capture ring (feature "audio" only): interleaved
    /// stereo i16 pairs (L0,R0,L1,R1,...), stored as an inline fixed-size
    /// array matching the by-value buffer style used elsewhere in this
    /// struct (`echo_buf_l`/`echo_buf_r` above) — no heap `Vec`, no
    /// per-frame allocation (D8). `AUDIO_RING_CAP` pairs of capacity;
    /// `audio_ring_head`/`audio_ring_len` track the valid region.
    /// Overflow policy: overwrite the oldest pair and count it in
    /// `audio_dropped_pairs`, so lanes that build with the feature on but
    /// never drain (e.g. non-interactive replay) stay bounded in memory.
    #[cfg(feature = "audio")]
    audio_ring: [i16; AUDIO_RING_CAP * 2],
    #[cfg(feature = "audio")]
    audio_ring_head: usize,
    #[cfg(feature = "audio")]
    audio_ring_len: usize,
    #[cfg(feature = "audio")]
    audio_dropped_pairs: u64,

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
            #[cfg(feature = "audio")]
            audio_ring: [0i16; AUDIO_RING_CAP * 2],
            #[cfg(feature = "audio")]
            audio_ring_head: 0,
            #[cfg(feature = "audio")]
            audio_ring_len: 0,
            #[cfg(feature = "audio")]
            audio_dropped_pairs: 0,
            sample_count: 0,
            endx: 0,
        }
    }

    // ─── Audio capture (feature "audio" only) ────────────────────────────────

    /// Push one computed stereo pair into the capture ring. Overflow policy:
    /// overwrite the oldest pair and count it in `audio_dropped_pairs`.
    #[cfg(feature = "audio")]
    fn push_audio_pair(&mut self, l: i16, r: i16) {
        if self.audio_ring_len == AUDIO_RING_CAP {
            // Full: overwrite the oldest pair in place and advance head.
            let idx = self.audio_ring_head;
            self.audio_ring[idx * 2] = l;
            self.audio_ring[idx * 2 + 1] = r;
            self.audio_ring_head = (self.audio_ring_head + 1) % AUDIO_RING_CAP;
            self.audio_dropped_pairs = self.audio_dropped_pairs.wrapping_add(1);
        } else {
            let idx = (self.audio_ring_head + self.audio_ring_len) % AUDIO_RING_CAP;
            self.audio_ring[idx * 2] = l;
            self.audio_ring[idx * 2 + 1] = r;
            self.audio_ring_len += 1;
        }
    }

    /// Move up to `out.len() / 2` pending stereo pairs into `out`
    /// (interleaved L,R — `out[0]` is the oldest pending left sample,
    /// `out[1]` its right partner, and so on). Returns the number of `i16`
    /// values written, always even. Samples beyond what fits in `out` (or
    /// beyond what has been produced since the last drain) remain queued for
    /// the next call; samples already lost to ring overflow are counted in
    /// [`Dsp::audio_dropped_pairs`] and cannot be recovered.
    #[cfg(feature = "audio")]
    pub fn drain_audio(&mut self, out: &mut [i16]) -> usize {
        let want = out.len() / 2;
        let n = want.min(self.audio_ring_len);
        for i in 0..n {
            let idx = (self.audio_ring_head + i) % AUDIO_RING_CAP;
            out[2 * i] = self.audio_ring[idx * 2];
            out[2 * i + 1] = self.audio_ring[idx * 2 + 1];
        }
        self.audio_ring_head = (self.audio_ring_head + n) % AUDIO_RING_CAP;
        self.audio_ring_len -= n;
        n * 2
    }

    /// Count of stereo pairs discarded by ring overflow (overwrite-oldest)
    /// since construction. Never decreases.
    #[cfg(feature = "audio")]
    pub fn audio_dropped_pairs(&self) -> u64 {
        self.audio_dropped_pairs
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
    /// Algorithm (FIX1-corrected, transliterated verbatim from blargg's
    /// `SPC_DSP::decode_brr` — snes9x `apu/bapu/dsp/SPC_DSP.cpp`,
    /// hardware-verified reference; see the pinned vector table in
    /// `fidelity_brr_filter_coefficients_match_hardware` below, which is
    /// the arbiter for this function). The key hardware detail: the filter
    /// is applied while `s` is still in the *halved* domain — it is only
    /// doubled at the very end, after the 16-bit clamp. An earlier version
    /// of this function doubled `s` before filtering and reformulated the
    /// filter coefficients for the full-scale domain; that reordering is
    /// *not* bit-exact with hardware because the intermediate right-shifts
    /// truncate differently in each domain (e.g. filter 2, nybble 0,
    /// shift 0, old1=old2=-100 must decode to exactly -100 on hardware).
    ///
    /// 1. Extract the 4-bit signed nybble and shift it into the halved
    ///    domain: `(nybble << shift) >> 1` for valid shifts (0..=12).
    ///    Shifts 13..=15 are the documented invalid-shift case: hardware
    ///    masks the halved value to `nybble & !0x7FF`, which collapses to
    ///    `-2048` for a negative nybble and `0` otherwise (still in the
    ///    halved domain — doubled at step 3 like everything else).
    /// 2. Apply the filter directly to the halved `s`, hardware forms.
    ///    `p1` is the FULL-SCALE previous sample (`old1`, not halved);
    ///    `p2` is the previous-previous sample HALVED (`old2 >> 1`) — this
    ///    asymmetry (p1 full-scale, p2 halved) is exactly what the
    ///    hardware does, not a simplification:
    ///    - mode 0: raw (`s` unchanged)
    ///    - mode 1: `s += (p1 >> 1) + ((-p1) >> 5)`
    ///    - mode 2: `s += p1; s -= p2; s += (p2 >> 4); s += ((p1*-3) >> 6)`
    ///    - mode 3: `s += p1; s -= p2; s += ((p1*-13) >> 7); s += ((p2*3) >> 4)`
    /// 3. D5: clamp the halved-domain accumulator to `i16` range
    ///    (`[-32768, 32767]`), THEN double (`* 2`) with 16-bit wraparound
    ///    (a wrap, not a saturate — values whose double would exceed the
    ///    `i16` range wrap around). The doubled result is always even by
    ///    construction, matching hardware's even-sample invariant.
    fn decode_brr_nybble(raw4: i32, shift: u8, filter: u8, old1: i16, old2: i16) -> i16 {
        // 1. Shift into the halved domain. shift > 12 is the documented
        // invalid-shift case, masked (not shifted) in the halved domain.
        let mut s: i32 = if shift <= 12 {
            (raw4 << shift) >> 1
        } else {
            raw4 & !0x7FF
        };

        // 2. Apply filter directly in the halved domain (hardware forms;
        // p1 full-scale, p2 halved — see doc comment above).
        let p1 = old1 as i32;
        let p2 = (old2 as i32) >> 1;
        match filter {
            0 => {}
            1 => {
                s += p1 >> 1;
                s += (-p1) >> 5;
            }
            2 => {
                s += p1;
                s -= p2;
                s += p2 >> 4;
                s += (p1 * -3) >> 6;
            }
            _ => {
                // 3
                s += p1;
                s -= p2;
                s += (p1 * -13) >> 7;
                s += (p2 * 3) >> 4;
            }
        }

        // 3. D5: clamp the halved-domain accumulator to i16 range, then
        // double with 16-bit wraparound (matches `CLAMP16(s); s = (int16_t)
        // (s * 2);` in the hardware reference exactly).
        let clamped = s.clamp(-32768, 32767);
        (clamped * 2) as i16
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
    /// buffer at the current playback position.
    ///
    /// `block_offset` (D1) is the voice's current sample position within
    /// the *currently playing* 16-sample BRR block (0..=15) — this is what
    /// makes the interpolator track playback instead of freezing at the
    /// block's first four samples for all 16 output ticks. The newest tap
    /// is at ring-buffer index `(buf_pos - 16 + block_offset) & 31`: since
    /// a block decode writes 16 fresh samples starting at the pre-decode
    /// `buf_pos` and then advances `buf_pos` by 16, `buf_pos - 16` is the
    /// start of the block currently playing, and adding `block_offset`
    /// lands on the sample at the current playback position within it. The
    /// three taps behind it (`newest-1..newest-3`, wrapping mod 32) reach
    /// back into the *previous* block once `block_offset` is small, which
    /// is exactly why `BRR_BUF_LEN` is 32 (current + previous block) rather
    /// than 16 — a 16-entry ring can't supply look-behind across a block
    /// boundary. On KON the ring is zeroed and `buf_pos`/`block_offset`
    /// both start at 0, so the very first block's look-behind reads zeros
    /// rather than stale/garbage data.
    ///
    /// The fractional part of the pitch counter (bits 11:4, 0..=255) selects
    /// the four GAUSS taps (D4-fixed assignment; oldest..newest = s0..s3):
    ///   s0: GAUSS[255 - frac]
    ///   s1: GAUSS[511 - frac]   (dominant tap at frac=0)
    ///   s2: GAUSS[256 + frac]
    ///   s3: GAUSS[frac]         (near-zero at frac=0)
    ///
    /// D10: hardware accumulates each tap's product individually shifted
    /// right by 11 (not the raw products summed then shifted once), wraps
    /// the running total to 16 bits after the first three taps — the
    /// documented interpolation-overflow quirk — then adds the fourth tap
    /// before the final clamp and even-only mask.
    fn gaussian_interp(buf: &[i16; BRR_BUF_LEN], buf_pos: u8, block_offset: u8, pitch_counter: u16) -> i32 {
        let frac = ((pitch_counter >> 4) & 0xFF) as usize;

        let len = BRR_BUF_LEN;
        let newest = (buf_pos as usize)
            .wrapping_sub(16)
            .wrapping_add(block_offset as usize)
            & (len - 1);
        let i3 = newest;
        let i2 = newest.wrapping_sub(1) & (len - 1);
        let i1 = newest.wrapping_sub(2) & (len - 1);
        let i0 = newest.wrapping_sub(3) & (len - 1);

        let s0 = buf[i0] as i32; // oldest
        let s1 = buf[i1] as i32;
        let s2 = buf[i2] as i32;
        let s3 = buf[i3] as i32; // newest

        let g0 = GAUSS[255 - frac] as i32;
        let g1 = GAUSS[511 - frac] as i32;
        let g2 = GAUSS[256 + frac] as i32;
        let g3 = GAUSS[frac] as i32;

        // D10: per-tap >>11, 16-bit wrap after the first three taps, then
        // add the fourth before the final clamp + even mask.
        let mut out: i32 = (g0 * s0) >> 11;
        out += (g1 * s1) >> 11;
        out += (g2 * s2) >> 11;
        out = out as i16 as i32;
        out += (g3 * s3) >> 11;

        out.clamp(-32768, 32767) & !1
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
                // D8: decay rate (DR) is ADSR1 bits 6:4, not 5:3 — the old
                // shift mixed the low attack-rate bit into the decay rate.
                let rate = ((adsr1 >> 4) & 0x07) as usize * 2 + 16;
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

    /// Advance the 15-bit noise LFSR by one step (D6).
    ///
    /// Hardware form: `new = (lfsr >> 1) | (((lfsr ^ (lfsr >> 1)) & 1) << 14)`
    /// — a *right*-shift LFSR with the feedback bit (XOR of bit 0 and the
    /// new bit 1, i.e. the two bits about to be shifted past each other)
    /// injected into bit 14. `lfsr >> 1` is at most 14 bits and the
    /// feedback term is either 0 or bit 14, so the result is always within
    /// 15 bits without needing an explicit mask — the `& 0x7FFF` below is
    /// defense-in-depth, not load-bearing.
    fn step_noise(&mut self) {
        let feedback = ((self.noise_lfsr ^ (self.noise_lfsr >> 1)) & 1) << 14;
        self.noise_lfsr = ((self.noise_lfsr >> 1) | feedback) & 0x7FFF;
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
                                // D7: do NOT reset brr_old1/brr_old2 here.
                                // Hardware carries the BRR filter history
                                // across the loop seam; zeroing it produced
                                // an audible click once per loop iteration.
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
                // D6: the noise sample is the 15-bit LFSR value shifted
                // into the 16-bit domain (matching how BRR/gaussian
                // samples are stored: full-scale, even values). Do NOT
                // shift back down afterward — that halves the noise level.
                ((self.noise_lfsr << 1) as i16) as i32
            } else {
                Self::gaussian_interp(
                    &self.voices[v].buf,
                    self.voices[v].buf_pos,
                    self.voices[v].brr_block_offset,
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
            // D10: mask the LSB (hardware voice output is always even).
            let env_sample = (((raw_sample * self.voices[v].envelope as i32) >> 11)
                .clamp(-32768, 32767) as i16)
                & !1;

            // Update OUTX (output before main volume): sample >> 8.
            self.voices[v].outx = (env_sample >> 8) as i8;
            self.voice_out[v] = env_sample;
            prev_voice_out = env_sample;

            if !mute {
                // Apply per-voice volume and accumulate into main mix.
                let vl = self.voice_vol_l(v) as i32;
                let vr = self.voice_vol_r(v) as i32;
                let s = env_sample as i32;
                let contrib_l = (s * vl) >> 7;
                let contrib_r = (s * vr) >> 7;
                // D10: clamp the main accumulator to 16 bits after *each*
                // voice add (hardware clamps here, not once at the end).
                main_l = (main_l + contrib_l).clamp(-32768, 32767);
                main_r = (main_r + contrib_r).clamp(-32768, 32767);

                // Echo input accumulation (if voice has echo enabled).
                // FIX3: clamp the echo accumulator to 16 bits after *each*
                // voice add, exactly like the main accumulator above —
                // hardware clamps both `t_main_out` and `t_echo_out` per
                // add in `voice_output` (blargg SPC_DSP.cpp), not once at
                // the end.
                if (eon_mask & (1u8 << v)) != 0 {
                    echo_in_l = (echo_in_l + contrib_l).clamp(-32768, 32767);
                    echo_in_r = (echo_in_r + contrib_r).clamp(-32768, 32767);
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

        // D10: MVOL applies to the voice sum (`main_l`/`main_r`) only.
        // EVOL applies to the FIR-filtered echo read (`fir_l`/`fir_r`) and
        // is added to the mix *after* the MVOL multiply — previously echo
        // was folded into `main_l`/`main_r` before the MVOL scale, so it
        // was double-scaled (once by EVOL, again by MVOL). Each term is
        // narrowed to i16 after its own `>>7` (matching hardware, which
        // truncates each scaled term individually before summing), then
        // the sum is clamped to 16 bits.
        let mvl = self.mvol_l() as i32;
        let mvr = self.mvol_r() as i32;
        let voice_scaled_l = ((main_l * mvl) >> 7) as i16 as i32;
        let voice_scaled_r = ((main_r * mvr) >> 7) as i16 as i32;

        let evl = self.evol_l() as i32;
        let evr = self.evol_r() as i32;
        let (echo_scaled_l, echo_scaled_r) = if mute {
            (0, 0)
        } else {
            (
                ((fir_l as i32 * evl) >> 7) as i16 as i32,
                ((fir_r as i32 * evr) >> 7) as i16 as i32,
            )
        };

        // Apply master volume: this is the final stereo sample for this tick.
        // With no feature enabled it is computed and discarded (matches
        // hardware timing, nothing observes the value). Under `introspect`
        // it is mirrored to `last_out_l`/`last_out_r`; under `audio` it is
        // pushed to the host capture ring below.
        #[cfg_attr(
            not(any(feature = "introspect", feature = "audio")),
            allow(unused_variables)
        )]
        let final_l = (voice_scaled_l + echo_scaled_l).clamp(-32768, 32767) as i16;
        #[cfg_attr(
            not(any(feature = "introspect", feature = "audio")),
            allow(unused_variables)
        )]
        let final_r = (voice_scaled_r + echo_scaled_r).clamp(-32768, 32767) as i16;

        #[cfg(feature = "introspect")]
        {
            self.last_out_l = final_l;
            self.last_out_r = final_r;
        }

        #[cfg(feature = "audio")]
        self.push_audio_pair(final_l, final_r);

        // Write echo feedback into echo buffer (if echo write not disabled).
        // D9: `clamp16(echo_in + ((fir * efb) >> 7))` — only the feedback
        // term (fir * efb) is scaled by >>7; the old code divided the whole
        // sum by 128, leaving echo nearly silent. D10: mask the LSB.
        if !echo_write_disable {
            let efb = self.efb() as i32;
            let new_echo_l =
                ((echo_in_l + ((fir_l as i32 * efb) >> 7)).clamp(-32768, 32767) as i16) & !1;
            let new_echo_r =
                ((echo_in_r + ((fir_r as i32 * efb) >> 7)).clamp(-32768, 32767) as i16) & !1;

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
        // D3: re-pinned to 262146 for the correct published S-DSP table
        // (the old pin, 290589, pinned a wrong/fabricated table whose
        // 4-tap kernels summed to 1.1-1.4x unity gain instead of ~1x).
        // Computed as: sum of all 512 i16 entries = 262146.
        // This value pins the table; any transcription error changes it.
        assert!(sum > 0, "Gaussian table sum should be positive");
        assert_eq!(
            sum, 262146,
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

    /// Test BRR shift overflow (shift > 12): invalid-shift rule (D5).
    #[test]
    fn brr_shift_overflow_saturates() {
        // shift = 13 with positive nybble → 0 (garbage shift rule).
        let result = Dsp::decode_brr_nybble(7, 13, 0, 0, 0);
        assert_eq!(result & !1, result, "must be even");
        // result should be 0 since nybble > 0 and shift > 12 → 0.
        assert_eq!(result, 0, "positive nybble with overflow shift → 0");

        // Negative nybble → -4096 (D5: the invalid-shift case is evaluated
        // in the hardware's halved domain and doubled back, not saturated
        // to -32768 — this value was updated from the old -32768 pin when
        // D5 landed; see hw_ref_decode in fidelity_tests for the reference
        // derivation).
        let result2 = Dsp::decode_brr_nybble(-8, 13, 0, 0, 0);
        assert_eq!(
            result2, -4096,
            "negative nybble with overflow shift → -4096"
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

    /// D7: BRR filter history (`old1`/`old2`) must carry across the loop
    /// seam, not reset to 0. Play a single self-looping BRR block through
    /// filter mode 1 (history-dependent) for two full iterations: since
    /// the source nybbles are identical every iteration, the *decoded*
    /// samples are identical between iterations only if `old1`/`old2`
    /// reset every loop (the old bug); with history correctly carried
    /// over, the second iteration starts from whatever `old1`/`old2`
    /// evolved to at the end of the first and so decodes differently.
    #[test]
    fn brr_loop_seam_preserves_filter_history() {
        let mut dsp = Dsp::new();
        let mut aram = [0u8; 0x10000];

        // Sample directory entry 0 at DIR=$01 -> $0100: start = loop = $0200
        // (a single block that loops on itself).
        aram[0x0100] = 0x00;
        aram[0x0101] = 0x02;
        aram[0x0102] = 0x00;
        aram[0x0103] = 0x02;

        // BRR block at $0200: shift=8, filter=1 (history-dependent), loop
        // + end bits set. Data: varied nonzero nybbles.
        aram[0x0200] = (8 << 4) | (1 << 2) | 0b11;
        let data: [u8; 8] = [0x71, 0x35, 0x62, 0x14, 0x53, 0x27, 0x46, 0x18];
        aram[0x0201..0x0209].copy_from_slice(&data);

        dsp.write_reg(0x00, 0x7F); // VOL_L
        dsp.write_reg(0x01, 0x7F); // VOL_R
        dsp.write_reg(0x02, 0x00); // PITCH lo
        dsp.write_reg(0x03, 0x10); // PITCH hi => $1000 (exactly 1 sample/tick)
        dsp.write_reg(0x04, 0x00); // SRCN 0
        dsp.write_reg(0x05, 0x00); // ADSR1: ADSR off => GAIN mode
        dsp.write_reg(0x07, 0x7F); // GAIN direct max
        dsp.write_reg(0x5D, 0x01); // DIR = $0100
        dsp.write_reg(0x6C, 0x20); // FLG: echo write disable only
        dsp.write_reg(0x4C, 0x01); // KON voice 0

        // KON delay (5 ticks, first decode on tick 5) + 16 ticks to finish
        // the first block + trigger the loop-back decode. Run generously
        // past that so both 16-sample blocks are fully in `buf`.
        for _ in 0..40 {
            dsp.step_sample(&mut aram);
        }

        // FIX4: three decodes occur by tick 40, not exactly two — the
        // initial decode plus two loop-back decodes (each 16-sample block
        // is consumed one sample per tick, and this single-block loop
        // triggers a fresh decode every 16 ticks after the KON delay).
        // buf_pos wraps every 16-sample write into the 32-entry ring, so
        // by the third decode buf[0..16) holds the *third* iteration
        // (having wrapped back around and overwritten the first), while
        // buf[16..32) still holds the second. The `first`/`second` names
        // below are positional (which half of the ring), not iteration
        // ordinals; the assertion only needs consecutive iterations to
        // differ, which still holds.
        let voice = &dsp.voices[0];
        let first: Vec<i16> = voice.buf[0..16].to_vec();
        let second: Vec<i16> = voice.buf[16..32].to_vec();

        assert_ne!(
            first, second,
            "second loop iteration must decode differently from the first \
             (filter history carried across the loop seam), got {first:?} == {second:?}"
        );
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

    /// D8: decay rate (DR) is ADSR1 bits 6:4. Two ADSR1 bytes that share
    /// bits 6:4 but differ in the attack-rate field (bits 3:0) must decay
    /// identically; the old `(adsr1 >> 3) & 0x07` shift mixed the low
    /// attack-rate bit into the decay rate, so this would previously have
    /// diverged.
    #[test]
    fn adsr_decay_rate_uses_bits_6_4_not_5_3() {
        let mut voice_a = Voice::new();
        voice_a.env_phase = EnvPhase::Decay;
        voice_a.envelope = 0x7FF;
        let adsr1_a = 0b0101_0000u8; // DR (bits 6:4) = 0b101 = 5, AR = 0b0000

        let mut voice_b = Voice::new();
        voice_b.env_phase = EnvPhase::Decay;
        voice_b.envelope = 0x7FF;
        let adsr1_b = 0b0101_1111u8; // same DR = 5, AR = 0b1111 (differs)

        let adsr2 = 0x00; // SL = 0 -> low sustain level, decay keeps stepping

        for _ in 0..2000 {
            Dsp::step_envelope(&mut voice_a, adsr1_a, adsr2, 0);
            Dsp::step_envelope(&mut voice_b, adsr1_b, adsr2, 0);
        }

        assert_eq!(
            voice_a.envelope, voice_b.envelope,
            "decay must depend only on ADSR1 bits 6:4, not the attack-rate bits 3:0"
        );
        assert_eq!(voice_a.env_phase, voice_b.env_phase);
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

    /// D9: the echo buffer write is `clamp16(echo_in + ((fir * efb) >> 7))`
    /// — only the feedback term is divided by 128, not the whole sum. The
    /// old `(echo_in + fir*efb) >> 7` made echo ~128x too quiet. Drive a
    /// loud voice with echo routing enabled (EFB = 0 to isolate the
    /// `echo_in` term) and check the echo buffer byte written into ARAM
    /// is loud, not attenuated by ~1/128.
    #[test]
    fn echo_level_is_not_attenuated_by_128() {
        let mut dsp = Dsp::new();
        let mut aram = [0u8; 0x10000];

        // Sample directory entry 0 at DIR=$01 -> $0100: start = loop = $0200
        // (a single self-looping block, filter 0, max positive amplitude).
        aram[0x0100] = 0x00;
        aram[0x0101] = 0x02;
        aram[0x0102] = 0x00;
        aram[0x0103] = 0x02;
        aram[0x0200] = (12 << 4) | 0b11; // shift=12, filter=0, loop+end
        for i in 0..8 {
            aram[0x0201 + i] = 0x77; // both nybbles = +7 (max positive)
        }

        dsp.write_reg(0x00, 0x7F); // VOL_L
        dsp.write_reg(0x01, 0x7F); // VOL_R
        dsp.write_reg(0x02, 0x00); // PITCH lo
        dsp.write_reg(0x03, 0x10); // PITCH hi => $1000
        dsp.write_reg(0x04, 0x00); // SRCN 0
        dsp.write_reg(0x05, 0x00); // ADSR1: ADSR off => GAIN mode
        dsp.write_reg(0x07, 0x7F); // GAIN direct max
        dsp.write_reg(0x5D, 0x01); // DIR = $0100
        dsp.write_reg(0x4D, 0x01); // EON: route voice 0 to echo
        dsp.write_reg(0x6D, 0x00); // ESA = 0 -> echo base $0000
        dsp.write_reg(0x7D, 0x01); // EDL = 1 -> echo buffer usable
        dsp.write_reg(0x0D, 0x00); // EFB = 0 (isolate the echo_in term)
        dsp.write_reg(0x6C, 0x00); // FLG: not muted, echo write enabled
        dsp.write_reg(0x4C, 0x01); // KON voice 0

        // Warm up well past the KON delay and BRR/gaussian ring fill so
        // the voice is at full, steady-state amplitude, then check the
        // echo write from the very next tick.
        for _ in 0..80 {
            dsp.step_sample(&mut aram);
        }
        let ep_before = dsp.echo_pos as usize;
        dsp.step_sample(&mut aram);

        let echo_base = (dsp.esa() as usize) << 8;
        let aram_off_l = (echo_base + ep_before * 4) & 0xFFFF;
        let written = i16::from_le_bytes([aram[aram_off_l], aram[(aram_off_l + 1) & 0xFFFF]]);

        // Correct: echo_in alone (EFB=0), on the order of tens of
        // thousands for a full-amplitude voice. Buggy (>>7 of the whole
        // sum): would land in the low hundreds. 5000 comfortably
        // separates the two.
        assert!(
            written.unsigned_abs() as i32 > 5000,
            "echo buffer write should be loud (not attenuated ~128x), got {written}"
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

    /// D6: the LFSR must step with the documented hardware form —
    /// `new = (lfsr >> 1) | (((lfsr ^ (lfsr >> 1)) & 1) << 14)` — a
    /// right-shift LFSR with feedback into bit 14, not the old left-shift
    /// form. Cross-check the first several steps against a hand-stepped
    /// reference computed directly from that formula, starting from the
    /// documented power-on state (0x4000).
    #[test]
    fn noise_lfsr_matches_hand_stepped_reference() {
        fn reference_step(lfsr: u16) -> u16 {
            let feedback = ((lfsr ^ (lfsr >> 1)) & 1) << 14;
            (lfsr >> 1) | feedback
        }

        let mut dsp = Dsp::new();
        assert_eq!(dsp.noise_lfsr, 0x4000, "power-on LFSR state");

        let mut reference = 0x4000u16;
        for step in 0..16 {
            dsp.step_noise();
            reference = reference_step(reference);
            assert_eq!(
                dsp.noise_lfsr, reference,
                "LFSR diverged from hand-stepped reference at step {step}"
            );
        }
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

    // ---- Audio capture ring (feature "audio" only) ----

    #[cfg(feature = "audio")]
    mod audio_ring_tests {
        use super::*;

        #[test]
        fn fill_drain_round_trip() {
            let mut dsp = Dsp::new();
            for i in 0..10i16 {
                dsp.push_audio_pair(i, -i);
            }
            let mut out = [0i16; 20];
            let n = dsp.drain_audio(&mut out);
            assert_eq!(n, 20, "10 pairs drained should write 20 i16 values");
            for i in 0..10i16 {
                assert_eq!(out[(i as usize) * 2], i, "L sample {i} mismatch");
                assert_eq!(out[(i as usize) * 2 + 1], -i, "R sample {i} mismatch");
            }
            // Ring is now empty.
            let mut out2 = [0i16; 4];
            assert_eq!(dsp.drain_audio(&mut out2), 0);
        }

        #[test]
        fn interleaving_is_l_then_r() {
            let mut dsp = Dsp::new();
            dsp.push_audio_pair(111, 222);
            let mut out = [0i16; 2];
            assert_eq!(dsp.drain_audio(&mut out), 2);
            assert_eq!(out, [111, 222], "expected L then R interleaving");
        }

        #[test]
        fn partial_drain_leaves_remainder_queued() {
            let mut dsp = Dsp::new();
            for i in 0..6i16 {
                dsp.push_audio_pair(i, i);
            }
            let mut out = [0i16; 4]; // room for 2 pairs only
            let n = dsp.drain_audio(&mut out);
            assert_eq!(n, 4);
            assert_eq!(out, [0, 0, 1, 1]);

            // Remaining 4 pairs (2,3,4,5) still queued.
            let mut out2 = [0i16; 8];
            let n2 = dsp.drain_audio(&mut out2);
            assert_eq!(n2, 8);
            assert_eq!(out2, [2, 2, 3, 3, 4, 4, 5, 5]);
        }

        #[test]
        fn overflow_overwrites_oldest_and_counts_drops() {
            let mut dsp = Dsp::new();
            // Push one more pair than capacity: the oldest (pair 0) is
            // overwritten and dropped.
            for i in 0..(AUDIO_RING_CAP as i32 + 1) {
                dsp.push_audio_pair(i as i16, i as i16);
            }
            assert_eq!(dsp.audio_dropped_pairs(), 1, "exactly one pair dropped");

            let mut out = [0i16; 2];
            assert_eq!(dsp.drain_audio(&mut out), 2);
            // Oldest surviving pair is index 1 (index 0 was overwritten).
            assert_eq!(out, [1, 1], "oldest pair should have been dropped");
        }

        #[test]
        fn odd_length_out_slice_writes_even_count() {
            let mut dsp = Dsp::new();
            for i in 0..5i16 {
                dsp.push_audio_pair(i, i);
            }
            let mut out = [0i16; 5]; // odd length: room for 2 pairs, 1 spare i16
            let n = dsp.drain_audio(&mut out);
            assert_eq!(n, 4, "odd-length out slice must still write an even count");
            assert_eq!(n % 2, 0);
        }
    }

    // ---- S-DSP fidelity audit (audio-path correctness vs real hardware) ----
    //
    // These tests assert *hardware-documented* S-DSP behaviour (fullsnes /
    // anomie's dsp.txt; algorithms cross-checked against hardware-verified
    // reference decoders). They were added while auditing the "loud music
    // sounds crunchy" symptom. A failing test here documents a fidelity
    // divergence in the production code above — the fix belongs in the DSP,
    // not in the test.
    mod fidelity_tests {
        use super::*;

        /// FIX2: pinned hardware-verified BRR decode vectors.
        ///
        /// Provenance: generated by a Python transliteration of blargg's
        /// `SPC_DSP::decode_brr` (snes9x `apu/bapu/dsp/SPC_DSP.cpp`,
        /// fetched from
        /// `https://raw.githubusercontent.com/snes9xgit/snes9x/master/apu/bapu/dsp/SPC_DSP.cpp`
        /// on 2026-07-16), NOT from this crate's own production code —
        /// the previous version of this test used a same-shaped
        /// `hw_ref_decode` reference function that reimplemented the
        /// production formulas and therefore could never disagree with a
        /// bug in `decode_brr_nybble` (a reviewer flagged the circularity:
        /// e.g. filter 2, nybble 0, shift 0, old1=old2=-100 must decode to
        /// exactly -100 on real hardware, which the old circular test could
        /// not catch). These 60 vectors are pinned constants, compared
        /// against production with **zero tolerance**.
        ///
        /// Coverage: all 4 filter modes x shifts {0, 5, 12, 13, 15}
        /// (0/5/12 = valid shift range boundaries and mid-range; 13/15 =
        /// the invalid-shift saturation case) x 3 history/nybble combos
        /// ("typical" — small in-range magnitudes; "divergent" — the
        /// reviewer-found case above; "wrap-boundary" — history near
        /// +/-32000 with the most-negative nybble, exercising the 16-bit
        /// wrap-not-saturate doubling step).
        ///
        /// Regeneration: see
        /// `scripts/gen_brr_vectors.py`-equivalent logic in the scratch
        /// generator used to produce this table (not checked into the
        /// repo): it clamps the halved-domain accumulator to i16 range
        /// then doubles with 16-bit wraparound, matching `CLAMP16(s); s =
        /// (int16_t)(s * 2);` in the C++ source exactly. Regenerate by
        /// re-fetching SPC_DSP.cpp and re-running the same transliteration
        /// if this table is ever suspected of drifting from hardware.
        ///
        /// Columns: (filter, shift, nybble, old1, old2, expected).
        #[rustfmt::skip]
        const BRR_HW_VECTORS: &[(u8, u8, i32, i16, i16, i16)] = &[
            (0, 0, 3, 100, 50, 2), (0, 0, 0, -100, -100, 0), (0, 0, -8, -32000, 32000, -8),
            (0, 5, 3, 100, 50, 96), (0, 5, 0, -100, -100, 0), (0, 5, -8, -32000, 32000, -256),
            (0, 12, 3, 100, 50, 12288), (0, 12, 0, -100, -100, 0), (0, 12, -8, -32000, 32000, -32768),
            (0, 13, 3, 100, 50, 0), (0, 13, 0, -100, -100, 0), (0, 13, -8, -32000, 32000, -4096),
            (0, 15, 3, 100, 50, 0), (0, 15, 0, -100, -100, 0), (0, 15, -8, -32000, 32000, -4096),
            (1, 0, 3, 100, 50, 94), (1, 0, 0, -100, -100, -94), (1, 0, -8, -32000, 32000, -30008),
            (1, 5, 3, 100, 50, 188), (1, 5, 0, -100, -100, -94), (1, 5, -8, -32000, 32000, -30256),
            (1, 12, 3, 100, 50, 12380), (1, 12, 0, -100, -100, -94), (1, 12, -8, -32000, 32000, 2768),
            (1, 13, 3, 100, 50, 92), (1, 13, 0, -100, -100, -94), (1, 13, -8, -32000, 32000, 31440),
            (1, 15, 3, 100, 50, 92), (1, 15, 0, -100, -100, -94), (1, 15, -8, -32000, 32000, 31440),
            (2, 0, 3, 100, 50, 144), (2, 0, 0, -100, -100, -100), (2, 0, -8, -32000, 32000, 0),
            (2, 5, 3, 100, 50, 238), (2, 5, 0, -100, -100, -100), (2, 5, -8, -32000, 32000, 0),
            (2, 12, 3, 100, 50, 12430), (2, 12, 0, -100, -100, -100), (2, 12, -8, -32000, 32000, 0),
            (2, 13, 3, 100, 50, 142), (2, 13, 0, -100, -100, -100), (2, 13, -8, -32000, 32000, 0),
            (2, 15, 3, 100, 50, 142), (2, 15, 0, -100, -100, -100), (2, 15, -8, -32000, 32000, 0),
            (3, 0, 3, 100, 50, 138), (3, 0, 0, -100, -100, -100), (3, 0, -8, -32000, 32000, 0),
            (3, 5, 3, 100, 50, 232), (3, 5, 0, -100, -100, -100), (3, 5, -8, -32000, 32000, 0),
            (3, 12, 3, 100, 50, 12424), (3, 12, 0, -100, -100, -100), (3, 12, -8, -32000, 32000, 0),
            (3, 13, 3, 100, 50, 136), (3, 13, 0, -100, -100, -100), (3, 13, -8, -32000, 32000, 0),
            (3, 15, 3, 100, 50, 136), (3, 15, 0, -100, -100, -100), (3, 15, -8, -32000, 32000, 0),
        ];

        /// The production BRR decoder must match every pinned hardware
        /// vector EXACTLY (no tolerance) — see `BRR_HW_VECTORS` doc comment
        /// for provenance and coverage.
        #[test]
        fn fidelity_brr_filter_coefficients_match_hardware() {
            for &(filter, shift, nyb, old1, old2, expected) in BRR_HW_VECTORS {
                let got = Dsp::decode_brr_nybble(nyb, shift, filter, old1, old2);
                assert_eq!(
                    got, expected,
                    "BRR filter {filter} diverges from pinned hardware vector at \
                     (nybble={nyb}, shift={shift}, old1={old1}, old2={old2}): \
                     expected={expected}, got={got}"
                );
            }
        }

        /// Gaussian coefficient assignment. Hardware tap weights (oldest s0
        /// .. newest s3), frac = i:
        ///   s0: GAUSS[255-i]   s1: GAUSS[511-i]
        ///   s2: GAUSS[256+i]   s3: GAUSS[i]
        /// so at i = 0 the newest sample (s3) has weight GAUSS[0] = 0, and
        /// the *dominant* tap is s1 (two samples behind newest) with weight
        /// GAUSS[511] = 1305 — the table's near-maximum. (An earlier
        /// version of this test targeted s2, one behind newest, expecting
        /// it to carry the near-max weight; per the verified formula above
        /// s2's weight at i=0 is GAUSS[256] = 374, a secondary lobe, not
        /// the dominant tap — corrected here to target s1, matching the
        /// hardware reference this test's own doc comment states.)
        #[test]
        fn fidelity_gauss_tap_coefficient_assignment() {
            // One BRR block decoded (buf_pos = 16), playback at its last
            // sample (block_offset = 15) => newest tap (s3) lives at index
            // 15, with s2/s1/s0 at 14/13/12.
            let mut newest = [0i16; BRR_BUF_LEN];
            newest[15] = 16000;
            let newest_only = Dsp::gaussian_interp(&newest, 16, 15, 0);
            assert!(
                newest_only.abs() <= 16,
                "newest tap must have ~zero weight at frac=0 (GAUSS[0]=0), got {newest_only}"
            );

            let mut dominant = [0i16; BRR_BUF_LEN];
            dominant[13] = 16000;
            let dominant_only = Dsp::gaussian_interp(&dominant, 16, 15, 0);
            assert!(
                dominant_only > 4000,
                "the tap two samples behind newest (s1) must carry the dominant weight \
                 GAUSS[511] at frac=0, got {dominant_only}"
            );
        }

        /// The published hardware GAUSS table starts at 0, peaks at
        /// $519 = 1305, and every 4-tap kernel
        ///   GAUSS[255-i] + GAUSS[511-i] + GAUSS[256+i] + GAUSS[i]
        /// sums to ~2048 (matching the >>11 normalization; a few indices
        /// overflow to 2049, the documented hardware overflow quirk).
        #[test]
        fn fidelity_gauss_table_matches_published_values() {
            assert_eq!(GAUSS[0], 0);
            assert_eq!(
                GAUSS[511], 1305,
                "published table max is $519 = 1305, got {}",
                GAUSS[511]
            );
            for i in 0..256usize {
                let k = GAUSS[255 - i] as i32
                    + GAUSS[511 - i] as i32
                    + GAUSS[256 + i] as i32
                    + GAUSS[i] as i32;
                assert!(
                    (2032..=2064).contains(&k),
                    "4-tap kernel at frac={i} sums to {k}, expected ~2048 \
                     (unity gain through the >>11 normalization)"
                );
            }
        }

        /// End-to-end smoothness: a filter-0 BRR triangle wave (PCM step of
        /// 256/sample) played at pitch $1000 through a fixed max GAIN
        /// envelope must come out of the Gaussian interpolator *smooth* —
        /// the 4-tap kernel is a low-pass with ~unity gain, so consecutive
        /// output deltas are bounded by a few hundred. Large jumps mean the
        /// interpolation is discontinuous (the "crunchy audio" defect).
        #[cfg(feature = "audio")]
        #[test]
        fn fidelity_pure_tone_renders_smoothly() {
            let mut dsp = Dsp::new();
            let mut aram = [0u8; 0x10000];

            // Sample directory entry 0 at DIR=$01 -> $0100: start = $0200,
            // loop = $0200.
            aram[0x0100] = 0x00;
            aram[0x0101] = 0x02;
            aram[0x0102] = 0x00;
            aram[0x0103] = 0x02;

            // Two BRR blocks at $0200 encoding a 32-sample triangle,
            // filter 0 (raw PCM), shift 8 => sample = nybble * 256.
            let tri_up: [i8; 16] = [-8, -7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7];
            let tri_dn: [i8; 16] = [7, 6, 5, 4, 3, 2, 1, 0, -1, -2, -3, -4, -5, -6, -7, -8];
            let write_block =
                |aram: &mut [u8; 0x10000], addr: usize, header: u8, nyb: &[i8; 16]| {
                    aram[addr] = header;
                    for i in 0..8 {
                        let hi = (nyb[i * 2] as u8) & 0xF;
                        let lo = (nyb[i * 2 + 1] as u8) & 0xF;
                        aram[addr + 1 + i] = (hi << 4) | lo;
                    }
                };
            write_block(&mut aram, 0x0200, 0x80, &tri_up); // shift=8, filter=0
            write_block(&mut aram, 0x0209, 0x83, &tri_dn); // + LOOP + END

            // Voice 0: full volume, pitch $1000 (1:1 rate), GAIN direct max,
            // no noise / echo / pmon. FLG $20 disables echo buffer writes.
            dsp.write_reg(0x00, 0x7F); // VOL_L
            dsp.write_reg(0x01, 0x7F); // VOL_R
            dsp.write_reg(0x02, 0x00); // PITCH lo
            dsp.write_reg(0x03, 0x10); // PITCH hi => $1000
            dsp.write_reg(0x04, 0x00); // SRCN 0
            dsp.write_reg(0x05, 0x00); // ADSR1: ADSR off => GAIN mode
            dsp.write_reg(0x07, 0x7F); // GAIN direct max => env = $7F0
            dsp.write_reg(0x0C, 0x7F); // MVOL_L
            dsp.write_reg(0x1C, 0x7F); // MVOL_R
            dsp.write_reg(0x5D, 0x01); // DIR = $0100
            dsp.write_reg(0x6C, 0x20); // FLG: echo write disable only
            dsp.write_reg(0x4C, 0x01); // KON voice 0

            let mut left = Vec::with_capacity(2048);
            let mut frame = [0i16; 2];
            for _ in 0..2048 {
                dsp.step_sample(&mut aram);
                let n = dsp.drain_audio(&mut frame);
                assert_eq!(n, 2, "expected exactly one stereo pair per step");
                left.push(frame[0]);
            }

            // Skip warmup (KON delay + first loop pass); analyze steady state.
            let body = &left[64..64 + 1024];

            // (i) Non-zero output.
            let peak = body.iter().map(|s| (*s as i32).abs()).max().unwrap();
            assert!(peak > 500, "voice should be audible, peak = {peak}");

            // (iii) 32-sample periodicity (sanity: the loop is playing).
            for i in 0..(1024 - 32) {
                assert_eq!(
                    body[i],
                    body[i + 32],
                    "output not 32-periodic at body index {i}"
                );
            }

            // (ii) Smoothness. Source PCM step is 256/sample; the Gaussian
            // kernel has ~unity gain, and envelope/volume scale by ~0.976
            // combined, so hardware-correct consecutive deltas stay in the
            // low hundreds. 600 is a generous bound; beyond it the
            // interpolator is emitting discontinuities.
            let max_delta = body
                .windows(2)
                .map(|w| (w[1] as i32 - w[0] as i32).abs())
                .max()
                .unwrap();
            assert!(
                max_delta <= 600,
                "gaussian-interpolated triangle must be smooth: max |delta| = {max_delta}, \
                 peak = {peak}, first 40 steady-state samples = {:?}",
                &body[..40]
            );
        }
    }
}
