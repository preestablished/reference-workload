//! `ramdiff search` — set-algebra narrowing over the candidate offset set.
//!
//! Each invocation:
//! 1. Loads `session.yaml` from the session directory.
//! 2. If candidates are empty, initializes them to all valid offsets for the
//!    session's search width (or the default width if first invocation).
//! 3. Applies one or more filter operations, intersecting with the current set.
//! 4. Writes the updated `session.yaml` back.
//!
//! Filter operations:
//! - `--changed A B`    — offset value differs between dump A and dump B
//! - `--unchanged A B`  — offset value is the same in both dumps
//! - `--inc A B`        — value at B > value at A
//! - `--dec A B`        — value at B < value at A
//! - `--value N --in A` — value equals N in dump A
//! - `--delta D A B`    — |value_B - value_A| == D (checked in both directions;
//!   values are widened to u32 first, so no u8/u16 wrap-around is involved)
//! - `--width u8|u16le` — (re-)initialize candidate set with this width

use crate::session::{CandidateSet, SearchWidth, Session, WRAM_SIZE};

/// A single filter clause.
#[derive(Clone, Debug)]
pub enum FilterOp {
    Changed { a: String, b: String },
    Unchanged { a: String, b: String },
    Increased { a: String, b: String },
    Decreased { a: String, b: String },
    ValueIn { value: u32, label: String },
    Delta { delta: u32, a: String, b: String },
    SetWidth(SearchWidth),
}

