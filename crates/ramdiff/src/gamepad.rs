//! Evdev gamepad input for interactive record mode (Linux only).
//!
//! Zero-dependency, unsafe-free reader for `/dev/input/event*`. Built for the
//! Logitech F310 but accepts any pad emitting the same codes. Both of the
//! F310's hardware layouts are mapped simultaneously (their key-code ranges
//! do not overlap, so no mode flag is needed):
//!
//! - **XInput mode** (switch on "X", kernel `xpad` driver): `BTN_SOUTH`(304)=A,
//!   `BTN_EAST`(305)=B, `BTN_NORTH`(307)=X, `BTN_WEST`(308)=Y, `BTN_TL`(310)=L,
//!   `BTN_TR`(311)=R, `BTN_SELECT`(314)=Select, `BTN_START`(315)=Start.
//!   `BTN_TL2`(312)/`BTN_TR2`(313) (the lower triggers) also fold into L/R.
//! - **DirectInput mode** (switch on "D", generic HID driver): the
//!   `BTN_JOYSTICK` range — 288=X, 289=A, 290=B, 291=Y, 292=L(LB), 293=R(RB),
//!   296=Select(Back), 297=Start. Codes 294(LT)/295(RT) also fold into L/R.
//!
//! The trigger fold is unconditional on Linux (unlike the macOS gilrs
//! backend, where it's gated on mapping source — see `gamepad_macos.rs`):
//! evdev key codes are a layout-fixed kernel ABI, not a resolved SDL mapping,
//! so 294/295/312/313 always mean "lower trigger" and never something else.
//! The SNES pad has no lower triggers, so LB+LT → L and RB+RT → R is
//! strictly more usable and cannot conflict with another button.
//!
//! The D-pad is read from the `ABS_HAT0X`/`ABS_HAT0Y` axes, which both
//! drivers emit. Analog sticks are deliberately ignored: the SNES pad has no
//! analog input, and the two drivers report incompatible stick ranges that
//! cannot be told apart without ioctls.
//!
//! Event framing: one `struct input_event` per 24 bytes on 64-bit Linux
//! (`tv_sec: u64`, `tv_usec: u64`, `type: u16`, `code: u16`, `value: i32`,
//! little-endian on every supported target). 32-bit targets (16-byte events)
//! are not supported; both lab machines are 64-bit.
//!
//! # `--pad-debug`
//!
//! When the caller opens the pad with `pad_debug: true`, every decoded
//! `EV_KEY` and hat (`ABS_HAT0X`/`ABS_HAT0Y`) event is printed to stderr as
//! `type/code/value`, plus whether the code mapped to a pad bit — the same
//! diagnostic role the macOS backend's `--pad-debug` output serves.

// The 24-byte event parse below is the 64-bit ABI only; fail loudly on any
// 32-bit Linux target (16-byte events) instead of silently misparsing pad
// input mid-session.
#[cfg(not(target_pointer_width = "64"))]
compile_error!("gamepad.rs assumes the 64-bit struct input_event layout (24 bytes)");

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// `O_NONBLOCK` on Linux (asm-generic). Hardcoded to avoid a libc dependency
/// for a single constant; guarded by the module's `target_os = "linux"` cfg.
const O_NONBLOCK: i32 = 0o4000;

const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;
const EVENT_SIZE: usize = 24;

/// Substrings (lowercase) an auto-detected device name must contain.
const NAME_HINTS: [&str; 4] = ["f310", "dual action", "logitech gamepad", "gamepad"];

/// API.md §3.4 pad bit for an evdev key code, covering both F310 layouts.
fn button_bit(code: u16) -> Option<u16> {
    Some(match code {
        // XInput mode (xpad driver).
        304 => 1 << 0,  // BTN_SOUTH  -> A
        305 => 1 << 1,  // BTN_EAST   -> B
        307 => 1 << 2,  // BTN_NORTH  -> X
        308 => 1 << 3,  // BTN_WEST   -> Y
        310 => 1 << 4,  // BTN_TL     -> L
        311 => 1 << 5,  // BTN_TR     -> R
        312 => 1 << 4,  // BTN_TL2 (lower trigger) -> L, folded (unconditional; see module doc)
        313 => 1 << 5,  // BTN_TR2 (lower trigger) -> R, folded (unconditional; see module doc)
        315 => 1 << 10, // BTN_START  -> Start
        314 => 1 << 11, // BTN_SELECT -> Select
        // DirectInput mode (hid-generic; F310 physical labels).
        289 => 1 << 0,  // A
        290 => 1 << 1,  // B
        288 => 1 << 2,  // X
        291 => 1 << 3,  // Y
        292 => 1 << 4,  // LB -> L
        293 => 1 << 5,  // RB -> R
        294 => 1 << 4,  // LT (lower trigger) -> L, folded (unconditional; see module doc)
        295 => 1 << 5,  // RT (lower trigger) -> R, folded (unconditional; see module doc)
        297 => 1 << 10, // Start
        296 => 1 << 11, // Back -> Select
        _ => return None,
    })
}

