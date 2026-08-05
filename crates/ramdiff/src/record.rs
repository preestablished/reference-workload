//! `ramdiff record` — scripted core replay with WRAM dump marks.
//!
//! Runs `refwork-emu` host-side, feeding pad words from a `.padlog` script.
//! Dumps WRAM at `--mark <frame>=<label>` positions or every `N` frames
//! with `--dump-every N`.
//!
//! # Interactive mode (`--interactive`, feature `interactive`)
//!
//! When the `interactive` cargo feature is enabled, the `--interactive` flag
//! opens a `minifb` window showing the blitted framebuffer.
//!
//! API.md §3.4 pad bitmask (bit 0..11): A B X Y L R Up Down Left Right Start Select
//!
//! Keyboard mapping:
//! | Key | Button | Bit |
//! |-----|--------|-----|
//! | X key | A | 0 |
//! | Z key | B | 1 |
//! | A key | X button | 2 |
//! | S key | Y button | 3 |
//! | Q key | L | 4 |
//! | W key | R | 5 |
//! | Up | Up | 6 |
//! | Down | Down | 7 |
//! | Left | Left | 8 |
//! | Right | Right | 9 |
//! | Enter | Start | 10 |
//! | RShift | Select | 11 |
//!
//! Hotkey `F5`: prompt for a label in the terminal, then dump WRAM.
//! Hotkey `M`: toggle audio mute (host-side only; never touches the pad word,
//! the padlog, or emulator state).
//!
//! Audio: the core's stereo S-DSP stream (`Core::take_audio_samples`) is
//! drained once per live frame and played through the default output device
//! via `cpal` (see `audio.rs`). A missing or failing audio device degrades
//! to silent playback with a single stderr note, exactly like a missing
//! gamepad degrades to keyboard-only. `--no-audio` skips opening a device
//! entirely.
//!
//! The input log is appended incrementally — one `HHHH\n` line per frame,
//! flushed per frame, so a killed session loses only the current frame.
//! The resulting file is a valid `.padlog` (FORMAT.md): header written once,
//! then one hex word per line (no RLE in incremental mode — valid per grammar).

use crate::session::{DumpMeta, Session, WRAM_SIZE};
use refwork_emu::{Cartridge, Core, RegionBuffers, WRAM_INIT_BYTE};
use refwork_script::PadLog;
use std::collections::BTreeMap;

/// Run `ramdiff record` with the given options.
///
/// `marks`: mapping from frame number → label string.
/// `dump_every`: if `Some(n)`, also dump at every nth frame (label = `"frame-<n>"`).
pub fn run_record(opts: &RecordOpts) -> Result<(), String> {
    // Ensure session directory exists.
    std::fs::create_dir_all(&opts.session_dir)
        .map_err(|e| format!("cannot create session dir: {}", e))?;

    let mut session = Session::load(&opts.session_dir)?;

    // Load ROM.
    let rom_bytes = std::fs::read(&opts.rom)
        .map_err(|e| format!("cannot read ROM {:?}: {}", opts.rom.display(), e))?;
    let cart = Cartridge::from_rom(rom_bytes, None).map_err(|e| format!("bad ROM: {:?}", e))?;

    // Allocate leaked WRAM buffer (matches hash_chain.rs pattern).
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    // Parse the input script.
    let script_text = std::fs::read_to_string(&opts.script)
        .map_err(|e| format!("cannot read script {:?}: {}", opts.script.display(), e))?;
    let pad_log =
        refwork_script::parse(&script_text).map_err(|e| format!("cannot parse script: {}", e))?;

    // Build mark map: frame → label (from CLI --mark flags).
    let marks: BTreeMap<u64, String> = opts.marks.iter().cloned().collect();

    // Run the core.
    let total_frames = opts.total_frames.unwrap_or(u64::MAX);

    for frame in 0u64..total_frames {
        let pad = get_pad(&pad_log, frame);
        let flags = core.run_one_frame(pad);
        if let Some(fault) = core.fault() {
            eprintln!(
                "record: fault at frame {} (flags={:?}): {:?}",
                frame, flags, fault
            );
            // Still perform any pending mark dump before bailing.
            dump_if_marked(frame, &marks, opts, &mut session, &core)?;
            break;
        }

        // Dump at marks.
        dump_if_marked(frame, &marks, opts, &mut session, &core)?;

        // Dump every N frames.
        if let Some(n) = opts.dump_every {
            if n > 0 && frame > 0 && frame % n == 0 {
                let label = format!("frame-{}", frame);
                do_dump(frame, &label, opts, &mut session, &core)?;
            }
        }
    }

    session.save()?;
    Ok(())
}

/// Get the pad word for `frame` from the log (hold last word past end).
pub fn get_pad(log: &PadLog, frame: u64) -> u16 {
    let idx = (frame as usize).min(log.frames.len().saturating_sub(1));
    if log.frames.is_empty() {
        0
    } else {
        log.frames[idx]
    }
}

fn dump_if_marked(
    frame: u64,
    marks: &BTreeMap<u64, String>,
    opts: &RecordOpts,
    session: &mut Session,
    core: &Core,
) -> Result<(), String> {
    if let Some(label) = marks.get(&frame) {
        do_dump(frame, label, opts, session, core)?;
    }
    Ok(())
}

fn do_dump(
    frame: u64,
    label: &str,
    opts: &RecordOpts,
    session: &mut Session,
    core: &Core,
) -> Result<(), String> {
    // Sanitize label for use as filename.
    let safe_label: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file = format!("{}.bin", safe_label);

    // Write raw WRAM bytes.
    let wram: &[u8; WRAM_SIZE] = core.wram();
    session.write_dump(&file, wram)?;

    let meta = DumpMeta {
        label: label.to_owned(),
        frame,
        file,
        region: "wram".to_owned(),
    };
    session.add_dump(meta);

    if !opts.quiet {
        eprintln!("record: dumped WRAM at frame {} → label {:?}", frame, label);
    }
    Ok(())
}

/// Options for `ramdiff record`.
pub struct RecordOpts {
    pub rom: std::path::PathBuf,
    pub script: std::path::PathBuf,
    pub session_dir: std::path::PathBuf,
    /// `(frame, label)` pairs from `--mark`.
    pub marks: Vec<(u64, String)>,
    /// Dump every N frames.
    pub dump_every: Option<u64>,
    /// Run for exactly this many frames; `None` = until script end or fault.
    pub total_frames: Option<u64>,
    pub quiet: bool,
}

/// Parse `--mark <frame>=<label>` argument.
pub fn parse_mark(s: &str) -> Result<(u64, String), String> {
    let (frame_str, label) = s
        .split_once('=')
        .ok_or_else(|| format!("--mark: expected <frame>=<label>, got {:?}", s))?;
    let frame = frame_str
        .parse::<u64>()
        .map_err(|_| format!("--mark: frame {:?} is not a valid integer", frame_str))?;
    if label.is_empty() {
        return Err("--mark: label must not be empty".to_owned());
    }
    Ok((frame, label.to_owned()))
}

// ─── Interactive record ───────────────────────────────────────────────────────

/// Options for interactive record mode (headless stub — same fields used by
/// both paths so the compiler always checks the type).
pub struct InteractiveOpts {
    pub rom: std::path::PathBuf,
    pub session_dir: std::path::PathBuf,
    /// Path to the output `.padlog` file (written incrementally).
    pub output_log: std::path::PathBuf,
    /// Replay the existing output log to restore emulator state, then append.
    pub resume: bool,
    /// On resume, downgrade replay-vs-dump divergence from an error to a
    /// warning (the restored state may then not match the recorded session).
    pub skip_replay_verify: bool,
    /// Explicit evdev gamepad node (Linux only). On macOS an explicit path
    /// warns and disables the gamepad for the session (keyboard-only, no
    /// auto-detect fallback — same as Linux when an explicit node fails to
    /// open). `None` auto-detects on both platforms.
    pub gamepad: Option<std::path::PathBuf>,
    /// `--pad-debug`: verbose per-event gamepad diagnostics on stderr (pad
    /// identity/mapping at open, then every button/hat event during
    /// polling). Interactive-only; see `gamepad.rs` and `gamepad_macos.rs`.
    pub pad_debug: bool,
    /// `--no-audio`: skip opening an audio sink entirely (no playback, no
    /// device probing). Interactive-only; see `audio.rs`.
    pub no_audio: bool,
    /// `--stats`: periodic (every 5s) frame/audio diagnostics line plus
    /// per-phase slow-frame attribution on stderr. Interactive-only; see
    /// `StatsWindow` below and beads issue refwork-xkp.
    pub stats: bool,
}

