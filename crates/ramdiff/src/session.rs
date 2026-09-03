//! Session model: persisted state for an ongoing `ramdiff` investigation.
//!
//! A session is a directory on disk containing:
//! - `session.yaml` — index of WRAM dumps and surviving candidate offsets
//! - `<label>.bin` — raw 128 KiB WRAM snapshots (one per dump mark)
//!
//! # Platform-captured dumps
//!
//! Any 128 KiB raw `.bin` file placed in the session directory can be
//! registered as a dump by adding a manual `session.yaml` entry:
//!
//! ```yaml
//! dumps:
//!   - label: "capture-a"
//!     frame: 0
//!     file: "capture-a.bin"
//!     region: "wram"
//! ```
//!
//! The `frame` field is informational when the dump comes from an external
//! capture tool; `0` is a valid sentinel. The format contract is simply
//! "raw region bytes, exactly 128 KiB."

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Number of bytes in WRAM (128 KiB).
pub const WRAM_SIZE: usize = 0x20000;

/// Width of a candidate search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SearchWidth {
    #[default]
    U8,
    U16le,
}

impl SearchWidth {
    pub fn byte_size(self) -> usize {
        match self {
            SearchWidth::U8 => 1,
            SearchWidth::U16le => 2,
        }
    }

    /// Read a value from `wram` at `offset` in this width (little-endian for u16le).
    pub fn read_value(self, wram: &[u8], offset: u32) -> u32 {
        let off = offset as usize;
        match self {
            SearchWidth::U8 => wram[off] as u32,
            SearchWidth::U16le => {
                let lo = wram[off] as u32;
                let hi = wram[off + 1] as u32;
                lo | (hi << 8)
            }
        }
    }
}

impl std::str::FromStr for SearchWidth {
    type Err = String;

    /// Parse from the CLI string `"u8"` or `"u16le"`.
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "u8" => Ok(SearchWidth::U8),
            "u16le" => Ok(SearchWidth::U16le),
            other => Err(format!("unknown width {:?}, expected u8 or u16le", other)),
        }
    }
}

impl std::fmt::Display for SearchWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchWidth::U8 => write!(f, "u8"),
            SearchWidth::U16le => write!(f, "u16le"),
        }
    }
}

/// Metadata for a single WRAM dump.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DumpMeta {
    /// Human label used to reference this dump in search commands.
    pub label: String,
    /// Frame number at which the dump was taken (informational for platform captures).
    pub frame: u64,
    /// File name (relative to session directory) of the raw `.bin` dump.
    pub file: String,
    /// Region name — currently always `"wram"`.
    pub region: String,
}

/// Surviving candidate offsets from successive search operations.
///
/// On first `search` invocation, candidates are initialized to every valid
/// offset within the region. Subsequent searches intersect with the current
/// set. The set is stored as a sorted `Vec<u32>` of byte offsets.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CandidateSet {
    /// Search width; all offsets are aligned to this width.
    pub width: SearchWidth,
    /// Sorted, deduplicated byte offsets within the region.
    pub offsets: Vec<u32>,
}

impl CandidateSet {
    /// Initialize with all valid offsets for the given region size and width.
    pub fn full(size: usize, width: SearchWidth) -> Self {
        let step = width.byte_size();
        // Offsets are 0, step, 2*step, … while off + step <= size.
        let count = if step > size {
            0
        } else {
            (size - step) / step + 1
        };
        let mut offsets = Vec::with_capacity(count);
        let mut off = 0u32;
        while (off as usize) + step <= size {
            offsets.push(off);
            off += step as u32;
        }
        CandidateSet { width, offsets }
    }

    /// Retain only offsets that satisfy `pred`. Returns number of survivors.
    pub fn retain<F>(&mut self, mut pred: F) -> usize
    where
        F: FnMut(u32) -> bool,
    {
        self.offsets.retain(|&off| pred(off));
        self.offsets.len()
    }
}

/// A complete `ramdiff` session.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Session {
    /// Path to the session directory (not serialized — set at load time).
    #[serde(skip)]
    pub dir: PathBuf,
    /// All recorded WRAM dumps, in insertion order.
    pub dumps: Vec<DumpMeta>,
    /// Number of pad frames in `interactive.padlog` at the last save, if this
    /// session was recorded interactively. Used at resume time to detect a
    /// truncated or replaced log (absent in scripted sessions and in
    /// `session.yaml` files written before this field existed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_frames: Option<u64>,
    /// `refwork_emu::EMU_VERSION` of the build that recorded this session
    /// (the determinism epoch marker). Written once, at the first save of a
    /// fresh interactive session or the first scripted `record` into an empty
    /// session; never rewritten by `--resume`. Absent in sessions recorded
    /// before the field existed — those get a warning on resume and fail
    /// `ramdiff lint` check C11.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emu_version: Option<String>,
    /// Lowercase BLAKE3 hex of the ROM image the session was recorded
    /// against. Same lifecycle as `emu_version`. The value is private
    /// (it identifies the ROM); tools compare it but never print it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rom_blake3: Option<String>,
    /// Current candidate set (may be empty before the first search).
    #[serde(default)]
    pub candidates: CandidateSet,
}