/// Decoded state fed by raw evdev packets. Pure logic, unit-tested.
#[derive(Default)]
struct PadState {
    buttons: u16,
    hat_x: i32,
    hat_y: i32,
    /// Carry-over for a partial event straddling two reads.
    partial: Vec<u8>,
}

impl PadState {
    /// Apply one decoded event.
    fn apply(&mut self, ev_type: u16, code: u16, value: i32) {
        match ev_type {
            EV_KEY => {
                if let Some(bit) = button_bit(code) {
                    // value: 0 release, 1 press, 2 auto-repeat (held).
                    if value == 0 {
                        self.buttons &= !bit;
                    } else {
                        self.buttons |= bit;
                    }
                }
            }
            EV_ABS => match code {
                ABS_HAT0X => self.hat_x = value.signum(),
                ABS_HAT0Y => self.hat_y = value.signum(),
                _ => {}
            },
            _ => {}
        }
    }

    /// Consume raw bytes (any length), buffering a trailing partial event.
    ///
    /// `debug`: `--pad-debug` — print each decoded `EV_KEY`/hat event to
    /// stderr as `type/code/value`, plus whether it mapped to a pad bit.
    fn feed(&mut self, bytes: &[u8], debug: bool) {
        self.partial.extend_from_slice(bytes);
        let complete = self.partial.len() / EVENT_SIZE * EVENT_SIZE;
        for start in (0..complete).step_by(EVENT_SIZE) {
            let ev: [u8; EVENT_SIZE] = self.partial[start..start + EVENT_SIZE]
                .try_into()
                .expect("slice is exactly EVENT_SIZE");
            let ev_type = u16::from_le_bytes([ev[16], ev[17]]);
            let code = u16::from_le_bytes([ev[18], ev[19]]);
            let value = i32::from_le_bytes([ev[20], ev[21], ev[22], ev[23]]);
            if debug {
                debug_print_event(ev_type, code, value);
            }
            self.apply(ev_type, code, value);
        }
        self.partial.drain(..complete);
    }

    /// Current API.md §3.4 pad word.
    fn pad_bits(&self) -> u16 {
        let mut pad = self.buttons;
        if self.hat_y < 0 {
            pad |= 1 << 6; // Up
        }
        if self.hat_y > 0 {
            pad |= 1 << 7; // Down
        }
        if self.hat_x < 0 {
            pad |= 1 << 8; // Left
        }
        if self.hat_x > 0 {
            pad |= 1 << 9; // Right
        }
        pad
    }
}

/// An open, non-blocking evdev gamepad.
pub struct Gamepad {
    file: File,
    state: PadState,
    /// Set once a fatal read error was reported; the pad then reads as 0.
    dead: bool,
    /// Human-readable identity for logs: "<name> (/dev/input/eventN)".
    pub description: String,
    /// `--pad-debug`: print every decoded event to stderr in `poll()`.
    pad_debug: bool,
}