#[cfg(any(feature = "interactive", test))]
fn load_resume_log(path: &std::path::Path, resume: bool) -> Result<PadLog, String> {
    if !resume || !path.exists() {
        return Ok(PadLog::default());
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read resume log {}: {}", path.display(), e))?;
    refwork_script::parse(&text)
        .map_err(|e| format!("cannot parse resume log {}: {}", path.display(), e))
}

/// Count pad lines in an on-disk padlog (anything beyond the header).
///
/// Only "is there recorded input" matters to callers, so RLE lines count as
/// one; the exact frame count comes from parsing, not from here.
#[cfg(any(feature = "interactive", test))]
fn count_pad_lines(path: &std::path::Path) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read log {}: {}", path.display(), e))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "padlog v1")
        .count())
}

/// A fresh (non-resume) interactive run must never start on top of an
/// existing recorded session: truncating the padlog would destroy the
/// recording, and appending dumps next to a rotated-away log would poison
/// future resumes. Session-dir rotation is the wrapper's job.
#[cfg(any(feature = "interactive", test))]
fn ensure_fresh_session(session: &Session, log_path: &std::path::Path) -> Result<(), String> {
    let pad_lines = count_pad_lines(log_path)?;
    if pad_lines == 0 && session.dumps.is_empty() {
        return Ok(());
    }
    Err(format!(
        "session dir already contains a recorded session ({} logged pad lines, {} dumps).\n\
         - to continue it:       re-run with --resume\n\
         - to start over safely: use record-ramdiff (it rotates the whole session dir aside)\n\
         \x20                        or pass a new --session directory\n\
         Nothing was modified.",
        pad_lines,
        session.dumps.len()
    ))
}

/// Validate that a resume log can contain the recorded session.
///
/// Dumps with `frame == 0` are the documented sentinel for platform-captured
/// dumps registered by hand (session.rs module docs); they are not
/// interactive checkpoints and impose no constraint on the log.
#[cfg(any(feature = "interactive", test))]
fn check_resume_integrity(
    log_frames: usize,
    log_exists: bool,
    session_log_frames: Option<u64>,
    dumps: &[DumpMeta],
) -> Result<(), String> {
    let max_dump = dumps
        .iter()
        .filter(|d| d.frame > 0)
        .max_by_key(|d| d.frame);

    if !log_exists {
        if max_dump.is_some() || session_log_frames.unwrap_or(0) > 0 {
            let recorded = match session_log_frames {
                Some(m) => format!(" and a log of {} frames", m),
                None => String::new(),
            };
            return Err(format!(
                "cannot resume: interactive.padlog is missing but session.yaml records \
                 {} dumps{}. The input log for this session is gone; its state cannot be \
                 restored by replay. The WRAM dumps remain valid for `ramdiff search`.",
                dumps.len(),
                recorded
            ));
        }
        return Ok(());
    }

    if let Some(m) = session_log_frames {
        if (log_frames as u64) < m {
            return Err(format!(
                "cannot resume: interactive.padlog holds {} frames but session.yaml recorded \
                 {} frames at the last save. The log tail has been truncated or the file \
                 replaced; resuming would silently restart from the wrong state.",
                log_frames, m
            ));
        }
    }

    if let Some(d) = max_dump {
        if d.frame >= log_frames as u64 {
            return Err(format!(
                "cannot resume: interactive.padlog holds {} frames but dump {:?} was recorded \
                 at frame {}. The log no longer contains the recorded session (it was likely \
                 truncated by an earlier run); resuming would silently restart from the wrong \
                 state.\n\
                 - the WRAM dumps and session.yaml are still valid for `ramdiff search`\n\
                 - to start over, use record-ramdiff (it rotates this session dir aside) or \
                 pass a new --session directory",
                log_frames, d.label, d.frame
            ));
        }
    }
    Ok(())
}

/// Compare replayed WRAM against a recorded dump at its checkpoint frame.
#[cfg(any(feature = "interactive", test))]
fn verify_checkpoint(
    frame: u64,
    label: &str,
    expected: &[u8],
    actual: &[u8],
) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    if expected.len() != actual.len() {
        return Err(format!(
            "replay diverged from the recorded state at frame {} (dump {:?}): dump is {} bytes \
             but WRAM is {} bytes",
            frame,
            label,
            expected.len(),
            actual.len()
        ));
    }
    let mut diff_count = 0usize;
    let mut first_diff = 0usize;
    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        if e != a {
            if diff_count == 0 {
                first_diff = i;
            }
            diff_count += 1;
        }
    }
    Err(format!(
        "replay diverged from the recorded state at frame {} (dump {:?}): {} of {} WRAM bytes \
         differ (first at 0x{:05x}). The emulator's behavior has changed since this session \
         was recorded; the restored state would not match what you played. Re-record the \
         session with the current build, or pass --skip-replay-verify to resume anyway \
         (state may be wrong).",
        frame,
        label,
        diff_count,
        expected.len(),
        first_diff
    ))
}

/// Open the interactive padlog: append when resuming an existing log,
/// otherwise create it with the header.
///
/// The truncating branch is safe only because `ensure_fresh_session` has
/// already proven the file is absent or header-only in fresh mode. The
/// exclusive lock (released automatically at process exit) prevents two
/// ramdiff processes from interleaving writes into one log.
#[cfg(any(feature = "interactive", test))]
fn open_interactive_log(path: &std::path::Path, resume: bool) -> Result<std::fs::File, String> {
    use std::io::Write;

    let append = resume && path.exists();
    let mut open = std::fs::OpenOptions::new();
    open.create(true);
    if append {
        open.append(true);
    } else {
        open.write(true).truncate(true);
    }
    let mut file = open
        .open(path)
        .map_err(|e| format!("cannot open log file {}: {}", path.display(), e))?;
    file.try_lock().map_err(|e| {
        format!(
            "cannot lock log file {} (is another ramdiff running on this session?): {}",
            path.display(),
            e
        )
    })?;
    if !append {
        writeln!(file, "padlog v1").map_err(|e| format!("write error: {}", e))?;
        file.flush().map_err(|e| format!("flush error: {}", e))?;
    }
    Ok(file)
}

// ─── Live-loop diagnostics (beads issue refwork-xkp) ─────────────────────────
//
// Pure logic only — every type here takes `now: Instant` as a parameter so
// tests need no sleeping and no device/window. Gated on `test` as well as the
// feature so the default-features CI run compiles and executes the tests
// (matching `open_interactive_log` above); the audio sink itself cannot get
// that treatment because the whole `audio` module is feature-gated.

/// Interval for both the periodic `--stats` line and the rate limits on the
/// re-prime / slow-frame notes.
#[cfg(any(feature = "interactive", test))]
const DIAG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// Slow-frame threshold. Deliberately well below `audio.rs`'s 100ms cushion:
/// proportional-only rate control settles the queue at ~2/3 of target, so
/// ~70ms stalls already drain it — a 100ms threshold would miss exactly the
/// stalls that cause re-primes. ~3 frame budgets is a real anomaly.
#[cfg(any(feature = "interactive", test))]
const SLOW_FRAME_MS: u64 = 50;

/// Wall-clock frame period for the live loop, chosen for **audio
/// equilibrium**: the core synthesizes
/// `MCLK_PER_FRAME × SPC_NUM / (SPC_DEN × DSP_CLOCKS_PER_SAMPLE)`
/// = 357,368 × 1024 / (21,477 × 32) ≈ 532.466 stereo pairs per frame
/// (those constants are private to `refwork-emu` — `timing.rs` and
/// `apu/mod.rs` — so they are restated here; a timing-model change there
/// must be mirrored, and the `frame_period_matches_audio_production_rate`
/// test pins the relationship). The sink consumes 32,000 pairs/s, so the
/// period is 532.466/32,000 s = 357,368/21,477,000 s ≈ 16.6396ms
/// (~60.098 fps).
///
/// The loop paces itself rather than using minifb's
/// `limit_update_rate`: that limiter is plain `thread::sleep`, which
/// overshoots ~0.8ms/frame on macOS — the measured 57.3 fps / −4.7% audio
/// deficit of refwork-ta9, far beyond the ±0.5% slew authority, draining
/// the queue every ~2.4s into an audible 100ms silence gap.
#[cfg(any(feature = "interactive", test))]
const FRAME_PERIOD: std::time::Duration = std::time::Duration::from_nanos(16_639_568);