impl Session {
    /// Create a new session in `dir` (directory must exist or be created by the caller).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Session {
            dir: dir.into(),
            dumps: Vec::new(),
            log_frames: None,
            emu_version: None,
            rom_blake3: None,
            candidates: CandidateSet::default(),
        }
    }

    /// Record the emulator epoch and ROM identity this session is recorded
    /// under. Pure setter: callers decide when a stamp is legitimate (fresh
    /// sessions only — see `record::check_epoch` for the resume side).
    pub fn stamp(&mut self, emu_version: &str, rom_blake3_hex: &str) {
        self.emu_version = Some(emu_version.to_owned());
        self.rom_blake3 = Some(rom_blake3_hex.to_owned());
    }

    /// Load a session from `dir/session.yaml`, or return a fresh session if
    /// the file does not exist.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, String> {
        let dir = dir.as_ref().to_owned();
        let yaml_path = dir.join("session.yaml");
        if !yaml_path.exists() {
            return Ok(Session::new(dir));
        }
        let text = std::fs::read_to_string(&yaml_path)
            .map_err(|e| format!("cannot read {}: {}", yaml_path.display(), e))?;
        let mut s: Session = serde_yaml::from_str(&text)
            .map_err(|e| format!("cannot parse {}: {}", yaml_path.display(), e))?;
        s.dir = dir;
        Ok(s)
    }

    /// Persist `session.yaml` to `self.dir`.
    ///
    /// Written via a temp file + rename so an interrupted save can never
    /// leave a half-written `session.yaml` behind.
    pub fn save(&self) -> Result<(), String> {
        let yaml_path = self.dir.join("session.yaml");
        let tmp_path = self.dir.join("session.yaml.tmp");
        let text =
            serde_yaml::to_string(self).map_err(|e| format!("cannot serialize session: {}", e))?;
        std::fs::write(&tmp_path, text)
            .map_err(|e| format!("cannot write {}: {}", tmp_path.display(), e))?;
        std::fs::rename(&tmp_path, &yaml_path)
            .map_err(|e| format!("cannot rename {} into place: {}", tmp_path.display(), e))?;
        Ok(())
    }

    /// Find a dump by label.
    pub fn dump_by_label(&self, label: &str) -> Option<&DumpMeta> {
        self.dumps.iter().find(|d| d.label == label)
    }

    /// Load the raw WRAM bytes for a dump label.
    pub fn load_dump_bytes(&self, label: &str) -> Result<Vec<u8>, String> {
        let meta = self
            .dump_by_label(label)
            .ok_or_else(|| format!("no dump with label {:?}", label))?;
        self.load_dump_bytes_for(meta)
    }

    /// Load the raw WRAM bytes for a specific dump entry by its file path.
    ///
    /// Labels are not unique (and distinct labels can sanitize to the same
    /// file name), so callers holding a `DumpMeta` must read through it
    /// rather than going back via the label.
    pub fn load_dump_bytes_for(&self, meta: &DumpMeta) -> Result<Vec<u8>, String> {
        let path = self.dir.join(&meta.file);
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("cannot read dump {:?}: {}", path.display(), e))?;
        if bytes.len() != WRAM_SIZE {
            return Err(format!(
                "dump {:?}: expected {} bytes, got {} (must be raw 128 KiB WRAM)",
                meta.file,
                WRAM_SIZE,
                bytes.len()
            ));
        }
        Ok(bytes)
    }

    /// Write a 128 KiB WRAM dump to `self.dir/<file>`.
    pub fn write_dump(&self, file: &str, wram: &[u8; WRAM_SIZE]) -> Result<(), String> {
        let path = self.dir.join(file);
        std::fs::write(&path, wram.as_slice())
            .map_err(|e| format!("cannot write dump {}: {}", path.display(), e))?;
        Ok(())
    }

    /// Register a new dump. Does not persist; call `save()` after.
    pub fn add_dump(&mut self, meta: DumpMeta) {
        self.dumps.push(meta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three-field YAML from the module docs' era: no stamp fields.
    const LEGACY_YAML: &str = "dumps:\n- label: a\n  frame: 5\n  file: a.bin\n  region: wram\nlog_frames: 10\ncandidates:\n  width: u8\n  offsets: []\n";

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ramdiff-session-{}-{}-{}",
            tag,
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn session_yaml_without_stamp_loads_and_stays_unstamped() {
        let dir = temp_dir("legacy");
        std::fs::write(dir.join("session.yaml"), LEGACY_YAML).unwrap();
        let s = Session::load(&dir).unwrap();
        assert_eq!(s.emu_version, None);
        assert_eq!(s.rom_blake3, None);
        assert_eq!(s.log_frames, Some(10));
        s.save().unwrap();
        let text = std::fs::read_to_string(dir.join("session.yaml")).unwrap();
        assert!(!text.contains("emu_version:"), "saved: {}", text);
        assert!(!text.contains("rom_blake3:"), "saved: {}", text);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn session_stamp_round_trips() {
        let dir = temp_dir("stamp");
        let mut s = Session::new(&dir);
        let hex = "ab".repeat(32);
        s.stamp("refwork-emu 9.9.9-test", &hex);
        s.save().unwrap();
        let loaded = Session::load(&dir).unwrap();
        assert_eq!(
            loaded.emu_version.as_deref(),
            Some("refwork-emu 9.9.9-test")
        );
        assert_eq!(loaded.rom_blake3.as_deref(), Some(hex.as_str()));
        let text = std::fs::read_to_string(dir.join("session.yaml")).unwrap();
        assert!(
            text.contains("emu_version: refwork-emu 9.9.9-test"),
            "{}",
            text
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_stamp_type_is_a_parse_error() {
        let dir = temp_dir("malformed");
        std::fs::write(
            dir.join("session.yaml"),
            "dumps: []\nemu_version: [1, 2]\ncandidates:\n  width: u8\n  offsets: []\n",
        )
        .unwrap();
        let err = Session::load(&dir).unwrap_err();
        assert!(err.contains("cannot parse"), "err: {}", err);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
