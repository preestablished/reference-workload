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
        let count = if step > size {
            0
        } else {
            (size - step + 1) / step + 1
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
            candidates: CandidateSet::default(),
        }
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
    pub fn save(&self) -> Result<(), String> {
        let yaml_path = self.dir.join("session.yaml");
        let text =
            serde_yaml::to_string(self).map_err(|e| format!("cannot serialize session: {}", e))?;
        std::fs::write(&yaml_path, text)
            .map_err(|e| format!("cannot write {}: {}", yaml_path.display(), e))?;
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