/// If pacing falls further behind than this (the F5 prompt, window
/// occlusion), restart the cadence from now instead of fast-forwarding
/// through the backlog at unlimited speed. Note the degenerate case: a
/// host that *persistently* cannot hold 60fps resyncs forever and the
/// pacer never sleeps — pacing degrades to free-run and audio still
/// starves. That is a host-capability limit the pacer cannot fix, only
/// make visible (the --stats fps line will sit below 60).
#[cfg(any(feature = "interactive", test))]
const PACE_RESYNC: std::time::Duration = std::time::Duration::from_millis(100);

/// Margin under the deadline where coarse `thread::sleep` hands over to a
/// spin finish — sleep alone overshoots by about this much, which is the
/// precise error the pacer exists to remove.
#[cfg(any(feature = "interactive", test))]
const PACE_SPIN_MARGIN: std::time::Duration = std::time::Duration::from_micros(1500);

/// Block until `deadline`: coarse sleep to within [`PACE_SPIN_MARGIN`],
/// then spin the rest. Returns immediately for a deadline already passed.
/// The spin finish costs up to ~1.5ms of busy CPU per frame (~9% of the
/// budget) — deliberately traded for the sub-ms pacing precision that
/// `thread::sleep` alone cannot deliver.
#[cfg(any(feature = "interactive", test))]
fn pace_until(deadline: std::time::Instant) {
    loop {
        let now = std::time::Instant::now();
        let Some(remaining) = deadline.checked_duration_since(now) else {
            return;
        };
        if remaining > PACE_SPIN_MARGIN {
            std::thread::sleep(remaining - PACE_SPIN_MARGIN);
        } else {
            std::hint::spin_loop();
        }
    }
}

/// Next frame's deadline after pacing to `deadline`. Absolute-cadence:
/// a small slip is absorbed by the next frame (no drift accumulates), but
/// falling behind by [`PACE_RESYNC`] or more restarts the schedule from
/// `now`. Pure logic, testable with synthetic instants.
#[cfg(any(feature = "interactive", test))]
fn advance_deadline(deadline: std::time::Instant, now: std::time::Instant) -> std::time::Instant {
    if now
        .checked_duration_since(deadline)
        .is_some_and(|late| late >= PACE_RESYNC)
    {
        now + FRAME_PERIOD
    } else {
        deadline + FRAME_PERIOD
    }
}

/// Wall-clock cost of one live-loop iteration, split by phase. `other` is
/// the residual (whole iteration minus the five measured spans): hotkey
/// handling and dump writes — anything in the loop body not explicitly
/// timed, so a stall outside the named phases is still visible instead of
/// being silently misattributed. (The loop-condition poll and the frame
/// pacer's sleep sit outside every span, including this one.)
#[cfg(any(feature = "interactive", test))]
#[derive(Clone, Copy, Default)]
struct PhaseTimes {
    /// `build_pad` + gamepad poll.
    input: std::time::Duration,
    /// `Core::run_one_frame`.
    emu: std::time::Duration,
    /// Audio ring drain + `AudioSink::push` (resampling included).
    audio: std::time::Duration,
    /// Pad-line write + flush (and the cheap fault check preceding it).
    write: std::time::Duration,
    /// Blit + `update_with_buffer` — real work only: frame pacing happens
    /// after the measured spans (see `pace_until` in the live loop), so
    /// neither this nor `total` includes the pacing sleep.
    blit: std::time::Duration,
    /// Residual: total − the five spans above.
    other: std::time::Duration,
}

#[cfg(any(feature = "interactive", test))]
impl PhaseTimes {
    fn total(&self) -> std::time::Duration {
        self.input + self.emu + self.audio + self.write + self.blit + self.other
    }

    /// Name of the phase that dominated this iteration.
    fn dominant(&self) -> &'static str {
        let pairs = [
            ("input", self.input),
            ("emu", self.emu),
            ("audio", self.audio),
            ("write", self.write),
            ("blit", self.blit),
            ("other", self.other),
        ];
        pairs
            .iter()
            .max_by_key(|(_, d)| *d)
            .map(|(name, _)| *name)
            .unwrap_or("other")
    }

    /// Full per-phase breakdown, e.g.
    /// `input 0ms, emu 3ms, audio 1ms, write 0ms, blit 236ms, other 0ms`.
    fn describe(&self) -> String {
        format!(
            "input {}ms, emu {}ms, audio {}ms, write {}ms, blit {}ms, other {}ms",
            self.input.as_millis(),
            self.emu.as_millis(),
            self.audio.as_millis(),
            self.write.as_millis(),
            self.blit.as_millis(),
            self.other.as_millis()
        )
    }
}

/// Minimal 1-per-interval limiter for stderr notes.
#[cfg(any(feature = "interactive", test))]
struct RateLimiter {
    last: Option<std::time::Instant>,
}

#[cfg(any(feature = "interactive", test))]
impl RateLimiter {
    fn new() -> RateLimiter {
        RateLimiter { last: None }
    }

    /// True (and arms the interval) if nothing was allowed in the last
    /// [`DIAG_INTERVAL`]; the first call is always allowed.
    fn allow(&mut self, now: std::time::Instant) -> bool {
        let ok = self
            .last
            .is_none_or(|last| now.duration_since(last) >= DIAG_INTERVAL);
        if ok {
            self.last = Some(now);
        }
        ok
    }
}

/// Rate-limited reporting for audio-queue re-primes (the "audio queue
/// drained" note that used to print unconditionally per event — the log
/// spam half of refwork-xkp). Suppressed events are counted and flushed
/// into the next printed note; the session total lands in the shutdown
/// summary regardless.
#[cfg(any(feature = "interactive", test))]
struct ReprimeReporter {
    limiter: RateLimiter,
    suppressed: u64,
}

#[cfg(any(feature = "interactive", test))]
impl ReprimeReporter {
    fn new() -> ReprimeReporter {
        ReprimeReporter {
            limiter: RateLimiter::new(),
            suppressed: 0,
        }
    }

    /// Report `count` new re-primes (this frame's delta; 0 is a no-op).
    /// Returns the message to print, at most one per [`DIAG_INTERVAL`].
    /// `gap` is the wall-clock time since the previous audio push — the
    /// datum that distinguishes a genuinely stalled frame loop (gap >> one
    /// frame) from a drain the loop never caused (gap ≈ 16ms, e.g. a large
    /// device callback eating a below-target cushion). `after_dump_prompt`
    /// marks the drain as the expected aftermath of the blocking F5 prompt.
    fn note(
        &mut self,
        count: u64,
        gap: Option<std::time::Duration>,
        after_dump_prompt: bool,
        now: std::time::Instant,
    ) -> Option<String> {
        if count == 0 {
            return None;
        }
        if !self.limiter.allow(now) {
            self.suppressed += count;
            return None;
        }
        let mut msg = String::from("interactive: audio queue drained — re-primed with silence");
        if let Some(gap) = gap {
            use std::fmt::Write;
            let _ = write!(msg, " (gap since last push: {}ms)", gap.as_millis());
        }
        if after_dump_prompt {
            msg.push_str(" (after dump prompt — expected)");
        } else {
            msg.push_str(" (stalled frame loop?)");
        }
        if self.suppressed > 0 {
            use std::fmt::Write;
            let _ = write!(msg, " [+{} earlier note(s) suppressed]", self.suppressed);
            self.suppressed = 0;
        }
        Some(msg)
    }
}

/// Accumulates per-iteration timings and emits the periodic `--stats` line.
#[cfg(any(feature = "interactive", test))]
struct StatsWindow {
    window_start: std::time::Instant,
    frames: u64,
    worst: PhaseTimes,
    /// Session-wide (not window) counters for the shutdown summary.
    slow_frames: u64,
    worst_ever_ms: u64,
}

#[cfg(any(feature = "interactive", test))]
impl StatsWindow {
    fn new(now: std::time::Instant) -> StatsWindow {
        StatsWindow {
            window_start: now,
            frames: 0,
            worst: PhaseTimes::default(),
            slow_frames: 0,
            worst_ever_ms: 0,
        }
    }

