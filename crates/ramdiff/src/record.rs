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
    /// Explicit evdev gamepad node (Linux). `None` enables auto-detection.
    pub gamepad: Option<std::path::PathBuf>,
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

    let mut window = Window::new(
        "ramdiff record [interactive] — F5=dump, Esc=quit",
        FB_WIDTH,
        FB_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| format!("cannot open window: {}", e))?;

    // ~60 fps: 16ms per frame.
    window.limit_update_rate(Some(std::time::Duration::from_millis(16)));

    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb_xrgb: Box<[u8; refwork_emu::FB_BYTES]> = Box::new([0u8; refwork_emu::FB_BYTES]);
    // minifb expects u32 XRGB8888 in native endian.
    let mut fb_u32 = vec![0u32; FB_WIDTH * FB_HEIGHT];

    let mut frame = prior_log.len() as u64;

    // Optional evdev gamepad (Linux): merged with the keyboard via OR.
    #[cfg(target_os = "linux")]
    let mut gamepad = match &opts.gamepad {
        Some(path) => match crate::gamepad::Gamepad::open_path(path) {
            Ok(g) => {
                eprintln!("interactive: gamepad {}", g.description);
                Some(g)
            }
            Err(e) => {
                eprintln!("interactive: {} - keyboard only", e);
                None
            }
        },
        None => match crate::gamepad::Gamepad::open_auto() {
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Build pad from current key state, merged with the gamepad if any.
        #[allow(unused_mut)]
        let mut pad = build_pad(&window);
        #[cfg(target_os = "linux")]
        if let Some(g) = gamepad.as_mut() {
            pad |= g.poll();
        }

        let flags = core.run_one_frame(pad);
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

        // Blit to window.
        core.blit_completed_frame(&mut fb_xrgb);
        xrgb_to_u32(&fb_xrgb[..], &mut fb_u32, FB_WIDTH, FB_HEIGHT);
        window
            .update_with_buffer(&fb_u32, FB_WIDTH, FB_HEIGHT)
            .map_err(|e| format!("window update: {}", e))?;

        // F5 hotkey: dump WRAM.
        if window.is_key_pressed(Key::F5, minifb::KeyRepeat::No) {
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

        frame += 1;
    }

    // `frame` was incremented past the last written line: it equals the
    // total pad lines now in the log.
    session.log_frames = Some(frame);
    session.save()?;
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
}
