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

    // Open the output log file.  Write header once, then append per frame.
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&opts.output_log)
        .map_err(|e| format!("cannot open log file: {}", e))?;
    writeln!(log_file, "padlog v1").map_err(|e| format!("write error: {}", e))?;
    log_file
        .flush()
        .map_err(|e| format!("flush error: {}", e))?;

    let mut window = Window::new(
        "ramdiff record [interactive] — F5=dump, Esc=quit",
        FB_WIDTH,
        FB_HEIGHT,
        WindowOptions::default(),
    )
    .map_err(|e| format!("cannot open window: {}", e))?;

    // ~60 fps: 16ms per frame.
    window.limit_update_rate(Some(std::time::Duration::from_millis(16)));

    // Boxed: a quarter-MiB by value blows the default test-thread stack.
    let mut fb_xrgb: Box<[u8; refwork_emu::FB_BYTES]> = Box::new([0u8; refwork_emu::FB_BYTES]);
    // minifb expects u32 XRGB8888 in native endian.
    let mut fb_u32 = vec![0u32; FB_WIDTH * FB_HEIGHT];

    let mut frame: u64 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Build pad from current key state.
        let pad = build_pad(&window);

        let flags = core.run_one_frame(pad);
        if let Some(fault) = core.fault() {
            eprintln!(
                "interactive: fault at frame {} {:?}: {:?}",
                frame, flags, fault
            );
            break;
        }

        // Append pad word to log (one hex line, no RLE).
        writeln!(log_file, "{:04x}", pad).map_err(|e| format!("write error: {}", e))?;
        log_file
            .flush()
            .map_err(|e| format!("flush error: {}", e))?;

        // Blit to window.
        core.blit_completed_frame(&mut fb_xrgb);
        xrgb_to_u32(&fb_xrgb, &mut fb_u32, FB_WIDTH, FB_HEIGHT);
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
                session.save()?;
                eprintln!("interactive: WRAM dumped at frame {} → {:?}", frame, label);
            }
        }

        frame += 1;
    }

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
#[cfg(feature = "interactive")]
fn xrgb_to_u32(src: &[u8], dst: &mut [u32], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..width {
            let base = (y * width + x) * 4;
            let _x_byte = src[base];
            let r = src[base + 1];
            let g = src[base + 2];
            let b = src[base + 3];
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
    fn parse_watch_addr() {
        let a = super::parse_watch_addr("wram:0x0010").unwrap();
        assert_eq!(a.region, "wram");
        assert_eq!(a.offset, 0x0010);
        let b = super::parse_watch_addr("wram:16").unwrap();
        assert_eq!(b.offset, 16);
    }
}
