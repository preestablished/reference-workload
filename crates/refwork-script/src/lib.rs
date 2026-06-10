//! `refwork-script` — the `.padlog` scripted-input format (FORMAT.md).
//!
//! One u16 pad word per frame, platform bit order (API.md §3.4: bit 0..11 =
//! A B X Y L R Up Down Left Right Start Select; bits 12–15 zero). Shared by
//! `ramdiff` (recording) and `refwork-verify` (replay) so the two tools can
//! never drift on the format.

#![forbid(unsafe_code)]

/// Pad words may only use bits 0..=11 (API.md §3.4).
pub const PAD_MASK: u16 = 0x0FFF;

/// A parsed input log: optional advisory ROM hash + one pad word per frame.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PadLog {
    /// BLAKE3 of the ROM the script was recorded against (advisory:
    /// consumers may warn on mismatch but must not refuse to run).
    pub rom_blake3: Option<[u8; 32]>,
    /// Pad word for frame N at index N. Frame 0 is the first
    /// `run_one_frame` after `Core::new`.
    pub frames: Vec<u16>,
}

/// Parse/validation errors, with 1-based line numbers where applicable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadLogError {
    /// First significant line is not a `padlog v1 …` header.
    BadHeader { line: usize },
    /// Header carries a version other than `v1`.
    UnsupportedVersion { line: usize, version: String },
    /// `rom=` value is not 64 hex characters.
    BadRomHash { line: usize },
    /// A frame line failed to parse.
    BadFrameLine { line: usize, text: String },
    /// A pad word sets one of the reserved bits 12–15.
    ReservedBitsSet { line: usize, word: u16 },
    /// A run-length count of zero.
    ZeroRun { line: usize },
    /// A pad word outside `PAD_MASK` passed to [`PadLog::from_frames`].
    ReservedBitsInFrames { index: usize, word: u16 },
}

impl std::fmt::Display for PadLogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PadLogError::BadHeader { line } => {
                write!(f, "line {}: expected `padlog v1` header", line)
            }
            PadLogError::UnsupportedVersion { line, version } => {
                write!(f, "line {}: unsupported padlog version `{}`", line, version)
            }
            PadLogError::BadRomHash { line } => {
                write!(f, "line {}: rom= value must be 64 hex characters", line)
            }
            PadLogError::BadFrameLine { line, text } => {
                write!(f, "line {}: cannot parse frame line `{}`", line, text)
            }
            PadLogError::ReservedBitsSet { line, word } => {
                write!(
                    f,
                    "line {}: pad word {:#06x} sets reserved bits 12-15",
                    line, word
                )
            }
            PadLogError::ZeroRun { line } => {
                write!(f, "line {}: run-length count must be >= 1", line)
            }
            PadLogError::ReservedBitsInFrames { index, word } => {
                write!(
                    f,
                    "frame {}: pad word {:#06x} sets reserved bits 12-15",
                    index, word
                )
            }
        }
    }
}

impl std::error::Error for PadLogError {}

impl PadLog {
    /// Build a log from raw frames, validating the reserved-bit invariant.
    pub fn from_frames(frames: Vec<u16>) -> Result<PadLog, PadLogError> {
        if let Some((index, &word)) = frames.iter().enumerate().find(|(_, &w)| w & !PAD_MASK != 0) {
            return Err(PadLogError::ReservedBitsInFrames { index, word });
        }
        Ok(PadLog {
            rom_blake3: None,
            frames,
        })
    }

    /// Number of frames in the script.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// True when the script holds no frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Parse `.padlog` text (FORMAT.md grammar).
pub fn parse(text: &str) -> Result<PadLog, PadLogError> {
    let mut log = PadLog::default();
    let mut header_seen = false;

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        // Strip comment, then surrounding whitespace.
        let line = match raw.find('#') {
            Some(pos) => &raw[..pos],
            None => raw,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            parse_header(line, line_no, &mut log)?;
            header_seen = true;
            continue;
        }

        let (count, word) = parse_frame_line(line, line_no)?;
        if word & !PAD_MASK != 0 {
            return Err(PadLogError::ReservedBitsSet {
                line: line_no,
                word,
            });
        }
        log.frames
            .extend(std::iter::repeat_n(word, count as usize));
    }

    if !header_seen {
        return Err(PadLogError::BadHeader {
            line: text.lines().count() + 1,
        });
    }
    Ok(log)
}

fn parse_header(line: &str, line_no: usize, log: &mut PadLog) -> Result<(), PadLogError> {
    let mut parts = line.split_whitespace();
    if parts.next() != Some("padlog") {
        return Err(PadLogError::BadHeader { line: line_no });
    }
    match parts.next() {
        Some("v1") => {}
        Some(v) => {
            return Err(PadLogError::UnsupportedVersion {
                line: line_no,
                version: v.to_string(),
            })
        }
        None => return Err(PadLogError::BadHeader { line: line_no }),
    }
    for field in parts {
        if let Some(hex) = field.strip_prefix("rom=") {
            log.rom_blake3 =
                Some(parse_hex32(hex).ok_or(PadLogError::BadRomHash { line: line_no })?);
        } else {
            return Err(PadLogError::BadHeader { line: line_no });
        }
    }
    Ok(())
}