    /// Record one iteration. `skip_worst` excludes it from worst-frame and
    /// slow-frame accounting (the F5-prompt iteration: a human typing a
    /// label is an intentional block, not a stall — though it still counts
    /// toward fps, because the session really did drop frames).
    fn record(&mut self, phases: &PhaseTimes, skip_worst: bool) {
        self.frames += 1;
        if skip_worst {
            return;
        }
        let total = phases.total();
        if total > self.worst.total() {
            self.worst = *phases;
        }
        let total_ms = total.as_millis() as u64;
        if total_ms > SLOW_FRAME_MS {
            self.slow_frames += 1;
        }
        self.worst_ever_ms = self.worst_ever_ms.max(total_ms);
    }

    /// Whether a stats line is due — callers use this to gather inputs with
    /// side effects (e.g. `AudioSink::take_min_depth_ms` resets the window
    /// minimum) only when [`StatsWindow::maybe_line`] will actually emit.
    fn due(&self, now: std::time::Instant) -> bool {
        self.frames > 0 && now.duration_since(self.window_start) >= DIAG_INTERVAL
    }

    /// Emit the stats line once per [`DIAG_INTERVAL`], then reset the
    /// window. `audio` is `(last_depth_ms, min_depth_ms)` for the window,
    /// `None` when the sink has no device (`--no-audio` / open failure);
    /// `reprimes_window`/`trims_window` are this window's deltas,
    /// `reprimes_total` the session total.
    fn maybe_line(
        &mut self,
        now: std::time::Instant,
        audio: Option<(u32, u32)>,
        reprimes_window: u64,
        reprimes_total: u64,
        trims_window: u64,
    ) -> Option<String> {
        let elapsed = now.duration_since(self.window_start);
        if elapsed < DIAG_INTERVAL || self.frames == 0 {
            return None;
        }
        let fps = self.frames as f64 / elapsed.as_secs_f64();
        let worst_ms = self.worst.total().as_millis();
        let mut msg = format!(
            "interactive: stats: {:.1} fps, worst frame {}ms ({})",
            fps,
            worst_ms,
            self.worst.dominant()
        );
        {
            use std::fmt::Write;
            match audio {
                Some((depth, min)) => {
                    let _ = write!(
                        msg,
                        ", audio depth {}ms (min {}ms), reprimes {} this window ({} total), \
                         trims {}",
                        depth, min, reprimes_window, reprimes_total, trims_window
                    );
                }
                None => msg.push_str(", audio off"),
            }
        }
        self.window_start = now;
        self.frames = 0;
        self.worst = PhaseTimes::default();
        Some(msg)
    }
}