impl Gamepad {
    /// Open an explicit evdev node.
    pub fn open_path(path: &Path, pad_debug: bool) -> Result<Self, String> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(O_NONBLOCK)
            .open(path)
            .map_err(|e| format!("cannot open gamepad {}: {}", path.display(), e))?;
        let name = device_name(path).unwrap_or_else(|| "unknown device".to_owned());
        if pad_debug {
            eprintln!("pad-debug: open name={:?} path={}", name, path.display());
        }
        Ok(Gamepad {
            file,
            state: PadState::default(),
            dead: false,
            description: format!("{} ({})", name, path.display()),
            pad_debug,
        })
    }

    /// Scan `/dev/input/event*` for a device whose name suggests a gamepad.
    /// `Ok(None)` when no candidate exists. A candidate that exists but is
    /// unreadable (typically: user not in the `input` group) is an `Err`,
    /// so the caller can print the actionable cause instead of silently
    /// falling back to keyboard-only.
    pub fn open_auto(pad_debug: bool) -> Result<Option<Self>, String> {
        let mut candidate: Option<PathBuf> = None;
        let entries = std::fs::read_dir("/dev/input").map_err(|e| e.to_string())?;
        let mut nodes: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("event"))
            })
            .collect();
        nodes.sort();
        for node in nodes {
            if let Some(name) = device_name(&node) {
                let lower = name.to_lowercase();
                if NAME_HINTS.iter().any(|h| lower.contains(h)) {
                    candidate = Some(node);
                    break;
                }
            }
        }
        match candidate {
            None => Ok(None),
            Some(path) => Self::open_path(&path, pad_debug).map(Some).map_err(|e| {
                if e.contains("ermission denied") {
                    format!(
                        "{} — replug the pad (logind grants a seat ACL to \
                         joystick devices) or add your user to the `input` group",
                        e
                    )
                } else {
                    e
                }
            }),
        }
    }

    /// Drain pending events and return the current pad word (0 if dead).
    pub fn poll(&mut self) -> u16 {
        if self.dead {
            return 0;
        }
        let mut buf = [0u8; EVENT_SIZE * 32];
        loop {
            match self.file.read(&mut buf) {
                // Evdev nodes report device removal as an error (ENODEV/EIO),
                // not EOF, so Ok(0) is treated as "nothing this tick" rather
                // than a dead device.
                Ok(0) => break,
                Ok(n) => self.state.feed(&buf[..n], self.pad_debug),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    eprintln!("gamepad: read failed ({}); continuing keyboard-only", e);
                    self.dead = true;
                    return 0;
                }
            }
        }
        self.state.pad_bits()
    }
}

/// `--pad-debug`: print one decoded `EV_KEY`/hat event to stderr as
/// `type/code/value`, plus whether the code mapped to a pad bit. Other event
/// types (EV_SYN, EV_MSC, ignored EV_ABS axes) are not pad-relevant and are
/// silently skipped, matching `PadState::apply`.
fn debug_print_event(ev_type: u16, code: u16, value: i32) {
    match ev_type {
        EV_KEY => {
            let mapped = button_bit(code).is_some();
            eprintln!(
                "pad-debug: EV_KEY code={} value={} mapped={}",
                code, value, mapped
            );
        }
        EV_ABS if code == ABS_HAT0X || code == ABS_HAT0Y => {
            eprintln!(
                "pad-debug: EV_ABS(hat) code={} value={} mapped=true",
                code, value
            );
        }
        _ => {}
    }
}