/// `HHHH` or `NxHHHH` (N decimal ≥ 1, word exactly 4 hex digits).
fn parse_frame_line(line: &str, line_no: usize) -> Result<(u64, u16), PadLogError> {
    let bad = || PadLogError::BadFrameLine {
        line: line_no,
        text: line.to_string(),
    };
    let (count_str, word_str) = match line.split_once(['x', 'X']) {
        Some((n, w)) => (Some(n), w),
        None => (None, line),
    };
    if word_str.len() != 4 || !word_str.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let word = u16::from_str_radix(word_str, 16).map_err(|_| bad())?;
    let count = match count_str {
        Some(n) => {
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            let c: u64 = n.parse().map_err(|_| bad())?;
            if c == 0 {
                return Err(PadLogError::ZeroRun { line: line_no });
            }
            c
        }
        None => 1,
    };
    Ok((count, word))
}

fn parse_hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        out[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(out)
}

/// Write a log in canonical form (FORMAT.md): lowercase hex, run-length
/// lines for runs > 1, `rom=` only when known, one trailing newline.
pub fn write(log: &PadLog) -> String {
    let mut out = String::from("padlog v1");
    if let Some(hash) = &log.rom_blake3 {
        out.push_str(" rom=");
        for b in hash {
            out.push_str(&format!("{:02x}", b));
        }
    }
    out.push('\n');

    let mut i = 0;
    while i < log.frames.len() {
        let word = log.frames[i];
        let mut run = 1usize;
        while i + run < log.frames.len() && log.frames[i + run] == word {
            run += 1;
        }
        if run > 1 {
            out.push_str(&format!("{}x{:04x}\n", run, word));
        } else {
            out.push_str(&format!("{:04x}\n", word));
        }
        i += run;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_canonical() {
        let log = PadLog {
            rom_blake3: Some([0xAB; 32]),
            frames: vec![0, 0, 0, 0x0400, 0x0400, 0x0001, 0, 0, 0],
        };
        let text = write(&log);
        assert_eq!(parse(&text).unwrap(), log);
        // write(parse(x)) canonicalizes: parsing the canonical text and
        // re-writing reproduces it byte-for-byte.
        assert_eq!(write(&parse(&text).unwrap()), text);
    }

    #[test]
    fn canonicalizes_messy_input() {
        let text = "  # leading comment\n\npadlog v1\n0000  # hold\n1x0000\n2X0400\n";
        let log = parse(text).unwrap();
        assert_eq!(log.frames, vec![0, 0, 0x0400, 0x0400]);
        assert_eq!(write(&log), "padlog v1\n2x0000\n2x0400\n");
    }

    #[test]
    fn single_frame_runs_stay_single() {
        let log = PadLog::from_frames(vec![0x0001, 0x0002]).unwrap();
        assert_eq!(write(&log), "padlog v1\n0001\n0002\n");
    }

    #[test]
    fn rle_edge_long_run() {
        let log = PadLog::from_frames(vec![0x0040; 100_000]).unwrap();
        let text = write(&log);
        assert_eq!(text, "padlog v1\n100000x0040\n");
        assert_eq!(parse(&text).unwrap(), log);
    }

    #[test]
    fn empty_log_is_header_only() {
        let log = PadLog::default();
        let text = write(&log);
        assert_eq!(text, "padlog v1\n");
        assert_eq!(parse(&text).unwrap(), log);
    }

    #[test]
    fn rom_hash_round_trips() {
        let mut hash = [0u8; 32];
        for (i, b) in hash.iter_mut().enumerate() {
            *b = i as u8;
        }
        let log = PadLog {
            rom_blake3: Some(hash),
            frames: vec![0],
        };
        assert_eq!(parse(&write(&log)).unwrap(), log);
    }

    #[test]
    fn rejects_missing_header() {
        assert!(matches!(
            parse("0000\n"),
            Err(PadLogError::BadFrameLine { .. }) | Err(PadLogError::BadHeader { .. })
        ));
        assert!(matches!(parse(""), Err(PadLogError::BadHeader { .. })));
    }

    #[test]
    fn rejects_wrong_version() {
        assert_eq!(
            parse("padlog v2\n"),
            Err(PadLogError::UnsupportedVersion {
                line: 1,
                version: "v2".into()
            })
        );
    }

    #[test]
    fn rejects_reserved_bits() {
        assert_eq!(
            parse("padlog v1\nf000\n"),
            Err(PadLogError::ReservedBitsSet {
                line: 2,
                word: 0xF000
            })
        );
        assert_eq!(
            PadLog::from_frames(vec![0x1000]),
            Err(PadLogError::ReservedBitsInFrames {
                index: 0,
                word: 0x1000
            })
        );
    }

    #[test]
    fn rejects_zero_run_and_garbage() {
        assert_eq!(
            parse("padlog v1\n0x0000\n"),
            Err(PadLogError::ZeroRun { line: 2 })
        );
        assert!(matches!(
            parse("padlog v1\n00\n"),
            Err(PadLogError::BadFrameLine { .. })
        ));
        assert!(matches!(
            parse("padlog v1\nx0000\n"),
            Err(PadLogError::BadFrameLine { .. })
        ));
        assert!(matches!(
            parse("padlog v1\n3y0000\n"),
            Err(PadLogError::BadFrameLine { .. })
        ));
        assert!(matches!(
            parse("padlog v1 bogus=1\n"),
            Err(PadLogError::BadHeader { .. })
        ));
        assert!(matches!(
            parse("padlog v1 rom=zz\n"),
            Err(PadLogError::BadRomHash { .. })
        ));
    }

    #[test]
    fn comments_and_blank_lines_anywhere() {
        let text = "# top\npadlog v1 # trailing\n# mid\n2x0001 # run\n\n0002\n";
        let log = parse(text).unwrap();
        assert_eq!(log.frames, vec![1, 1, 2]);
    }
}
