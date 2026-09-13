//! `ramdiff lint` — mechanical "is this recorded session complete and
//! sound?" check against a committed label checklist.
//!
//! The checklist (`tools/discovery-02-required-labels.yaml`, kind
//! `ramdiff-label-checklist`) names, per session kind, the labels a
//! recording must contain, their order, and substrings that must not
//! appear. The lint reads the session directory read-only — `session.yaml`,
//! `interactive.padlog`, dump sizes, marker files — and reports one
//! `PASS`/`FAIL`/`INFO` line per check.
//!
//! Two modes: mid-session (default) treats missing required labels as
//! information and only requires `log_frames <= padlog frames` (the padlog
//! is longer than the last save by design); `--final` requires every
//! required label and `log_frames == padlog frames`.
//!
//! Output hygiene: the report never contains a WRAM value, an offset, a
//! hash, or an absolute path — only check ids, label names, frame numbers
//! and counts. The session path is not printed; the kind name is.
//!
//! Pure logic (`lint_session`) is separated from I/O (`run_lint`) so unit
//! tests build `SessionFacts` directly.

use crate::session::{Session, WRAM_SIZE};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The checklist file's `kind` value.
pub const CHECKLIST_KIND: &str = "ramdiff-label-checklist";
/// The only `label_pattern` this implementation understands (it is the F5
/// sanitizer's kept set, so a matching label names its own dump file).
pub const SUPPORTED_LABEL_PATTERN: &str = "^[A-Za-z0-9_-]+$";
/// Marker file that declares a session kind not applicable to the game.
pub const NOT_APPLICABLE_MARKER: &str = "NOT-APPLICABLE.md";
/// Operator notes file; the `--waive` override requires the marker line
/// below inside it.
pub const SESSION_NOTES: &str = "SESSION-NOTES.md";
/// Waiver markers: each `(line, glob)` pair means "if `SESSION-NOTES.md`
/// contains `line` (trimmed, exact), then `--waive <glob>` is authorized".
/// A glob may have more than one authorizing line (either present authorizes).
/// This replaces the earlier single global flag, under which any `--waive`
/// glob was authorized by the one score-counter line regardless of fit.
pub const WAIVE_MARKERS: &[(&str, &str)] = &[
    ("score_counter: NOT-APPLICABLE", "score-*"),
    ("currency: NOT-APPLICABLE", "currency-only-*"),
    ("currency_only: NOT-AVAILABLE", "currency-only-*"),
    ("score_only: NOT-AVAILABLE", "score-only-*"),
    ("both_event: NOT-AVAILABLE", "both-*"),
];
/// The interactive input log's file name inside a session directory.
pub const PADLOG_FILE: &str = "interactive.padlog";
/// The exact header line an interactive padlog starts with.
pub const PADLOG_HEADER: &str = "padlog v1";

/// Options for `ramdiff lint`.
pub struct LintOpts {
    pub session_dir: PathBuf,
    pub checklist: PathBuf,
    /// Session kind to lint as; defaults to the directory basename with any
    /// `-take<N>` suffix stripped (see [`resolve_kind`]).
    pub kind: Option<String>,
    /// `--final`: every required label must be present and
    /// `log_frames == padlog frames`.
    pub final_: bool,
    /// `--waive <glob>` (repeatable): required labels matching a glob are
    /// reported `WAIVED` instead of missing. Each glob is honored only when
    /// `SESSION-NOTES.md` contains a line that authorizes it (see
    /// [`WAIVE_MARKERS`]).
    pub waive: Vec<String>,
}

/// The parsed checklist file.
#[derive(Debug, Clone, Deserialize)]
pub struct Checklist {
    pub kind: String,
    pub schema_version: u32,
    pub label_pattern: String,
    pub sessions: BTreeMap<String, SessionSpec>,
}

/// One session kind's requirements.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SessionSpec {
    #[serde(default)]
    pub may_be_not_applicable: bool,
    #[serde(default)]
    pub ordered: bool,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub optional: Vec<String>,
    #[serde(default)]
    pub forbidden_substrings: Vec<String>,
}

/// Outcome of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Info,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Pass => write!(f, "PASS"),
            Status::Fail => write!(f, "FAIL"),
            Status::Info => write!(f, "INFO"),
        }
    }
}

/// One rendered check line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub reason: String,
}

/// The lint result.
#[derive(Debug, Clone, Default)]
pub struct LintReport {
    pub kind: String,
    pub checks: Vec<Check>,
    /// Required labels absent from the session (waived ones excluded).
    pub missing_required: Vec<String>,
    /// Required labels absent but covered by an honored `--waive` glob.
    pub waived: Vec<String>,
    /// The session is a NOT-APPLICABLE marker and the kind allows it.
    pub not_applicable: bool,
}

impl LintReport {
    fn push(&mut self, id: &'static str, status: Status, reason: impl Into<String>) {
        self.checks.push(Check {
            id,
            status,
            reason: reason.into(),
        });
    }

    /// True when no check failed.
    pub fn passed(&self) -> bool {
        !self.checks.iter().any(|c| c.status == Status::Fail)
    }

    /// The report's last line.
    pub fn verdict(&self) -> String {
        if !self.passed() {
            let n = self
                .checks
                .iter()
                .filter(|c| c.status == Status::Fail)
                .count();
            return format!("lint: FAIL ({} checks)", n);
        }
        if self.not_applicable {
            return "lint: PASS (NOT-APPLICABLE)".to_owned();
        }
        if !self.waived.is_empty() {
            return format!("lint: PASS (WAIVED: {})", self.waived.join(", "));
        }
        "lint: PASS".to_owned()
    }

    /// Full text: kind line, one line per check, verdict last.
    pub fn render(&self) -> String {
        let mut out = format!("kind: {}\n", self.kind);
        for c in &self.checks {
            out.push_str(&format!("{:<4} {:<4} {}\n", c.id, c.status, c.reason));
        }
        out.push_str(&self.verdict());
        out.push('\n');
        out
    }
}