/// Device name for `/dev/input/eventN` via sysfs (no ioctl needed).
fn device_name(node: &Path) -> Option<String> {
    let event = node.file_name()?.to_str()?;
    let sys = format!("/sys/class/input/{}/device/name", event);
    std::fs::read_to_string(sys)
        .ok()
        .map(|s| s.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(ev_type: u16, code: u16, value: i32) -> [u8; EVENT_SIZE] {
        let mut b = [0u8; EVENT_SIZE];
        b[16..18].copy_from_slice(&ev_type.to_le_bytes());
        b[18..20].copy_from_slice(&code.to_le_bytes());
        b[20..24].copy_from_slice(&value.to_le_bytes());
        b
    }

    #[test]
    fn xinput_buttons_map_to_pad_bits() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 304, 1), false); // A down
        s.feed(&event(EV_KEY, 315, 1), false); // Start down
        assert_eq!(s.pad_bits(), (1 << 0) | (1 << 10));
        s.feed(&event(EV_KEY, 304, 0), false); // A up
        assert_eq!(s.pad_bits(), 1 << 10);
    }

    #[test]
    fn dinput_buttons_map_to_pad_bits() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 288, 1), false); // X (D-mode)
        s.feed(&event(EV_KEY, 296, 1), false); // Back -> Select
        assert_eq!(s.pad_bits(), (1 << 2) | (1 << 11));
    }

    #[test]
    fn hat_maps_to_dpad_bits_and_releases() {
        let mut s = PadState::default();
        s.feed(&event(EV_ABS, ABS_HAT0X, -1), false);
        s.feed(&event(EV_ABS, ABS_HAT0Y, 1), false);
        assert_eq!(s.pad_bits(), (1 << 8) | (1 << 7)); // Left + Down
        s.feed(&event(EV_ABS, ABS_HAT0X, 0), false);
        s.feed(&event(EV_ABS, ABS_HAT0Y, 0), false);
        assert_eq!(s.pad_bits(), 0);
    }

    #[test]
    fn xpad_hat_full_range_values_still_signum() {
        // xpad reports HAT0 as -1/0/1 already, but be robust to any magnitude.
        let mut s = PadState::default();
        s.feed(&event(EV_ABS, ABS_HAT0X, 32767), false);
        assert_eq!(s.pad_bits(), 1 << 9); // Right
    }

    #[test]
    fn auto_repeat_counts_as_held() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 305, 2), false); // B auto-repeat
        assert_eq!(s.pad_bits(), 1 << 1);
    }

    #[test]
    fn unknown_codes_and_types_are_ignored() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 999, 1), false);
        s.feed(&event(0x04, 4, 589826), false); // EV_MSC scan code noise
        s.feed(&event(EV_ABS, 0, -32768), false); // left stick ignored by design
        assert_eq!(s.pad_bits(), 0);
    }

    #[test]
    fn partial_event_across_reads_is_reassembled() {
        let mut s = PadState::default();
        let ev = event(EV_KEY, 304, 1);
        s.feed(&ev[..10], false);
        assert_eq!(s.pad_bits(), 0); // incomplete: nothing applied
        s.feed(&ev[10..], false);
        assert_eq!(s.pad_bits(), 1 << 0);
    }

    #[test]
    fn sync_padding_between_events_is_harmless() {
        // A realistic burst: key press + EV_SYN(0,0,0) + hat move.
        let mut s = PadState::default();
        let mut burst = Vec::new();
        burst.extend_from_slice(&event(EV_KEY, 310, 1)); // L
        burst.extend_from_slice(&event(0x00, 0, 0)); // EV_SYN
        burst.extend_from_slice(&event(EV_ABS, ABS_HAT0Y, -1)); // Up
        s.feed(&burst, false);
        assert_eq!(s.pad_bits(), (1 << 4) | (1 << 6));
    }

    #[test]
    fn dinput_lower_triggers_fold_into_l_and_r() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 294, 1), false); // LT (D-mode) -> L
        s.feed(&event(EV_KEY, 295, 1), false); // RT (D-mode) -> R
        assert_eq!(s.pad_bits(), (1 << 4) | (1 << 5));
    }

    #[test]
    fn xinput_lower_triggers_fold_into_l_and_r() {
        let mut s = PadState::default();
        s.feed(&event(EV_KEY, 312, 1), false); // BTN_TL2 -> L
        s.feed(&event(EV_KEY, 313, 1), false); // BTN_TR2 -> R
        assert_eq!(s.pad_bits(), (1 << 4) | (1 << 5));
    }

    /// Mirrors `gamepad_macos.rs`'s `button_bits_cover_each_pad_bit_exactly_once`:
    /// every one of the 12 API.md §3.4 pad bits must be reachable, either via
    /// a `button_bit`-recognized evdev code or via the hat axes.
    #[test]
    fn all_twelve_pad_bits_are_reachable_via_button_bit_and_hat() {
        // One representative code per bit from each recognized code range
        // (XInput, DirectInput, and the folded lower-trigger codes).
        let known_codes: [u16; 20] = [
            304, 305, 307, 308, 310, 311, 315, 314, // XInput: A B X Y L R Start Select
            289, 290, 288, 291, 292, 293, 297, 296, // DirectInput: A B X Y L R Start Select
            312, 313, // XInput lower triggers (folded) -> L R
            294, 295, // DirectInput lower triggers (folded) -> L R
        ];
        let mut seen = 0u16;
        for code in known_codes {
            if let Some(bit) = button_bit(code) {
                seen |= bit;
            }
        }
        // D-pad bits come from the hat axes (ABS_HAT0X/ABS_HAT0Y), not
        // button_bit.
        seen |= (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9);
        assert_eq!(seen, 0x0fff, "not all 12 pad bits reachable: {:#06x}", seen);
    }
}