#[cfg(feature = "interactive")]
pub fn run_interactive(opts: &InteractiveOpts) -> Result<(), String> {
    use minifb::{Key, Window, WindowOptions};
    use refwork_emu::{FB_HEIGHT, FB_WIDTH};
    use std::io::Write;

    // Ensure session directory exists.
    std::fs::create_dir_all(&opts.session_dir)
        .map_err(|e| format!("cannot create session dir: {}", e))?;

    let mut session = Session::load(&opts.session_dir)?;

    // Guards before anything is opened or replayed: a refused run must leave
    // every session file untouched.
    let prior_log = load_resume_log(&opts.output_log, opts.resume)?;
    if opts.resume {
        check_resume_integrity(
            prior_log.len(),
            opts.output_log.exists(),
            session.log_frames,
            &session.dumps,
        )?;
    } else {
        ensure_fresh_session(&session, &opts.output_log)?;
    }

    // Load ROM.
    let rom_bytes = std::fs::read(&opts.rom).map_err(|e| format!("cannot read ROM: {}", e))?;
    let cart = Cartridge::from_rom(rom_bytes, None).map_err(|e| format!("bad ROM: {:?}", e))?;

    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    // Resume is deterministic replay: validate and run every recorded input
    // before opening the log for append. A replay fault or a divergence from
    // a recorded dump leaves the log intact.
    if !prior_log.is_empty() {
        // Checkpoints: dumps whose WRAM the replay must reproduce at their
        // recorded frames. Keyed by file with the max-frame entry winning:
        // labels are not unique and distinct labels can sanitize to the same
        // file name, and the .bin on disk holds only the latest dump written
        // to that path. frame == 0 entries are platform captures, not
        // checkpoints.
        let mut by_file: BTreeMap<&str, &DumpMeta> = BTreeMap::new();
        for d in session.dumps.iter().filter(|d| d.frame > 0) {
            if let Some(prev) = by_file.get(d.file.as_str()) {
                let (shadowed, kept) = if d.frame > prev.frame {
                    (*prev, d)
                } else {
                    (d, *prev)
                };
                eprintln!(
                    "interactive: note: dump {:?} (frame {}) shares file {:?} with a later \
                     dump; it cannot be verified during replay",
                    shadowed.label, shadowed.frame, shadowed.file
                );
                by_file.insert(kept.file.as_str(), kept);
            } else {
                by_file.insert(d.file.as_str(), d);
            }
        }
        let checkpoints: BTreeMap<u64, &DumpMeta> =
            by_file.values().map(|d| (d.frame, *d)).collect();

        eprintln!(
            "interactive: replaying {} frames to restore session state ({} checkpoints)",
            prior_log.len(),
            checkpoints.len()
        );
        for (index, &pad) in prior_log.frames.iter().enumerate() {
            let flags = core.run_one_frame(pad);
            if let Some(fault) = core.fault() {
                return Err(format!(
                    "cannot resume: replay fault at frame {} (flags={:?}): {:?}",
                    index, flags, fault
                ));
            }
            // A dump tagged frame F was taken right after the live loop ran
            // pad index F, so compare here, after this frame.
            if let Some(dump) = checkpoints.get(&(index as u64)) {
                let expected = session.load_dump_bytes_for(dump)?;
                match verify_checkpoint(index as u64, &dump.label, &expected, core.wram()) {
                    Ok(()) => eprintln!(
                        "interactive: replay checkpoint OK at frame {} ({:?})",
                        index, dump.label
                    ),
                    Err(e) if opts.skip_replay_verify => {
                        eprintln!("interactive: warning: {}", e);
                    }
                    Err(e) => return Err(format!("cannot resume: {}", e)),
                }
            }
            if (index + 1).is_multiple_of(10_000) {
                eprintln!("interactive: replayed {} frames", index + 1);
            }
        }
        eprintln!("interactive: resumed at frame {}", prior_log.len());
    }

    let mut log_file = open_interactive_log(&opts.output_log, opts.resume)?;

    const BASE_TITLE: &str = "ramdiff record [interactive] — F5=dump, M=mute, Esc=quit";
    let mut window = Window::new(
        BASE_TITLE,
        FB_WIDTH,
        FB_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| format!("cannot open window: {}", e))?;

    // Frame pacing is ours, not minifb's (refwork-ta9): its sleep-based
    // limiter overshoots ~0.8ms/frame on macOS (57.3 fps measured), a −4.7%
    // audio deficit that drained the queue into a 100ms silence gap every
    // ~2.4s. The loop paces itself against [`FRAME_PERIOD`] instead — see
    // the pacer constants above and `pace_until` at the loop's end.
    window.limit_update_rate(None);

    // Audio: construct only after replay has finished (replay must never be
    // audible) and after the window exists — the device starts consuming the
    // 100ms silence cushion the moment the stream opens, so opening it
    // before the (potentially slow, cold-start) window creation would
    // guarantee a spurious drained-queue event before the first live frame.
    // `--no-audio` skips device construction entirely —
    // `AudioSink::disabled()` is the same never-fails no-op sink a real
    // device error would degrade to.
    let mut audio_sink = if opts.no_audio {
        crate::audio::AudioSink::disabled()
    } else {
        crate::audio::AudioSink::open()
    };
    // The replay above ran the core at full speed with no realtime pacing,
    // so any audio it produced has no relationship to wall-clock playback.
    // Drain and discard it so a resumed session does not play a stale burst
    // at startup; a single drain call only moves one buffer's worth, so
    // loop until the ring reports empty.
    let mut audio_scratch = [0i16; 4096];
    loop {
        if core.take_audio_samples(&mut audio_scratch) == 0 {
            break;
        }
    }
    // Baseline for the shutdown summary: a resumed session legitimately
    // overflows the ring during replay (nothing drains it during that
    // phase), so only the delta accrued during the *live* loop is reported.
    let audio_dropped_baseline = core.audio_dropped_pairs();

    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb_xrgb: Box<[u8; refwork_emu::FB_BYTES]> = Box::new([0u8; refwork_emu::FB_BYTES]);
    // minifb expects u32 XRGB8888 in native endian.
    let mut fb_u32 = vec![0u32; FB_WIDTH * FB_HEIGHT];

    let mut frame = prior_log.len() as u64;

    // Optional gamepad (evdev on Linux, gilrs on macOS): merged with the
    // keyboard via OR. Both backends expose the same surface.
    #[cfg(target_os = "linux")]
    use crate::gamepad as pad_backend;
    #[cfg(target_os = "macos")]
    use crate::gamepad_macos as pad_backend;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let mut gamepad = match &opts.gamepad {
        Some(path) => match pad_backend::Gamepad::open_path(path, opts.pad_debug) {
            Ok(g) => {
                eprintln!("interactive: gamepad {}", g.description);
                Some(g)
            }
            Err(e) => {
                eprintln!("interactive: {} - keyboard only", e);
                None
            }
        },
        None => match pad_backend::Gamepad::open_auto(opts.pad_debug) {
            Ok(Some(g)) => {
                eprintln!("interactive: gamepad {}", g.description);
                Some(g)
            }
            Ok(None) => {
                eprintln!("interactive: no gamepad detected - keyboard only");
                None
            }
            Err(e) => {
                eprintln!("interactive: {} - keyboard only", e);
                None
            }
        },
    };

    // Live-loop diagnostics (refwork-xkp): per-phase wall-clock timing,
    // rate-limited re-prime notes, optional periodic `--stats` line. The
    // checkpoints below cost a handful of `Instant::now()` calls per frame —
    // noise against the 16.7ms budget.
    let mut stats = StatsWindow::new(std::time::Instant::now());
    let mut reprime_reporter = ReprimeReporter::new();
    let mut slow_frame_limiter = RateLimiter::new();
    // End of the previous iteration's audio push — the re-prime note's "gap
    // since last push" is measured against this.
    let mut last_push_done: Option<std::time::Instant> = None;
    // Whether the previous iteration ran the blocking F5 dump prompt: the
    // drain it causes is only *detected* by the next iteration's push, so
    // that re-prime must be attributed to the prompt, not to a stall.
    let mut prompt_last_iter = false;
    let mut reprimes_after_prompt: u64 = 0;
    // `--stats` window baselines for per-window deltas.
    let mut reprimes_window_base: u64 = 0;
    let mut trims_window_base: u64 = 0;

    // First frame-pacing deadline (see the pacer constants above).
    let mut next_deadline = std::time::Instant::now() + FRAME_PERIOD;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let t_start = std::time::Instant::now();
        // Build pad from current key state, merged with the gamepad if any.
        #[allow(unused_mut)]
        let mut pad = build_pad(&window);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if let Some(g) = gamepad.as_mut() {
            pad |= g.poll();
        }
        let t_input = std::time::Instant::now();

        let flags = core.run_one_frame(pad);
        let t_emu = std::time::Instant::now();

        // Drain this frame's synthesized audio and hand it to the sink.
        // Looped: the scratch buffer is smaller than a full frame's worth
        // could theoretically be if the sink fell behind, so keep draining
        // until the ring reports empty.
        let reprimes_before = audio_sink.reprimes();
        loop {
            let n = core.take_audio_samples(&mut audio_scratch);
            if n == 0 {
                break;
            }
            audio_sink.push(&audio_scratch[..n]);
        }
        let t_audio = std::time::Instant::now();
        let reprime_delta = audio_sink.reprimes() - reprimes_before;

        if let Some(fault) = core.fault() {
            // This frame's pad line was not written yet, so the log holds
            // exactly `frame` frames.
            session.log_frames = Some(frame);
            session.save()?;
            return Err(format!(
                "interactive: fault at frame {} {:?}: {:?}",
                frame, flags, fault
            ));
        }

        // Append pad word to log (one hex line, no RLE).
        writeln!(log_file, "{:04x}", pad).map_err(|e| format!("write error: {}", e))?;
        log_file
            .flush()
            .map_err(|e| format!("flush error: {}", e))?;
        let t_write = std::time::Instant::now();

        // Blit to window.
        core.blit_completed_frame(&mut fb_xrgb);
        xrgb_to_u32(&fb_xrgb[..], &mut fb_u32, FB_WIDTH, FB_HEIGHT);
        window
            .update_with_buffer(&fb_u32, FB_WIDTH, FB_HEIGHT)
            .map_err(|e| format!("window update: {}", e))?;
        let t_blit = std::time::Instant::now();

        let mut dump_prompt_this_iter = false;

        // F5 hotkey: dump WRAM.
        if window.is_key_pressed(Key::F5, minifb::KeyRepeat::No) {
            dump_prompt_this_iter = true;
            eprint!("interactive: dump label: ");
            let _ = std::io::stderr().flush();
            let mut label = String::new();
            let _ = std::io::stdin().read_line(&mut label);
            let label = label.trim().to_owned();
            if !label.is_empty() {
                let wram_ref: &[u8; WRAM_SIZE] = core.wram();
                let safe: String = label
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let file = format!("{}.bin", safe);
                session.write_dump(&file, wram_ref)?;
                session.add_dump(DumpMeta {
                    label: label.clone(),
                    frame,
                    file,
                    region: "wram".to_owned(),
                });
                // This frame's pad line is already written: frame + 1 total.
                session.log_frames = Some(frame + 1);
                session.save()?;
                eprintln!("interactive: WRAM dumped at frame {} → {:?}", frame, label);
            }
        }

        // M hotkey: toggle audio mute. Host-side only — never touches the
        // pad word, the padlog, or the core.
        if window.is_key_pressed(Key::M, minifb::KeyRepeat::No) {
            let now_muted = !audio_sink.muted();
            audio_sink.set_muted(now_muted);
            if now_muted {
                window.set_title(&format!("{} [muted]", BASE_TITLE));
            } else {
                window.set_title(BASE_TITLE);
            }
        }

        frame += 1;

        // End-of-iteration diagnostics. `other` picks up everything the
        // named spans don't (F5/M hotkey handling incl. the dump write) so
        // a stall there is attributed instead of vanishing.
        let t_end = std::time::Instant::now();
        let phases = PhaseTimes {
            input: t_input.duration_since(t_start),
            emu: t_emu.duration_since(t_input),
            audio: t_audio.duration_since(t_emu),
            write: t_write.duration_since(t_audio),
            blit: t_blit.duration_since(t_write),
            other: t_end.duration_since(t_blit),
        };
        // Re-prime note (rate-limited): emitted here, outside every measured
        // span, so the note's own eprintln cost can never show up as a fake
        // "write"/"audio" stall in the very diagnostics it belongs to. The
        // gap is measured from the previous iteration's push to this one's
        // (≈ how long the loop was away), not from note to note.
        if reprime_delta > 0 {
            if prompt_last_iter {
                reprimes_after_prompt += reprime_delta;
            }
            let gap = last_push_done.map(|t| t_emu.duration_since(t));
            if let Some(msg) = reprime_reporter.note(reprime_delta, gap, prompt_last_iter, t_end)
            {
                eprintln!("{}", msg);
            }
        }
        last_push_done = Some(t_audio);

        stats.record(&phases, dump_prompt_this_iter);
        if opts.stats {
            // Slow-frame attribution is keyed to this iteration's own
            // wall-clock only: a re-prime observed here was caused by the
            // *previous* iteration (detection is one push late), so printing
            // this frame's breakdown for it would attribute the stall to an
            // innocent frame. Sub-threshold drains are covered by the
            // re-prime note's gap and the stats line's min depth instead.
            let total_ms = phases.total().as_millis() as u64;
            if !dump_prompt_this_iter
                && total_ms > SLOW_FRAME_MS
                && slow_frame_limiter.allow(t_end)
            {
                eprintln!(
                    "interactive: slow frame: {}ms ({})",
                    total_ms,
                    phases.describe()
                );
            }
            if stats.due(t_end) {
                // `take_min_depth_ms` resets the window minimum, so only
                // call it when a line is actually due.
                let audio_diag = match (audio_sink.depth_ms(), audio_sink.take_min_depth_ms()) {
                    (Some(depth), Some(min)) => Some((depth, min)),
                    _ => None,
                };
                let reprimes_total = audio_sink.reprimes();
                let trims_total = audio_sink.watermark_drops();
                if let Some(line) = stats.maybe_line(
                    t_end,
                    audio_diag,
                    reprimes_total - reprimes_window_base,
                    reprimes_total,
                    trims_total - trims_window_base,
                ) {
                    eprintln!("{}", line);
                }
                reprimes_window_base = reprimes_total;
                trims_window_base = trims_total;
            }
        }
        prompt_last_iter = dump_prompt_this_iter;

        // Frame pacing (refwork-ta9): sleep+spin to the absolute deadline,
        // deliberately outside every measured span so `worst frame` and the
        // slow-frame line reflect real work, not the pacer's idle time.
        pace_until(next_deadline);
        next_deadline = advance_deadline(next_deadline, std::time::Instant::now());
    }

    // `frame` was incremented past the last written line: it equals the
    // total pad lines now in the log.
    session.log_frames = Some(frame);
    session.save()?;

    // Shutdown diagnostic: all counters reflect only the live loop (the
    // audio baseline was captured after replay, before any live frame ran;
    // the sink itself only exists for the live loop). This is also the
    // final flush for re-primes whose individual notes were rate-limited
    // away.
    let watermark_drops = audio_sink.watermark_drops();
    let reprimes = audio_sink.reprimes();
    let dropped_pairs = core
        .audio_dropped_pairs()
        .saturating_sub(audio_dropped_baseline);
    if watermark_drops > 0 || dropped_pairs > 0 || reprimes > 0 {
        eprintln!(
            "interactive: audio: {} watermark trim(s), {} pair(s) dropped by ring overflow, \
             {} queue re-prime(s) ({} attributed to the dump prompt) during this session",
            watermark_drops, dropped_pairs, reprimes, reprimes_after_prompt
        );
    }
    if stats.slow_frames > 0 {
        eprintln!(
            "interactive: {} frame(s) over {}ms this session, worst {}ms{}",
            stats.slow_frames,
            SLOW_FRAME_MS,
            stats.worst_ever_ms,
            if opts.stats {
                ""
            } else {
                " — rerun with --stats for per-phase attribution"
            }
        );
    }

    Ok(())
}