/// Everything the pure lint needs, gathered from disk by [`run_lint`].
#[derive(Debug, Clone)]
pub struct SessionFacts {
    /// `Ok(Some(session))` when `session.yaml` parses, `Ok(None)` when it
    /// does not exist, `Err(reason)` when it exists but does not parse.
    /// The reason must not contain a path (it is printed).
    pub session: Result<Option<Session>, String>,
    /// Full text of `interactive.padlog`, if the file exists and is readable.
    pub padlog: Option<String>,
    /// Set when `interactive.padlog` exists but could not be read (I/O error
    /// or not UTF-8); `padlog` is then `None`. Path-free text.
    pub padlog_read_error: Option<String>,
    /// Size in bytes of each file named by a dump entry (`None` = missing).
    pub dump_sizes: BTreeMap<String, Option<u64>>,
    /// `NOT-APPLICABLE.md` exists in the session dir.
    pub not_applicable_marker: bool,
    /// The waiver-marker lines (from [`WAIVE_MARKERS`]) present in
    /// `SESSION-NOTES.md`. A `--waive` glob is authorized iff a present marker
    /// maps to it.
    pub waive_markers: BTreeSet<String>,
}

/// Load and validate a checklist file.
pub fn load_checklist(path: &Path) -> Result<Checklist, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read checklist {}: {}", path.display(), e))?;
    let cl: Checklist = serde_yaml::from_str(&text)
        .map_err(|e| format!("cannot parse checklist {}: {}", path.display(), e))?;
    if cl.kind != CHECKLIST_KIND {
        return Err(format!(
            "checklist kind is {:?}, expected {:?}",
            cl.kind, CHECKLIST_KIND
        ));
    }
    if cl.schema_version != 1 {
        return Err(format!(
            "checklist schema_version {} is not supported (expected 1)",
            cl.schema_version
        ));
    }
    if cl.label_pattern != SUPPORTED_LABEL_PATTERN {
        return Err(format!(
            "checklist label_pattern {:?} is not supported; this build implements {:?}",
            cl.label_pattern, SUPPORTED_LABEL_PATTERN
        ));
    }
    if cl.sessions.is_empty() {
        return Err("checklist declares no sessions".to_owned());
    }
    Ok(cl)
}

/// Session kind for a directory basename: strips one trailing
/// `-take<digits>` (retakes are linted as their base kind).
pub fn resolve_kind(dir_basename: &str) -> String {
    if let Some(idx) = dir_basename.rfind("-take") {
        let digits = &dir_basename[idx + "-take".len()..];
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
            return dir_basename[..idx].to_owned();
        }
    }
    dir_basename.to_owned()
}