/// Run the search command: apply `ops` to the session at `dir`.
pub fn run_search(dir: &std::path::Path, ops: &[FilterOp]) -> Result<(), String> {
    let mut session = Session::load(dir)?;

    // Determine the effective width.
    let width = ops
        .iter()
        .filter_map(|op| match op {
            FilterOp::SetWidth(w) => Some(*w),
            _ => None,
        })
        .next_back()
        .unwrap_or(session.candidates.width);

    // Initialize candidate set if empty (first search) or width changed.
    if session.candidates.offsets.is_empty() || session.candidates.width != width {
        session.candidates = CandidateSet::full(WRAM_SIZE, width);
    }

    // Apply each filter in order.
    for op in ops {
        match op {
            FilterOp::SetWidth(_) => {} // already handled above
            FilterOp::Changed { a, b } => {
                let da = session.load_dump_bytes(a)?;
                let db = session.load_dump_bytes(b)?;
                let w = session.candidates.width;
                session
                    .candidates
                    .retain(|off| w.read_value(&da, off) != w.read_value(&db, off));
            }
            FilterOp::Unchanged { a, b } => {
                let da = session.load_dump_bytes(a)?;
                let db = session.load_dump_bytes(b)?;
                let w = session.candidates.width;
                session
                    .candidates
                    .retain(|off| w.read_value(&da, off) == w.read_value(&db, off));
            }
            FilterOp::Increased { a, b } => {
                let da = session.load_dump_bytes(a)?;
                let db = session.load_dump_bytes(b)?;
                let w = session.candidates.width;
                session
                    .candidates
                    .retain(|off| w.read_value(&db, off) > w.read_value(&da, off));
            }
            FilterOp::Decreased { a, b } => {
                let da = session.load_dump_bytes(a)?;
                let db = session.load_dump_bytes(b)?;
                let w = session.candidates.width;
                session
                    .candidates
                    .retain(|off| w.read_value(&db, off) < w.read_value(&da, off));
            }
            FilterOp::ValueIn { value, label } => {
                let d = session.load_dump_bytes(label)?;
                let w = session.candidates.width;
                let target = *value;
                session
                    .candidates
                    .retain(|off| w.read_value(&d, off) == target);
            }
            FilterOp::Delta { delta, a, b } => {
                let da = session.load_dump_bytes(a)?;
                let db = session.load_dump_bytes(b)?;
                let w = session.candidates.width;
                let d = *delta;
                session.candidates.retain(|off| {
                    let va = w.read_value(&da, off);
                    let vb = w.read_value(&db, off);
                    vb.wrapping_sub(va) == d || va.wrapping_sub(vb) == d
                });
            }
        }
    }

    let survivors = session.candidates.offsets.len();
    eprintln!("search: {} candidate(s) remain", survivors);
    session.save()?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::session::{DumpMeta, Session, WRAM_SIZE};

    /// Build a synthetic WRAM dump with `values` planted at the given offsets.
    /// All other bytes are zero. Width is U8.
    pub fn make_wram_u8(plants: &[(usize, u8)]) -> Vec<u8> {
        let mut mem = vec![0u8; WRAM_SIZE];
        for &(off, val) in plants {
            mem[off] = val;
        }
        mem
    }

    /// Build a synthetic WRAM dump with u16le values planted.
    pub fn make_wram_u16le(plants: &[(usize, u16)]) -> Vec<u8> {
        let mut mem = vec![0u8; WRAM_SIZE];
        for &(off, val) in plants {
            mem[off] = (val & 0xFF) as u8;
            mem[off + 1] = ((val >> 8) & 0xFF) as u8;
        }
        mem
    }

    /// Write dumps to a temp session directory and return the session.
    pub fn make_session_dir(
        dumps: &[(&str, Vec<u8>)],
        width: SearchWidth,
    ) -> (tempfile_shim::TempDir, Session) {
        let tmp = tempfile_shim::TempDir::new();
        let dir = tmp.path.clone();
        let mut session = Session::new(&dir);
        session.candidates = CandidateSet::full(WRAM_SIZE, width);
        for (label, bytes) in dumps {
            assert_eq!(bytes.len(), WRAM_SIZE);
            let file = format!("{}.bin", label);
            std::fs::write(dir.join(&file), bytes).unwrap();
            session.add_dump(DumpMeta {
                label: label.to_string(),
                frame: 0,
                file,
                region: "wram".to_owned(),
            });
        }
        session.save().unwrap();
        (tmp, session)
    }

    #[test]
    fn filter_changed_finds_only_changed_offsets() {
        // Plant offset 0x0010 = 0x01→0x05; offset 0x0020 stays 0x00 both.
        let dump_a = make_wram_u8(&[(0x0010, 0x01)]);
        let dump_b = make_wram_u8(&[(0x0010, 0x05)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Changed {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(
            session.candidates.offsets.contains(&0x0010),
            "changed offset must survive"
        );
        assert!(
            !session.candidates.offsets.contains(&0x0020),
            "unchanged offset must be eliminated"
        );
    }

    #[test]
    fn filter_unchanged_retains_unchanged_offsets() {
        let dump_a = make_wram_u8(&[(0x0010, 0x07), (0x0020, 0x03)]);
        let dump_b = make_wram_u8(&[(0x0010, 0x07), (0x0020, 0x04)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Unchanged {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        // 0x0010 has value 7 in both → survives
        assert!(session.candidates.offsets.contains(&0x0010));
        // 0x0020 changed → eliminated
        assert!(!session.candidates.offsets.contains(&0x0020));
    }

    #[test]
    fn filter_inc_only_increasing() {
        // 0x0010: 3→7 (inc), 0x0020: 7→3 (dec), 0x0030: 5→5 (same)
        let dump_a = make_wram_u8(&[(0x0010, 3), (0x0020, 7), (0x0030, 5)]);
        let dump_b = make_wram_u8(&[(0x0010, 7), (0x0020, 3), (0x0030, 5)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Increased {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(session.candidates.offsets.contains(&0x0010));
        assert!(!session.candidates.offsets.contains(&0x0020));
        assert!(!session.candidates.offsets.contains(&0x0030));
    }

    #[test]
    fn filter_dec_only_decreasing() {
        let dump_a = make_wram_u8(&[(0x0010, 7), (0x0020, 3)]);
        let dump_b = make_wram_u8(&[(0x0010, 3), (0x0020, 7)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Decreased {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(session.candidates.offsets.contains(&0x0010));
        assert!(!session.candidates.offsets.contains(&0x0020));
    }

    #[test]
    fn filter_value_in() {
        // Plant 0x0050 = 42 in dump "a".
        let dump_a = make_wram_u8(&[(0x0050, 42), (0x0060, 10)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::ValueIn {
                value: 42,
                label: "a".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(session.candidates.offsets.contains(&0x0050));
        assert!(!session.candidates.offsets.contains(&0x0060));
    }

    #[test]
    fn filter_delta_exact() {
        // 0x0010: 10→16 (delta=6), 0x0020: 10→20 (delta=10)
        let dump_a = make_wram_u8(&[(0x0010, 10), (0x0020, 10)]);
        let dump_b = make_wram_u8(&[(0x0010, 16), (0x0020, 20)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Delta {
                delta: 6,
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(session.candidates.offsets.contains(&0x0010));
        assert!(!session.candidates.offsets.contains(&0x0020));
    }

    #[test]
    fn composition_narrows_to_plant() {
        // Two dumps: only 0x0010 changed AND increased.
        let dump_a = make_wram_u8(&[(0x0010, 1), (0x0020, 5), (0x0030, 5)]);
        let dump_b = make_wram_u8(&[(0x0010, 3), (0x0020, 5), (0x0030, 3)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U8);
        let dir = tmp.path.clone();

        // Step 1: changed
        run_search(
            &dir,
            &[FilterOp::Changed {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();
        // Step 2: increased
        run_search(
            &dir,
            &[FilterOp::Increased {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        // Only 0x0010 survived both filters.
        assert_eq!(session.candidates.offsets, vec![0x0010]);
    }

    #[test]
    fn u16le_filter_changed() {
        // Plant 0x0010 (u16le) = 0x0100→0x0200; 0x0020 stays 0x0000.
        let dump_a = make_wram_u16le(&[(0x0010, 0x0100)]);
        let dump_b = make_wram_u16le(&[(0x0010, 0x0200)]);

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U16le);
        let dir = tmp.path.clone();

        run_search(
            &dir,
            &[FilterOp::Changed {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(session.candidates.offsets.contains(&0x0010));
        assert!(!session.candidates.offsets.contains(&0x0020));
    }

    #[test]
    fn composition_u16le_narrows_to_plant() {
        // Frame counter at 0x0010: grows 5→10. No other u16le offset should match both.
        let mut dump_a = vec![0u8; WRAM_SIZE];
        let mut dump_b = vec![0u8; WRAM_SIZE];
        // Plant frame counter: 5 at dump_a, 10 at dump_b (u16le)
        dump_a[0x0010] = 5;
        dump_a[0x0011] = 0;
        dump_b[0x0010] = 10;
        dump_b[0x0011] = 0;

        let (tmp, _) = make_session_dir(&[("a", dump_a), ("b", dump_b)], SearchWidth::U16le);
        let dir = tmp.path.clone();

        // changed + increased
        run_search(
            &dir,
            &[FilterOp::Changed {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();
        run_search(
            &dir,
            &[FilterOp::Increased {
                a: "a".to_owned(),
                b: "b".to_owned(),
            }],
        )
        .unwrap();
        run_search(
            &dir,
            &[FilterOp::ValueIn {
                value: 10,
                label: "b".to_owned(),
            }],
        )
        .unwrap();

        let session = Session::load(&dir).unwrap();
        assert!(
            session.candidates.offsets.contains(&0x0010),
            "frame counter offset 0x0010 must survive; got {:?}",
            session.candidates.offsets
        );
    }
}

// ─── Tempfile shim (no external tempfile dep) ────────────────────────────────

/// Minimal temp-dir shim used in tests (no `tempfile` crate dependency).
#[cfg(test)]
pub mod tempfile_shim {
    use std::path::PathBuf;

    pub struct TempDir {
        pub path: PathBuf,
    }

    impl Default for TempDir {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TempDir {
        pub fn new() -> Self {
            // Use a deterministic unique path based on process id + counter.
            use std::sync::atomic::{AtomicU64, Ordering};
            static CTR: AtomicU64 = AtomicU64::new(0);
            let n = CTR.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("ramdiff_test_{}_{}", std::process::id(), n));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            TempDir { path: dir }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