/// Build a pad word from the current window key state.
///
/// API.md §3.4 bit layout (bit 0..11): A B X Y L R Up Down Left Right Start Select
///
/// Key mapping:
/// - X key → A (bit 0)
/// - Z key → B (bit 1)
/// - A key → X button (bit 2)
/// - S key → Y button (bit 3)
/// - Q key → L (bit 4)
/// - W key → R (bit 5)
/// - Up arrow → Up (bit 6)
/// - Down arrow → Down (bit 7)
/// - Left arrow → Left (bit 8)
/// - Right arrow → Right (bit 9)
/// - Enter → Start (bit 10)
/// - RShift → Select (bit 11)
#[cfg(feature = "interactive")]
fn build_pad(window: &minifb::Window) -> u16 {
    use minifb::Key;
    let mut pad: u16 = 0;
    if window.is_key_down(Key::X) {
        pad |= 1 << 0;
    } // A
    if window.is_key_down(Key::Z) {
        pad |= 1 << 1;
    } // B
    if window.is_key_down(Key::A) {
        pad |= 1 << 2;
    } // X button
    if window.is_key_down(Key::S) {
        pad |= 1 << 3;
    } // Y button
    if window.is_key_down(Key::Q) {
        pad |= 1 << 4;
    } // L
    if window.is_key_down(Key::W) {
        pad |= 1 << 5;
    } // R
    if window.is_key_down(Key::Up) {
        pad |= 1 << 6;
    } // Up
    if window.is_key_down(Key::Down) {
        pad |= 1 << 7;
    } // Down
    if window.is_key_down(Key::Left) {
        pad |= 1 << 8;
    } // Left
    if window.is_key_down(Key::Right) {
        pad |= 1 << 9;
    } // Right
    if window.is_key_down(Key::Enter) {
        pad |= 1 << 10;
    } // Start
    if window.is_key_down(Key::RightShift) {
        pad |= 1 << 11;
    } // Select
    pad
}

/// Convert XRGB8888 framebuffer bytes to minifb's u32 slice.
/// minifb expects each u32 as 0x00RRGGBB (native endian, X byte ignored).
/// The emulator buffer stores little-endian 0x00RRGGBB as `[B, G, R, X]`.
#[cfg(feature = "interactive")]
fn xrgb_to_u32(src: &[u8], dst: &mut [u32], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..width {
            let base = (y * width + x) * 4;
            let b = src[base];
            let g = src[base + 1];
            let r = src[base + 2];
            dst[y * width + x] = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
    }
}

/// Stub that always fails when the `interactive` feature is disabled.
#[cfg(not(feature = "interactive"))]
pub fn run_interactive(_opts: &InteractiveOpts) -> Result<(), String> {
    Err("interactive mode is not compiled in; rebuild with --features interactive".to_owned())
}

// ─── Watch (replay, print value changes) ────────────────────────────────────

/// Options for `ramdiff watch`.
pub struct WatchOpts {
    pub rom: std::path::PathBuf,
    pub script: std::path::PathBuf,
    /// `"wram:<offset_hex_or_dec>"` format.
    pub addr: WatchAddr,
    pub width: crate::session::SearchWidth,
}

pub struct WatchAddr {
    pub region: String,
    pub offset: u32,
}

/// Parse `"wram:0x1234"` or `"wram:4660"` style address.
pub fn parse_watch_addr(s: &str) -> Result<WatchAddr, String> {
    let (region, offset_str) = s
        .split_once(':')
        .ok_or_else(|| format!("--addr: expected <region>:<offset>, got {:?}", s))?;
    let offset = if offset_str.starts_with("0x") || offset_str.starts_with("0X") {
        u32::from_str_radix(&offset_str[2..], 16)
            .map_err(|_| format!("--addr: bad hex offset {:?}", offset_str))?
    } else {
        offset_str
            .parse::<u32>()
            .map_err(|_| format!("--addr: bad decimal offset {:?}", offset_str))?
    };
    Ok(WatchAddr {
        region: region.to_owned(),
        offset,
    })
}