/// True when `label` matches [`SUPPORTED_LABEL_PATTERN`].
pub fn label_matches_pattern(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Minimal glob: `*` matches any run (including empty), `?` one char.
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.split_first() {
            None => t.is_empty(),
            Some(('*', rest)) => (0..=t.len()).any(|i| go(rest, &t[i..])),
            Some(('?', rest)) => !t.is_empty() && go(rest, &t[1..]),
            Some((c, rest)) => t.first() == Some(c) && go(rest, &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// Padlog frame count with the writer's possible partial trailing line
/// dropped (the interactive writer flushes per frame, so only the last
/// line can be incomplete). Returns `Err` when the remaining text does not
/// parse.
pub fn padlog_frame_count(text: &str) -> Result<u64, String> {
    let complete = match text.rfind('\n') {
        Some(idx) => &text[..=idx],
        None => "",
    };
    refwork_script::parse(complete)
        .map(|log| log.len() as u64)
        .map_err(|e| e.to_string())
}

/// Gather [`SessionFacts`] from a session directory (read-only; no lock).
pub fn gather_facts(session_dir: &Path) -> SessionFacts {
    let yaml_path = session_dir.join("session.yaml");
    let session = if yaml_path.exists() {
        match std::fs::read_to_string(&yaml_path) {
            Ok(text) => match serde_yaml::from_str::<Session>(&text) {
                Ok(mut s) => {
                    s.dir = session_dir.to_owned();
                    Ok(Some(s))
                }
                Err(e) => Err(format!("session.yaml does not parse as a Session: {}", e)),
            },
            Err(e) => Err(format!("cannot read session.yaml: {}", e)),
        }
    } else {
        Ok(None)
    };

    let padlog_path = session_dir.join(PADLOG_FILE);
    let (padlog, padlog_read_error) = if padlog_path.exists() {
        match std::fs::read_to_string(&padlog_path) {
            Ok(text) => (Some(text), None),
            Err(e) => (
                None,
                Some(format!("interactive.padlog cannot be read: {}", e)),
            ),
        }
    } else {
        (None, None)
    };

    let mut dump_sizes = BTreeMap::new();
    if let Ok(Some(s)) = &session {
        for d in &s.dumps {
            let size = std::fs::metadata(session_dir.join(&d.file))
                .ok()
                .filter(|m| m.is_file())
                .map(|m| m.len());
            dump_sizes.insert(d.file.clone(), size);
        }
    }

    let not_applicable_marker = session_dir.join(NOT_APPLICABLE_MARKER).is_file();
    let waive_markers: BTreeSet<String> = std::fs::read_to_string(session_dir.join(SESSION_NOTES))
        .map(|t| {
            t.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| WAIVE_MARKERS.iter().any(|(m, _)| *m == l))
                .collect()
        })
        .unwrap_or_default();

    SessionFacts {
        session,
        padlog,
        padlog_read_error,
        dump_sizes,
        not_applicable_marker,
        waive_markers,
    }
}

/// The lint proper. `current_emu` is the running build's
/// `refwork_emu::EMU_VERSION`; `waive` globs are assumed already
/// authorized (see [`run_lint`]).
pub fn lint_session(
    kind: &str,
    spec: &SessionSpec,
    facts: &SessionFacts,
    final_: bool,
    waive: &[String],
    current_emu: &str,
) -> LintReport {
    let mut r = LintReport {
        kind: kind.to_owned(),
        ..LintReport::default()
    };

    // C12: NOT-APPLICABLE path.
    if facts.not_applicable_marker && facts.padlog.is_none() {
        if spec.may_be_not_applicable {
            r.push(
                "C12",
                Status::Pass,
                "NOT-APPLICABLE marker present, no padlog; this kind may be not applicable",
            );
            r.not_applicable = true;
            return r;
        }
        r.push(
            "C12",
            Status::Fail,
            "NOT-APPLICABLE marker present but this session kind must be recorded",
        );
    } else if facts.not_applicable_marker {
        r.push(
            "C12",
            Status::Fail,
            "NOT-APPLICABLE marker present alongside a padlog; remove one",
        );
    }

    // C1: session.yaml parses.
    let session = match &facts.session {
        Ok(Some(s)) => {
            r.push("C1", Status::Pass, "session.yaml parses");
            s
        }
        Ok(None) => {
            r.push("C1", Status::Fail, "session.yaml is missing");
            return r;
        }
        Err(e) => {
            r.push("C1", Status::Fail, e.clone());
            return r;
        }
    };

    // C2: padlog exists with the exact header.
    let padlog = match &facts.padlog {
        Some(text) => {
            let first = text.lines().map(str::trim).find(|l| !l.is_empty());
            if first == Some(PADLOG_HEADER) {
                r.push(
                    "C2",
                    Status::Pass,
                    "interactive.padlog has the padlog v1 header",
                );
                Some(text.as_str())
            } else {
                r.push(
                    "C2",
                    Status::Fail,
                    "interactive.padlog first line is not exactly `padlog v1`",
                );
                None
            }
        }
        None => {
            match &facts.padlog_read_error {
                Some(e) => r.push("C2", Status::Fail, e.clone()),
                None => r.push("C2", Status::Fail, "interactive.padlog is missing"),
            }
            None
        }
    };

    // C3: frame count F.
    let frames: Option<u64> = match padlog.map(padlog_frame_count) {
        Some(Ok(f)) => {
            r.push("C3", Status::Info, format!("padlog holds {} frames", f));
            Some(f)
        }
        Some(Err(e)) => {
            r.push("C3", Status::Fail, format!("padlog does not parse: {}", e));
            None
        }
        None => None,
    };

    // C4: log_frames L vs F.
    match (session.log_frames, frames) {
        (None, _) => r.push(
            "C4",
            Status::Fail,
            "log_frames is absent (never saved by an interactive run)",
        ),
        (Some(l), Some(f)) => {
            if final_ {
                if l == f {
                    r.push(
                        "C4",
                        Status::Pass,
                        format!("log_frames == padlog frames ({})", l),
                    );
                } else {
                    r.push(
                        "C4",
                        Status::Fail,
                        format!(
                            "log_frames {} != padlog frames {} (final requires equality; was \
                             the session quit with Esc?)",
                            l, f
                        ),
                    );
                }
            } else if l <= f {
                r.push(
                    "C4",
                    Status::Pass,
                    format!("log_frames {} <= padlog frames {}", l, f),
                );
            } else {
                r.push(
                    "C4",
                    Status::Fail,
                    format!(
                        "log_frames {} > padlog frames {} (log truncated or replaced)",
                        l, f
                    ),
                );
            }
        }
        (Some(l), None) => r.push(
            "C4",
            Status::Fail,
            format!(
                "log_frames {} recorded but the padlog frame count is unknown",
                l
            ),
        ),
    }

    // C5: dump files exist with WRAM_SIZE bytes.
    {
        let mut bad = Vec::new();
        for d in &session.dumps {
            match facts.dump_sizes.get(&d.file).copied().flatten() {
                Some(n) if n as usize == WRAM_SIZE => {}
                Some(n) => bad.push(format!("{} ({} bytes)", d.file, n)),
                None => bad.push(format!("{} (missing)", d.file)),
            }
        }
        if bad.is_empty() {
            r.push(
                "C5",
                Status::Pass,
                format!(
                    "{} dump file(s), each {} bytes",
                    session.dumps.len(),
                    WRAM_SIZE
                ),
            );
        } else {
            r.push(
                "C5",
                Status::Fail,
                format!("dump file problems: {}", bad.join(", ")),
            );
        }
    }

    // C6: unique files; labels match the pattern and name their file.
    {
        let mut seen = BTreeSet::new();
        let mut dup = Vec::new();
        let mut bad_label = Vec::new();
        for d in &session.dumps {
            if !seen.insert(d.file.as_str()) {
                dup.push(d.file.clone());
            }
            if !label_matches_pattern(&d.label) || format!("{}.bin", d.label) != d.file {
                bad_label.push(d.label.clone());
            }
        }
        let mut problems = Vec::new();
        if !dup.is_empty() {
            problems.push(format!("duplicate file(s): {}", dup.join(", ")));
        }
        if !bad_label.is_empty() {
            problems.push(format!(
                "label(s) not matching {} or not naming their file: {:?}",
                SUPPORTED_LABEL_PATTERN, bad_label
            ));
        }
        if problems.is_empty() {
            r.push("C6", Status::Pass, "dump files unique; labels well-formed");
        } else {
            r.push("C6", Status::Fail, problems.join("; "));
        }
    }

    // C7: frames strictly increase (frame > 0 only) and are < F.
    {
        let mut prev: Option<u64> = None;
        let mut problems = Vec::new();
        for d in session.dumps.iter().filter(|d| d.frame > 0) {
            if let Some(p) = prev {
                if d.frame <= p {
                    problems.push(format!(
                        "{} at frame {} does not follow frame {}",
                        d.label, d.frame, p
                    ));
                }
            }
            if let Some(f) = frames {
                if d.frame >= f {
                    problems.push(format!(
                        "{} at frame {} is beyond the padlog ({} frames)",
                        d.label, d.frame, f
                    ));
                }
            }
            prev = Some(d.frame);
        }
        if problems.is_empty() {
            r.push(
                "C7",
                Status::Pass,
                "dump frames strictly increase within the padlog",
            );
        } else {
            r.push("C7", Status::Fail, problems.join("; "));
        }
    }

    // C13: no frame-0 dump.
    {
        let zero: Vec<&str> = session
            .dumps
            .iter()
            .filter(|d| d.frame == 0)
            .map(|d| d.label.as_str())
            .collect();
        if zero.is_empty() {
            r.push("C13", Status::Pass, "no frame-0 dump");
        } else {
            r.push(
                "C13",
                Status::Fail,
                format!(
                    "frame-0 dump(s) {:?}: treated as platform captures by every tool; \
                     never press F5 on the first frame",
                    zero
                ),
            );
        }
    }

    // C8: required labels present.
    let labels: BTreeSet<&str> = session.dumps.iter().map(|d| d.label.as_str()).collect();
    {
        for req in &spec.required {
            if labels.contains(req.as_str()) {
                continue;
            }
            if waive.iter().any(|g| glob_matches(g, req)) {
                r.waived.push(req.clone());
            } else {
                r.missing_required.push(req.clone());
            }
        }
        let waived_note = if r.waived.is_empty() {
            String::new()
        } else {
            format!("; WAIVED: {}", r.waived.join(", "))
        };
        if r.missing_required.is_empty() {
            r.push(
                "C8",
                Status::Pass,
                format!(
                    "all {} required labels present{}",
                    spec.required.len(),
                    waived_note
                ),
            );
        } else if final_ {
            r.push(
                "C8",
                Status::Fail,
                format!(
                    "missing required label(s): {}{}",
                    r.missing_required.join(", "),
                    waived_note
                ),
            );
        } else {
            r.push(
                "C8",
                Status::Info,
                format!(
                    "{} of {} required labels still to take: {}{}",
                    r.missing_required.len(),
                    spec.required.len(),
                    r.missing_required.join(", "),
                    waived_note
                ),
            );
        }
    }

    // C9: order of the present required labels (first occurrence each).
    if spec.ordered {
        let mut first_frame: BTreeMap<&str, u64> = BTreeMap::new();
        for d in &session.dumps {
            first_frame.entry(d.label.as_str()).or_insert(d.frame);
        }
        let mut prev: Option<(&str, u64)> = None;
        let mut problems = Vec::new();
        for req in &spec.required {
            let Some(&f) = first_frame.get(req.as_str()) else {
                continue;
            };
            if let Some((plabel, pf)) = prev {
                if f <= pf {
                    problems.push(format!(
                        "{} (frame {}) should come after {} (frame {})",
                        req, f, plabel, pf
                    ));
                }
            }
            prev = Some((req.as_str(), f));
        }
        if problems.is_empty() {
            r.push(
                "C9",
                Status::Pass,
                "present required labels are in checklist order",
            );
        } else {
            r.push("C9", Status::Fail, problems.join("; "));
        }
    } else {
        r.push("C9", Status::Info, "kind is unordered");
    }

    // C10: forbidden substrings (case-insensitive) in any label.
    {
        let mut hits = Vec::new();
        for d in &session.dumps {
            let lower = d.label.to_ascii_lowercase();
            for sub in &spec.forbidden_substrings {
                if lower.contains(&sub.to_ascii_lowercase()) {
                    hits.push(format!("{} contains {:?}", d.label, sub));
                }
            }
        }
        if hits.is_empty() {
            r.push("C10", Status::Pass, "no forbidden substring in any label");
        } else {
            r.push("C10", Status::Fail, hits.join("; "));
        }
    }

    // C11: epoch stamp.
    {
        let mut problems = Vec::new();
        match session.emu_version.as_deref() {
            None => problems.push("emu_version absent".to_owned()),
            Some(v) if v != current_emu => problems.push(format!(
                "emu_version is {:?} but this build is {:?}",
                v, current_emu
            )),
            Some(_) => {}
        }
        match session.rom_blake3.as_deref() {
            None => problems.push("rom_blake3 absent".to_owned()),
            Some(h)
                if h.len() != 64
                    || !h
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                problems.push("rom_blake3 is not 64 lowercase hex characters".to_owned())
            }
            Some(_) => {}
        }
        if problems.is_empty() {
            r.push(
                "C11",
                Status::Pass,
                format!(
                    "stamped emu_version {:?}; rom_blake3 well-formed",
                    current_emu
                ),
            );
        } else {
            r.push("C11", Status::Fail, problems.join("; "));
        }
    }

    r
}

/// Run the lint against a session directory: load the checklist, resolve
/// the kind, authorize `--waive`, gather facts, lint.
pub fn run_lint(opts: &LintOpts) -> Result<LintReport, String> {
    let checklist = load_checklist(&opts.checklist)?;
    let kind = match &opts.kind {
        Some(k) => k.clone(),
        None => {
            let base = opts
                .session_dir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| "cannot derive a session kind from --session".to_owned())?;
            resolve_kind(base)
        }
    };
    let spec = checklist.sessions.get(&kind).ok_or_else(|| {
        format!(
            "checklist has no session kind {:?} (known: {}); pass --kind",
            kind,
            checklist
                .sessions
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    if !opts.session_dir.is_dir() {
        return Err("--session is not a directory".to_owned());
    }
    let facts = gather_facts(&opts.session_dir);
    for glob in &opts.waive {
        let authorized = WAIVE_MARKERS
            .iter()
            .any(|(m, g)| g == glob && facts.waive_markers.contains(*m));
        if !authorized {
            let needed: Vec<&str> = WAIVE_MARKERS
                .iter()
                .filter(|(_, g)| g == glob)
                .map(|(m, _)| *m)
                .collect();
            return Err(if needed.is_empty() {
                format!(
                    "--waive {:?}: no waiver marker is defined for this glob",
                    glob
                )
            } else {
                format!(
                    "--waive {:?} requires one of these lines in {}: {}",
                    glob,
                    SESSION_NOTES,
                    needed.join(" | ")
                )
            });
        }
    }
    Ok(lint_session(
        &kind,
        spec,
        &facts,
        opts.final_,
        &opts.waive,
        refwork_emu::EMU_VERSION,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::DumpMeta;

    const REPO_CHECKLIST: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/discovery-02-required-labels.yaml"
    );

    fn checklist() -> Checklist {
        load_checklist(Path::new(REPO_CHECKLIST)).unwrap()
    }

    fn main_spec() -> SessionSpec {
        checklist().sessions["discovery-02-main"].clone()
    }

    fn padlog(frames: u64) -> String {
        let mut s = String::from("padlog v1\n");
        for _ in 0..frames {
            s.push_str("0000\n");
        }
        s
    }

    fn valid_hex() -> String {
        "0123456789abcdef".repeat(4)
    }

    /// A complete, passing main session: every required label at
    /// increasing frames, one optional label, padlog of `frames` frames.
    fn passing_facts(spec: &SessionSpec, frames: u64) -> SessionFacts {
        let mut s = Session::new("/nonexistent-session-dir");
        let mut sizes = BTreeMap::new();
        let mut frame = 100u64;
        for label in spec.required.iter().chain(spec.optional.iter().take(1)) {
            let file = format!("{}.bin", label);
            s.dumps.push(DumpMeta {
                label: label.clone(),
                frame,
                file: file.clone(),
                region: "wram".to_owned(),
            });
            sizes.insert(file, Some(WRAM_SIZE as u64));
            frame += 100;
        }
        s.log_frames = Some(frames);
        s.stamp(refwork_emu::EMU_VERSION, &valid_hex());
        SessionFacts {
            session: Ok(Some(s)),
            padlog: Some(padlog(frames)),
            padlog_read_error: None,
            dump_sizes: sizes,
            not_applicable_marker: false,
            waive_markers: BTreeSet::new(),
        }
    }

    fn session_mut(f: &mut SessionFacts) -> &mut Session {
        f.session.as_mut().unwrap().as_mut().unwrap()
    }

    fn fails(report: &LintReport) -> Vec<&'static str> {
        report
            .checks
            .iter()
            .filter(|c| c.status == Status::Fail)
            .map(|c| c.id)
            .collect()
    }

    #[test]
    fn checklist_file_in_repo_parses_and_lists_three_kinds() {
        let cl = checklist();
        let kinds: Vec<&String> = cl.sessions.keys().collect();
        assert_eq!(
            kinds,
            vec![
                "discovery-02-gameover-death",
                "discovery-02-gameover-timer",
                "discovery-02-main",
                "discovery-02-score-currency"
            ]
        );
        let main = &cl.sessions["discovery-02-main"];
        for l in [
            "w1s1-entry",
            "w1s2-entry",
            "w1s3-entry",
            "w1s4-entry",
            "w2s1-entry",
            "session-end",
        ] {
            assert!(main.required.iter().any(|r| r == l), "missing {}", l);
        }
        assert_eq!(main.required.len(), 22);
        assert!(main.ordered);
        assert!(!main.may_be_not_applicable);
        assert_eq!(
            main.forbidden_substrings,
            vec!["gameover", "game-over", "dead", "death"]
        );
        assert!(cl.sessions["discovery-02-gameover-timer"].may_be_not_applicable);
        assert!(!cl.sessions["discovery-02-gameover-death"].may_be_not_applicable);
        // Every listed label matches the sanitizer pattern (so it names its file).
        for spec in cl.sessions.values() {
            for l in spec.required.iter().chain(spec.optional.iter()) {
                assert!(label_matches_pattern(l), "bad label {:?}", l);
            }
        }
    }

    #[test]
    fn resolve_kind_strips_take_suffix() {
        assert_eq!(resolve_kind("discovery-02-main-take2"), "discovery-02-main");
        assert_eq!(
            resolve_kind("discovery-02-main-take12"),
            "discovery-02-main"
        );
        assert_eq!(resolve_kind("discovery-02-main"), "discovery-02-main");
        assert_eq!(
            resolve_kind("discovery-02-main-takeX"),
            "discovery-02-main-takeX"
        );
        assert_eq!(
            resolve_kind("discovery-02-main-take"),
            "discovery-02-main-take"
        );
    }

    #[test]
    fn glob_matches_star_and_question() {
        assert!(glob_matches("score-*", "score-before-1"));
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("score-?efore-1", "score-before-1"));
        assert!(!glob_matches("score-*", "health-full"));
        assert!(!glob_matches("score-?", "score-before-1"));
        assert!(glob_matches("exact", "exact"));
        assert!(!glob_matches("exact", "exactly"));
    }

    #[test]
    fn passing_final_session() {
        let spec = main_spec();
        let facts = passing_facts(&spec, 5000);
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
        assert_eq!(r.verdict(), "lint: PASS");
        assert!(r.missing_required.is_empty());
        let text = r.render();
        assert!(text.starts_with("kind: discovery-02-main\n"), "{}", text);
        assert!(text.ends_with("lint: PASS\n"), "{}", text);
    }

    #[test]
    fn mid_session_reports_missing_as_info() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            s.dumps
                .retain(|d| !["w2-hub", "w2s1-entry", "session-end"].contains(&d.label.as_str()));
            s.log_frames = Some(2100); // saved at the last dump; padlog is longer
        }
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
        let c8 = r.checks.iter().find(|c| c.id == "C8").unwrap();
        assert_eq!(c8.status, Status::Info);
        assert!(c8.reason.contains("3 of 22"), "{}", c8.reason);
        assert_eq!(
            r.missing_required,
            vec!["w2-hub", "w2s1-entry", "session-end"]
        );
        assert_eq!(r.verdict(), "lint: PASS");
    }

    #[test]
    fn final_fails_on_missing_label() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| d.label != "w1-clear");
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C8"], "{}", r.render());
        assert_eq!(r.verdict(), "lint: FAIL (1 checks)");
        assert!(r.render().contains("w1-clear"));
    }

    #[test]
    fn final_requires_log_frames_equal_padlog() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts).log_frames = Some(4999);
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C4"], "{}", r.render());
        // Mid-session the same state is fine.
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
    }

    #[test]
    fn fails_on_log_frames_ahead_of_padlog() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts).log_frames = Some(5001);
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C4"], "{}", r.render());
    }

    #[test]
    fn fails_on_missing_log_frames() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts).log_frames = None;
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C4"], "{}", r.render());
    }

    #[test]
    fn fails_on_wrong_dump_size() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        facts.dump_sizes.insert("w1-hub.bin".to_owned(), Some(12));
        facts.dump_sizes.insert("w1s1-entry.bin".to_owned(), None);
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C5"], "{}", r.render());
        let c5 = r.checks.iter().find(|c| c.id == "C5").unwrap();
        assert!(c5.reason.contains("w1-hub.bin (12 bytes)"), "{}", c5.reason);
        assert!(
            c5.reason.contains("w1s1-entry.bin (missing)"),
            "{}",
            c5.reason
        );
    }

    #[test]
    fn fails_on_duplicate_file() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            // Two distinct labels that sanitize to the same file (F5 hazard).
            let last = s.dumps.last().unwrap().frame;
            s.dumps.push(DumpMeta {
                label: "session end".to_owned(),
                frame: last + 10,
                file: "session_end.bin".to_owned(),
                region: "wram".to_owned(),
            });
            s.dumps.push(DumpMeta {
                label: "session_end".to_owned(),
                frame: last + 20,
                file: "session_end.bin".to_owned(),
                region: "wram".to_owned(),
            });
        }
        facts
            .dump_sizes
            .insert("session_end.bin".to_owned(), Some(WRAM_SIZE as u64));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C6"], "{}", r.render());
        let c6 = r.checks.iter().find(|c| c.id == "C6").unwrap();
        assert!(
            c6.reason.contains("duplicate file(s): session_end.bin"),
            "{}",
            c6.reason
        );
        assert!(c6.reason.contains("\"session end\""), "{}", c6.reason);
    }

    #[test]
    fn fails_on_out_of_order_required() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            // Swap the frames of w1s3-entry and w1s4-entry but keep insertion
            // order sorted, so C7 stays clean and only C9 fires.
            let s = session_mut(&mut facts);
            let i3 = s
                .dumps
                .iter()
                .position(|d| d.label == "w1s3-entry")
                .unwrap();
            let i4 = s
                .dumps
                .iter()
                .position(|d| d.label == "w1s4-entry")
                .unwrap();
            s.dumps.swap(i3, i4);
            let f3 = s.dumps[i3].frame;
            let f4 = s.dumps[i4].frame;
            s.dumps[i3].frame = f4.min(f3);
            s.dumps[i4].frame = f4.max(f3);
        }
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C9"], "{}", r.render());
    }

    #[test]
    fn fails_on_non_increasing_dump_frames_and_beyond_padlog() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            let n = s.dumps.len();
            s.dumps[n - 1].frame = 6000; // beyond the 5000-frame padlog
            s.dumps[1].frame = s.dumps[0].frame; // not strictly increasing
        }
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        let f = fails(&r);
        assert!(f.contains(&"C7"), "{}", r.render());
    }

    #[test]
    fn fails_on_missing_stamp() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            s.emu_version = None;
            s.rom_blake3 = None;
        }
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C11"], "{}", r.render());
    }

    #[test]
    fn fails_on_stale_stamp() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts).emu_version = Some("refwork-emu 0.2.0".to_owned());
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C11"], "{}", r.render());
        // Malformed hash too.
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts).rom_blake3 = Some("ABCDEF".to_owned());
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C11"], "{}", r.render());
    }

    #[test]
    fn fails_on_forbidden_label_in_main() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            let last = s.dumps.last().unwrap().frame;
            s.dumps.push(DumpMeta {
                label: "note-GameOver-oops".to_owned(),
                frame: last + 10,
                file: "note-GameOver-oops.bin".to_owned(),
                region: "wram".to_owned(),
            });
        }
        facts
            .dump_sizes
            .insert("note-GameOver-oops.bin".to_owned(), Some(WRAM_SIZE as u64));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C10"], "{}", r.render());
    }

    #[test]
    fn fails_on_hyphenated_game_over_label_in_main() {
        // Regression: discovery-02-main take 1 carried a `game-over-screen`
        // dump that slipped past a `gameover`-only forbidden list.
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            let last = s.dumps.last().unwrap().frame;
            s.dumps.push(DumpMeta {
                label: "game-over-screen".to_owned(),
                frame: last + 10,
                file: "game-over-screen.bin".to_owned(),
                region: "wram".to_owned(),
            });
        }
        facts
            .dump_sizes
            .insert("game-over-screen.bin".to_owned(), Some(WRAM_SIZE as u64));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(fails(&r).contains(&"C10"), "{}", r.render());
    }

    #[test]
    fn unknown_extra_labels_are_ignored() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            let last = s.dumps.last().unwrap().frame;
            s.dumps.push(DumpMeta {
                label: "note-shop-entered".to_owned(),
                frame: last + 10,
                file: "note-shop-entered.bin".to_owned(),
                region: "wram".to_owned(),
            });
        }
        facts
            .dump_sizes
            .insert("note-shop-entered.bin".to_owned(), Some(WRAM_SIZE as u64));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
    }

    #[test]
    fn partial_trailing_line_is_tolerated() {
        assert_eq!(padlog_frame_count("padlog v1\n0000\n0001\n00").unwrap(), 2);
        assert_eq!(padlog_frame_count("padlog v1\n0000\n0001\n").unwrap(), 2);
        assert_eq!(padlog_frame_count("padlog v1\n").unwrap(), 0);
        assert!(!padlog_frame_count("padlog v1").unwrap_err().is_empty());
        assert!(padlog_frame_count("padlog v1\nzzzz\n").is_err());

        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        facts.padlog.as_mut().unwrap().push_str("00"); // writer mid-line
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
    }

    #[test]
    fn padlog_header_must_be_exact() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        facts.padlog = Some(format!("padlog v1 rom={}\n0000\n", valid_hex()));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(fails(&r).contains(&"C2"), "{}", r.render());
        facts.padlog = None;
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(fails(&r).contains(&"C2"), "{}", r.render());
    }

    #[test]
    fn unreadable_padlog_is_reported_distinctly() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        facts.padlog = None;
        facts.padlog_read_error = Some("interactive.padlog cannot be read: bad utf-8".to_owned());
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            false,
            &[],
            refwork_emu::EMU_VERSION,
        );
        let c2 = r.checks.iter().find(|c| c.id == "C2").unwrap();
        assert_eq!(c2.status, Status::Fail);
        assert!(c2.reason.contains("cannot be read"), "{}", c2.reason);
        assert!(!c2.reason.contains("missing"), "{}", c2.reason);
    }

    #[test]
    fn not_applicable_marker_passes_only_when_allowed() {
        let cl = checklist();
        let facts = SessionFacts {
            session: Ok(None),
            padlog: None,
            padlog_read_error: None,
            dump_sizes: BTreeMap::new(),
            not_applicable_marker: true,
            waive_markers: BTreeSet::new(),
        };
        let timer = &cl.sessions["discovery-02-gameover-timer"];
        let r = lint_session(
            "discovery-02-gameover-timer",
            timer,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
        assert!(r.not_applicable);
        assert_eq!(r.verdict(), "lint: PASS (NOT-APPLICABLE)");
        assert_eq!(r.checks.len(), 1);

        let death = &cl.sessions["discovery-02-gameover-death"];
        let r = lint_session(
            "discovery-02-gameover-death",
            death,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(!r.passed());
        assert!(fails(&r).contains(&"C12"), "{}", r.render());

        // Marker next to a real padlog: contradictory, fails even when allowed.
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        facts.not_applicable_marker = true;
        let r = lint_session(
            "discovery-02-gameover-timer",
            timer,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert!(fails(&r).contains(&"C12"), "{}", r.render());
    }

    #[test]
    fn fails_on_frame_zero_dump() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        {
            let s = session_mut(&mut facts);
            s.dumps.insert(
                0,
                DumpMeta {
                    label: "too-early".to_owned(),
                    frame: 0,
                    file: "too-early.bin".to_owned(),
                    region: "wram".to_owned(),
                },
            );
        }
        facts
            .dump_sizes
            .insert("too-early.bin".to_owned(), Some(WRAM_SIZE as u64));
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &[],
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C13"], "{}", r.render());
    }

    #[test]
    fn waive_reports_waived_and_passes() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| !d.label.starts_with("score-"));
        facts
            .waive_markers
            .insert("score_counter: NOT-APPLICABLE".to_owned());
        let waive = vec!["score-*".to_owned()];
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &waive,
            refwork_emu::EMU_VERSION,
        );
        assert!(r.passed(), "{}", r.render());
        assert_eq!(
            r.waived,
            vec![
                "score-before-1",
                "score-after-1",
                "score-before-2",
                "score-after-2"
            ]
        );
        assert_eq!(
            r.verdict(),
            "lint: PASS (WAIVED: score-before-1, score-after-1, score-before-2, score-after-2)"
        );
    }

    #[test]
    fn waive_does_not_cover_other_missing_labels() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| !d.label.starts_with("score-") && d.label != "w2-hub");
        facts
            .waive_markers
            .insert("score_counter: NOT-APPLICABLE".to_owned());
        let waive = vec!["score-*".to_owned()];
        let r = lint_session(
            "discovery-02-main",
            &spec,
            &facts,
            true,
            &waive,
            refwork_emu::EMU_VERSION,
        );
        assert_eq!(fails(&r), vec!["C8"], "{}", r.render());
        assert_eq!(r.missing_required, vec!["w2-hub"]);
        assert_eq!(r.waived.len(), 4);
    }

    // ─── I/O path (temp dirs, no ROM) ────────────────────────────────────

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> TempDir {
            let dir = std::env::temp_dir().join(format!(
                "ramdiff-lint-{}-{}-{}",
                tag,
                std::process::id(),
                std::thread::current().name().unwrap_or("test")
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Write a real on-disk session from `facts` (dump files are filled with
    /// `fill`).
    fn materialize(dir: &Path, facts: &SessionFacts, fill: u8) {
        let s = facts.session.as_ref().unwrap().as_ref().unwrap();
        let mut s = s.clone();
        s.dir = dir.to_owned();
        s.save().unwrap();
        if let Some(p) = &facts.padlog {
            std::fs::write(dir.join(PADLOG_FILE), p).unwrap();
        }
        for (file, size) in &facts.dump_sizes {
            if let Some(n) = size {
                std::fs::write(dir.join(file), vec![fill; *n as usize]).unwrap();
            }
        }
    }

    #[test]
    fn run_lint_end_to_end_and_output_hygiene() {
        let spec = main_spec();
        let mut facts = passing_facts(&spec, 5000);
        // Make it fail (stale stamp) and give it a recognizable hash.
        let hash = "deadbeef".repeat(8);
        {
            let s = session_mut(&mut facts);
            s.emu_version = Some("refwork-emu 0.0.0-old".to_owned());
            s.rom_blake3 = Some(hash.clone());
        }
        let tmp = TempDir::new("e2e");
        let session_dir = tmp.0.join("discovery-02-main-take3");
        std::fs::create_dir_all(&session_dir).unwrap();
        materialize(&session_dir, &facts, 0xA5);

        let report = run_lint(&LintOpts {
            session_dir: session_dir.clone(),
            checklist: PathBuf::from(REPO_CHECKLIST),
            kind: None,
            final_: true,
            waive: vec![],
        })
        .unwrap();
        assert_eq!(report.kind, "discovery-02-main", "take suffix resolves");
        assert_eq!(fails(&report), vec!["C11"], "{}", report.render());
        let text = report.render();
        assert!(!text.contains(&hash), "hash leaked: {}", text);
        assert!(
            !text.contains(session_dir.to_str().unwrap()),
            "path leaked: {}",
            text
        );
        assert!(!text.contains("a5a5"), "dump bytes leaked: {}", text);
    }

    #[test]
    fn run_lint_unknown_kind_and_bad_checklist() {
        let tmp = TempDir::new("kinds");
        let session_dir = tmp.0.join("something-else");
        std::fs::create_dir_all(&session_dir).unwrap();
        let err = run_lint(&LintOpts {
            session_dir: session_dir.clone(),
            checklist: PathBuf::from(REPO_CHECKLIST),
            kind: None,
            final_: false,
            waive: vec![],
        })
        .unwrap_err();
        assert!(err.contains("no session kind"), "{}", err);
        assert!(err.contains("discovery-02-main"), "{}", err);

        let bad = tmp.0.join("bad.yaml");
        std::fs::write(
            &bad,
            "kind: other\nschema_version: 1\nlabel_pattern: x\nsessions: {}\n",
        )
        .unwrap();
        let err = load_checklist(&bad).unwrap_err();
        assert!(err.contains("kind"), "{}", err);
    }

    #[test]
    fn waive_requires_notes_marker() {
        let spec = main_spec();
        let facts = passing_facts(&spec, 5000);
        let tmp = TempDir::new("waive");
        let session_dir = tmp.0.join("discovery-02-main");
        std::fs::create_dir_all(&session_dir).unwrap();
        materialize(&session_dir, &facts, 0);
        let opts = |waive: Vec<String>| LintOpts {
            session_dir: session_dir.clone(),
            checklist: PathBuf::from(REPO_CHECKLIST),
            kind: None,
            final_: true,
            waive,
        };
        let err = run_lint(&opts(vec!["score-*".to_owned()])).unwrap_err();
        assert!(err.contains("score_counter: NOT-APPLICABLE"), "{}", err);

        // Marker present (with surrounding notes): accepted.
        std::fs::write(
            session_dir.join(SESSION_NOTES),
            "date: 2026-09-03\nscore_counter: NOT-APPLICABLE\ndeath_causes: none\n",
        )
        .unwrap();
        let r = run_lint(&opts(vec!["score-*".to_owned()])).unwrap();
        assert!(r.passed(), "{}", r.render());
        // Nothing was missing, so nothing is waived and the verdict is plain.
        assert_eq!(r.verdict(), "lint: PASS");

        // Without --waive the marker changes nothing.
        let r = run_lint(&opts(vec![])).unwrap();
        assert!(r.passed());
    }

    #[test]
    fn waive_marker_is_scoped_per_glob() {
        // The score-currency kind requires currency-only-* labels; a session
        // missing them can be waived only by a currency marker, never by the
        // score-counter marker (the pre-refactor global-flag bug).
        let cl = checklist();
        let spec = cl.sessions["discovery-02-score-currency"].clone();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| !d.label.starts_with("currency-only-"));
        let tmp = TempDir::new("scoped-waive");
        let dir = tmp.0.join("discovery-02-score-currency");
        std::fs::create_dir_all(&dir).unwrap();
        materialize(&dir, &facts, 0);
        let opts = |notes: Option<&str>, waive: Vec<String>| {
            if let Some(n) = notes {
                std::fs::write(dir.join(SESSION_NOTES), n).unwrap();
            } else {
                let _ = std::fs::remove_file(dir.join(SESSION_NOTES));
            }
            LintOpts {
                session_dir: dir.clone(),
                checklist: PathBuf::from(REPO_CHECKLIST),
                kind: None,
                final_: true,
                waive,
            }
        };

        // No marker at all: --waive currency-only-* is refused, naming the lines.
        let err = run_lint(&opts(None, vec!["currency-only-*".to_owned()])).unwrap_err();
        assert!(err.contains("currency: NOT-APPLICABLE"), "{}", err);
        assert!(err.contains("currency_only: NOT-AVAILABLE"), "{}", err);

        // The WRONG marker (score-counter) must NOT authorize currency-only-*.
        let err = run_lint(&opts(
            Some(
                "score_counter: NOT-APPLICABLE
",
            ),
            vec!["currency-only-*".to_owned()],
        ))
        .unwrap_err();
        assert!(err.contains("currency"), "{}", err);

        // The right marker authorizes it → PASS (WAIVED).
        let r = run_lint(&opts(
            Some(
                "currency: NOT-APPLICABLE
",
            ),
            vec!["currency-only-*".to_owned()],
        ))
        .unwrap();
        assert!(r.passed(), "{}", r.render());
        assert!(
            r.verdict().contains("WAIVED: currency-only-"),
            "{}",
            r.verdict()
        );

        // The alternate currency marker also authorizes it.
        let r = run_lint(&opts(
            Some(
                "currency_only: NOT-AVAILABLE
",
            ),
            vec!["currency-only-*".to_owned()],
        ))
        .unwrap();
        assert!(r.passed(), "{}", r.render());
    }

    #[test]
    fn waive_score_only_glob_needs_its_own_marker() {
        let cl = checklist();
        let spec = cl.sessions["discovery-02-score-currency"].clone();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| !d.label.starts_with("score-only-"));
        let tmp = TempDir::new("scoreonly-waive");
        let dir = tmp.0.join("discovery-02-score-currency");
        std::fs::create_dir_all(&dir).unwrap();
        materialize(&dir, &facts, 0);
        let mk = |notes: &str, waive: Vec<String>| {
            std::fs::write(dir.join(SESSION_NOTES), notes).unwrap();
            LintOpts {
                session_dir: dir.clone(),
                checklist: PathBuf::from(REPO_CHECKLIST),
                kind: None,
                final_: true,
                waive,
            }
        };
        // currency marker does not authorize score-only-*.
        let err = run_lint(&mk(
            "currency: NOT-APPLICABLE
",
            vec!["score-only-*".to_owned()],
        ))
        .unwrap_err();
        assert!(err.contains("score_only: NOT-AVAILABLE"), "{}", err);
        // its own marker does.
        let r = run_lint(&mk(
            "score_only: NOT-AVAILABLE
",
            vec!["score-only-*".to_owned()],
        ))
        .unwrap();
        assert!(r.passed(), "{}", r.render());
        assert!(
            r.verdict().contains("WAIVED: score-only-"),
            "{}",
            r.verdict()
        );
    }

    #[test]
    fn waive_both_event_glob_needs_its_own_marker() {
        let cl = checklist();
        let spec = cl.sessions["discovery-02-score-currency"].clone();
        let mut facts = passing_facts(&spec, 5000);
        session_mut(&mut facts)
            .dumps
            .retain(|d| !d.label.starts_with("both-"));
        let tmp = TempDir::new("both-waive");
        let dir = tmp.0.join("discovery-02-score-currency");
        std::fs::create_dir_all(&dir).unwrap();
        materialize(&dir, &facts, 0);
        let mk = |notes: &str, waive: Vec<String>| {
            std::fs::write(dir.join(SESSION_NOTES), notes).unwrap();
            LintOpts {
                session_dir: dir.clone(),
                checklist: PathBuf::from(REPO_CHECKLIST),
                kind: None,
                final_: true,
                waive,
            }
        };
        // A currency marker does not authorize both-*.
        let err =
            run_lint(&mk("currency: NOT-APPLICABLE\n", vec!["both-*".to_owned()])).unwrap_err();
        assert!(err.contains("both_event: NOT-AVAILABLE"), "{}", err);
        // Its own marker does.
        let r = run_lint(&mk(
            "both_event: NOT-AVAILABLE\n",
            vec!["both-*".to_owned()],
        ))
        .unwrap();
        assert!(r.passed(), "{}", r.render());
        assert!(r.verdict().contains("WAIVED: both-"), "{}", r.verdict());
    }

    #[test]
    fn run_lint_not_applicable_dir() {
        let tmp = TempDir::new("na");
        let session_dir = tmp.0.join("discovery-02-gameover-timer");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join(NOT_APPLICABLE_MARKER),
            "NOT-APPLICABLE: the demo game shows no level timer.\n",
        )
        .unwrap();
        let r = run_lint(&LintOpts {
            session_dir,
            checklist: PathBuf::from(REPO_CHECKLIST),
            kind: None,
            final_: true,
            waive: vec![],
        })
        .unwrap();
        assert_eq!(r.verdict(), "lint: PASS (NOT-APPLICABLE)");
    }
}