/// Run `ramdiff watch`: replay and print value at `addr` whenever it changes.
pub fn run_watch(opts: &WatchOpts) -> Result<(), String> {
    let rom_bytes = std::fs::read(&opts.rom).map_err(|e| format!("cannot read ROM: {}", e))?;
    let cart = Cartridge::from_rom(rom_bytes, None).map_err(|e| format!("bad ROM: {:?}", e))?;

    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core =
        Core::new(cart, regions).map_err(|e| format!("core construction failed: {:?}", e))?;

    let script_text =
        std::fs::read_to_string(&opts.script).map_err(|e| format!("cannot read script: {}", e))?;
    let pad_log =
        refwork_script::parse(&script_text).map_err(|e| format!("cannot parse script: {}", e))?;

    let total = if pad_log.is_empty() {
        0
    } else {
        pad_log.len() as u64
    };

    if opts.addr.region != "wram" {
        return Err(format!(
            "watch: only 'wram' region is supported; got {:?}",
            opts.addr.region
        ));
    }

    let width = opts.width;
    let offset = opts.addr.offset;
    let byte_size = width.byte_size();
    if (offset as usize) + byte_size > WRAM_SIZE {
        return Err(format!(
            "watch: offset 0x{:x} + {} exceeds WRAM size",
            offset, byte_size
        ));
    }

    let mut prev: Option<u32> = None;

    for frame in 0u64..total {
        let pad = crate::record::get_pad(&pad_log, frame);
        let flags = core.run_one_frame(pad);
        if let Some(fault) = core.fault() {
            eprintln!("watch: fault at frame {} {:?}: {:?}", frame, flags, fault);
            break;
        }
        let val = width.read_value(core.wram(), offset);
        match prev {
            None => {
                println!("frame {:6}: {:?} = {}", frame, opts.addr.region, val);
                prev = Some(val);
            }
            Some(p) if p != val => {
                println!("frame {:6}: {:?} {}→{}", frame, opts.addr.region, p, val);
                prev = Some(val);
            }
            _ => {}
        }
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mark_valid() {
        assert_eq!(
            parse_mark("100=after-intro").unwrap(),
            (100, "after-intro".to_owned())
        );
        assert_eq!(parse_mark("0=start").unwrap(), (0, "start".to_owned()));
    }

    #[test]
    fn parse_mark_errors() {
        assert!(parse_mark("noint=label").is_err());
        assert!(parse_mark("100").is_err());
        assert!(parse_mark("100=").is_err());
    }

    #[test]
    fn get_pad_holds_last() {
        let log = PadLog::from_frames(vec![0x0001, 0x0002, 0x0003]).unwrap();
        assert_eq!(get_pad(&log, 0), 0x0001);
        assert_eq!(get_pad(&log, 2), 0x0003);
        // Past end — hold last.
        assert_eq!(get_pad(&log, 100), 0x0003);
    }

    #[test]
    fn get_pad_empty_log() {
        let log = PadLog::from_frames(vec![]).unwrap();
        assert_eq!(get_pad(&log, 0), 0);
        assert_eq!(get_pad(&log, 99), 0);
    }

    #[test]
    fn resume_log_is_loaded_only_when_requested() {
        let temp = std::env::temp_dir().join(format!(
            "ramdiff-resume-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&temp, "padlog v1\n0001\n2x0002\n").unwrap();

        let fresh = load_resume_log(&temp, false).unwrap();
        assert!(fresh.is_empty());
        let resumed = load_resume_log(&temp, true).unwrap();
        assert_eq!(resumed.frames, vec![1, 2, 2]);

        std::fs::remove_file(temp).unwrap();
    }

    #[test]
    fn resume_log_open_appends_without_rewriting_header() {
        use std::io::Write;

        let temp = std::env::temp_dir().join(format!(
            "ramdiff-append-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&temp, "padlog v1\n0001\n").unwrap();

        let mut file = open_interactive_log(&temp, true).unwrap();
        writeln!(file, "0002").unwrap();
        drop(file);

        let parsed = refwork_script::parse(&std::fs::read_to_string(&temp).unwrap()).unwrap();
        assert_eq!(parsed.frames, vec![1, 2]);
        std::fs::remove_file(temp).unwrap();
    }

    fn dump(label: &str, frame: u64, file: &str) -> DumpMeta {
        DumpMeta {
            label: label.to_owned(),
            frame,
            file: file.to_owned(),
            region: "wram".to_owned(),
        }
    }

    fn temp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ramdiff-{}-{}-{}",
            tag,
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn fresh_session_refuses_existing_pad_lines() {
        let log = temp_path("fresh-padlines");
        std::fs::write(&log, "padlog v1\n0001\n").unwrap();
        let session = Session::new(std::env::temp_dir());

        let err = ensure_fresh_session(&session, &log).unwrap_err();
        assert!(err.contains("--resume"), "err: {}", err);
        // The refused run must not modify the file.
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "padlog v1\n0001\n"
        );
        std::fs::remove_file(log).unwrap();
    }

    #[test]
    fn fresh_session_refuses_existing_dumps() {
        let log = temp_path("fresh-dumps");
        let _ = std::fs::remove_file(&log);
        let mut session = Session::new(std::env::temp_dir());
        session.add_dump(dump("boss", 100, "boss.bin"));

        assert!(ensure_fresh_session(&session, &log).is_err());
    }

    #[test]
    fn fresh_session_accepts_clean_dir() {
        let log = temp_path("fresh-clean");
        let session = Session::new(std::env::temp_dir());

        // Missing file.
        let _ = std::fs::remove_file(&log);
        assert!(ensure_fresh_session(&session, &log).is_ok());

        // Header-only file (aborted start).
        std::fs::write(&log, "padlog v1\n").unwrap();
        assert!(ensure_fresh_session(&session, &log).is_ok());
        std::fs::remove_file(log).unwrap();
    }

    #[test]
    fn resume_integrity_detects_truncated_log() {
        // The real incident: dump at frame 77146, log rewritten to 8605 frames.
        let dumps = vec![dump("1-4 boss defeated", 77146, "boss.bin")];
        let err = check_resume_integrity(8605, true, None, &dumps).unwrap_err();
        assert!(err.contains("77146"), "err: {}", err);
        assert!(err.contains("8605"), "err: {}", err);
        assert!(err.contains("1-4 boss defeated"), "err: {}", err);
    }

    #[test]
    fn resume_integrity_frame_boundaries() {
        let dumps = vec![dump("d", 77146, "d.bin")];
        // Dump at frame F needs >= F + 1 logged frames.
        assert!(check_resume_integrity(77147, true, None, &dumps).is_ok());
        assert!(check_resume_integrity(77146, true, None, &dumps).is_err());
    }

    #[test]
    fn resume_integrity_names_worst_dump() {
        let dumps = vec![
            dump("early", 100, "early.bin"),
            dump("late", 5000, "late.bin"),
            dump("mid", 2000, "mid.bin"),
        ];
        let err = check_resume_integrity(3000, true, None, &dumps).unwrap_err();
        assert!(err.contains("\"late\""), "err: {}", err);
    }

    #[test]
    fn resume_integrity_ignores_platform_capture_sentinel() {
        // frame == 0 marks hand-registered platform captures.
        let dumps = vec![dump("external", 0, "external.bin")];
        assert!(check_resume_integrity(0, true, None, &dumps).is_ok());
        assert!(check_resume_integrity(10, true, None, &dumps).is_ok());

        // A real dump alongside the sentinel still governs.
        let dumps = vec![dump("external", 0, "external.bin"), dump("real", 50, "real.bin")];
        assert!(check_resume_integrity(51, true, None, &dumps).is_ok());
        assert!(check_resume_integrity(50, true, None, &dumps).is_err());
    }

    #[test]
    fn resume_integrity_empty_session_is_ok() {
        assert!(check_resume_integrity(0, false, None, &[]).is_ok());
        assert!(check_resume_integrity(1000, true, None, &[]).is_ok());
    }

    #[test]
    fn resume_integrity_missing_log_with_dumps() {
        let dumps = vec![dump("boss", 100, "boss.bin")];
        let err = check_resume_integrity(0, false, None, &dumps).unwrap_err();
        assert!(err.contains("missing"), "err: {}", err);
        assert!(!err.contains("truncated"), "err: {}", err);
    }

    #[test]
    fn resume_integrity_uses_recorded_frame_count() {
        // Tail truncation past the last dump: no dump violated, but the
        // session recorded more frames than the log now holds.
        let dumps = vec![dump("d", 100, "d.bin")];
        assert!(check_resume_integrity(900, true, Some(1000), &dumps).is_err());
        assert!(check_resume_integrity(1000, true, Some(1000), &dumps).is_ok());
    }

    #[test]
    fn verify_checkpoint_matches_and_diverges() {
        let a = vec![0u8; 64];
        assert!(verify_checkpoint(7, "ok", &a, &a).is_ok());

        let mut b = a.clone();
        b[9] = 0xff;
        let err = verify_checkpoint(7, "bad", &a, &b).unwrap_err();
        assert!(err.contains("frame 7"), "err: {}", err);
        assert!(err.contains("\"bad\""), "err: {}", err);
        assert!(err.contains("1 of 64"), "err: {}", err);
        assert!(err.contains("0x00009"), "err: {}", err);

        let short = vec![0u8; 32];
        assert!(verify_checkpoint(7, "len", &a, &short).is_err());
    }

    #[test]
    fn open_interactive_log_creates_fresh_with_header() {
        let log = temp_path("open-fresh");
        let _ = std::fs::remove_file(&log);
        let file = open_interactive_log(&log, false).unwrap();
        drop(file);
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "padlog v1\n");

        // Resume on a missing file also starts fresh with a header.
        std::fs::remove_file(&log).unwrap();
        let file = open_interactive_log(&log, true).unwrap();
        drop(file);
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "padlog v1\n");
        std::fs::remove_file(log).unwrap();
    }

    #[test]
    fn open_interactive_log_rejects_second_locker() {
        let log = temp_path("open-lock");
        let _ = std::fs::remove_file(&log);
        let first = open_interactive_log(&log, false).unwrap();
        let second = open_interactive_log(&log, true);
        assert!(second.is_err(), "second open must fail while locked");
        drop(first);
        std::fs::remove_file(log).unwrap();
    }

    #[cfg(feature = "interactive")]
    #[test]
    fn interactive_framebuffer_conversion_preserves_rgb_channels() {
        let src = [0x33, 0x22, 0x11, 0x00, 0xcc, 0xbb, 0xaa, 0x00];
        let mut dst = [0u32; 2];
        xrgb_to_u32(&src, &mut dst, 2, 1);
        assert_eq!(dst, [0x0011_2233, 0x00aa_bbcc]);
    }

    #[test]
    fn parse_watch_addr() {
        let a = super::parse_watch_addr("wram:0x0010").unwrap();
        assert_eq!(a.region, "wram");
        assert_eq!(a.offset, 0x0010);
        let b = super::parse_watch_addr("wram:16").unwrap();
        assert_eq!(b.offset, 16);
    }

    // ─── Live-loop diagnostics (refwork-xkp) ─────────────────────────────
    //
    // `Instant` has no synthetic constructor; all instants are one
    // `Instant::now()` base plus `Duration` offsets. Note these tests cover
    // the pure decide/format logic only — the wiring inside the live loop
    // (and `AudioSink::push` counting) needs a device and a window, and is
    // exercised by the follow-up live-run bead, not here.

    fn ms(n: u64) -> std::time::Duration {
        std::time::Duration::from_millis(n)
    }

    #[test]
    fn rate_limiter_allows_first_then_gates_on_interval() {
        let base = std::time::Instant::now();
        let mut rl = RateLimiter::new();
        assert!(rl.allow(base));
        assert!(!rl.allow(base + ms(4_999)));
        assert!(rl.allow(base + ms(5_000)));
        assert!(!rl.allow(base + ms(5_001)));
    }

    #[test]
    fn reprime_reporter_first_event_logs_immediately() {
        let base = std::time::Instant::now();
        let mut rep = ReprimeReporter::new();
        assert_eq!(rep.note(0, None, false, base), None);
        let msg = rep.note(1, Some(ms(240)), false, base).unwrap();
        assert!(msg.contains("audio queue drained"), "{msg}");
        assert!(msg.contains("gap since last push: 240ms"), "{msg}");
        assert!(msg.contains("stalled frame loop?"), "{msg}");
        assert!(!msg.contains("suppressed"), "{msg}");
    }

    #[test]
    fn reprime_reporter_suppresses_within_interval_then_flushes_count() {
        let base = std::time::Instant::now();
        let mut rep = ReprimeReporter::new();
        assert!(rep.note(1, None, false, base).is_some());
        assert_eq!(rep.note(1, None, false, base + ms(1_000)), None);
        assert_eq!(rep.note(2, None, false, base + ms(2_000)), None);
        let msg = rep.note(1, None, false, base + ms(6_000)).unwrap();
        assert!(msg.contains("[+3 earlier note(s) suppressed]"), "{msg}");
        // The suppressed counter resets once flushed.
        let msg = rep.note(1, None, false, base + ms(12_000)).unwrap();
        assert!(!msg.contains("suppressed"), "{msg}");
    }

    #[test]
    fn reprime_reporter_annotates_dump_prompt_drains() {
        let base = std::time::Instant::now();
        let mut rep = ReprimeReporter::new();
        let msg = rep.note(1, Some(ms(30_000)), true, base).unwrap();
        assert!(msg.contains("after dump prompt — expected"), "{msg}");
        assert!(!msg.contains("stalled frame loop"), "{msg}");
    }

    #[test]
    fn phase_times_dominant_and_describe() {
        let p = PhaseTimes {
            input: ms(0),
            emu: ms(3),
            audio: ms(1),
            write: ms(0),
            blit: ms(236),
            other: ms(0),
        };
        assert_eq!(p.total(), ms(240));
        assert_eq!(p.dominant(), "blit");
        assert_eq!(
            p.describe(),
            "input 0ms, emu 3ms, audio 1ms, write 0ms, blit 236ms, other 0ms"
        );
        // A stall outside the named spans lands in (and is attributed to)
        // the residual bucket.
        let p2 = PhaseTimes {
            other: ms(200),
            ..PhaseTimes::default()
        };
        assert_eq!(p2.dominant(), "other");
    }

    #[test]
    fn stats_window_emits_once_per_interval_and_resets() {
        let base = std::time::Instant::now();
        let mut sw = StatsWindow::new(base);
        let frame = PhaseTimes {
            emu: ms(16),
            ..PhaseTimes::default()
        };
        for _ in 0..300 {
            sw.record(&frame, false);
        }
        assert!(!sw.due(base + ms(4_999)));
        assert!(sw.due(base + ms(5_000)));
        let line = sw
            .maybe_line(base + ms(5_000), Some((96, 41)), 1, 3, 0)
            .unwrap();
        assert!(line.contains("60.0 fps"), "{line}");
        assert!(line.contains("worst frame 16ms (emu)"), "{line}");
        assert!(line.contains("audio depth 96ms (min 41ms)"), "{line}");
        assert!(line.contains("reprimes 1 this window (3 total)"), "{line}");
        // The window reset: nothing due right after an emitted line.
        assert!(!sw.due(base + ms(5_001)));
        assert_eq!(sw.maybe_line(base + ms(5_001), None, 0, 0, 0), None);
    }

    #[test]
    fn stats_window_no_audio_renders_audio_off() {
        let base = std::time::Instant::now();
        let mut sw = StatsWindow::new(base);
        sw.record(&PhaseTimes::default(), false);
        let line = sw.maybe_line(base + ms(5_000), None, 0, 0, 0).unwrap();
        assert!(line.contains("audio off"), "{line}");
        assert!(!line.contains("depth"), "{line}");
    }

    #[test]
    fn stats_window_slow_frame_accounting_skips_prompt_iterations() {
        let base = std::time::Instant::now();
        let mut sw = StatsWindow::new(base);
        // The F5-prompt iteration: an intentional block, not a stall — it
        // must not pollute worst-frame or slow-frame accounting (but still
        // counts toward fps).
        let prompt = PhaseTimes {
            other: ms(30_000),
            ..PhaseTimes::default()
        };
        sw.record(&prompt, true);
        assert_eq!(sw.frames, 1);
        assert_eq!(sw.slow_frames, 0);
        assert_eq!(sw.worst_ever_ms, 0);
        assert_eq!(sw.worst.total(), ms(0));
        // A genuinely slow frame is counted and remembered.
        let slow = PhaseTimes {
            blit: ms(240),
            ..PhaseTimes::default()
        };
        sw.record(&slow, false);
        assert_eq!(sw.slow_frames, 1);
        assert_eq!(sw.worst_ever_ms, 240);
        // A normal 16ms frame is not slow.
        let normal = PhaseTimes {
            emu: ms(16),
            ..PhaseTimes::default()
        };
        sw.record(&normal, false);
        assert_eq!(sw.slow_frames, 1);
    }

    // ─── Frame pacer (refwork-ta9) ───────────────────────────────────────

    #[test]
    fn frame_period_matches_audio_production_rate() {
        // Audio equilibrium is the whole point of the pacer: at one frame
        // per FRAME_PERIOD, the core's per-frame output
        // (MCLK_PER_FRAME × SPC_NUM / (SPC_DEN × DSP_CLOCKS_PER_SAMPLE)
        //  = 357,368 × 1024 / (21,477 × 32) pairs — the refwork-emu
        // constants restated, see FRAME_PERIOD's doc) must equal the
        // sink's 32,000 pairs/s draw.
        let pairs_per_frame = 357_368.0 * 1024.0 / (21_477.0 * 32.0);
        let produced_per_s = pairs_per_frame / FRAME_PERIOD.as_secs_f64();
        assert!(
            (produced_per_s - 32_000.0).abs() < 0.5,
            "audio production at FRAME_PERIOD is {produced_per_s} pairs/s, want 32000"
        );
    }

    #[test]
    fn advance_deadline_keeps_cadence_and_resyncs_when_far_behind() {
        let base = std::time::Instant::now();
        let deadline = base + FRAME_PERIOD;
        // On time: one period later — absolute cadence, no drift.
        assert_eq!(
            advance_deadline(deadline, deadline),
            deadline + FRAME_PERIOD
        );
        // Early (paced-to before the deadline was reached — cannot really
        // happen, but must not panic or shift the schedule).
        assert_eq!(advance_deadline(deadline, base), deadline + FRAME_PERIOD);
        // Slightly late (< resync threshold): keep the absolute schedule so
        // the next frame absorbs the slip.
        assert_eq!(
            advance_deadline(deadline, deadline + ms(50)),
            deadline + FRAME_PERIOD
        );
        // Exactly at the threshold: resync (the comparison is >=).
        let at_threshold = deadline + PACE_RESYNC;
        assert_eq!(
            advance_deadline(deadline, at_threshold),
            at_threshold + FRAME_PERIOD
        );
        // Far behind (>= resync): restart from now — no fast-forward burst
        // after the F5 prompt or a long occlusion.
        let very_late = deadline + ms(30_000);
        assert_eq!(
            advance_deadline(deadline, very_late),
            very_late + FRAME_PERIOD
        );
    }

    #[test]
    fn pace_until_past_deadline_returns_immediately() {
        let start = std::time::Instant::now();
        pace_until(start);
        assert!(
            start.elapsed() < ms(50),
            "pace_until on an expired deadline must not sleep"
        );
    }
}
