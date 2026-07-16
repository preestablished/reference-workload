//! Gamepad input for interactive record mode on macOS, via gilrs (IOKit HID).
//!
//! macOS has no evdev and no XInput driver, so the Logitech F310 must have
//! its back switch on **D** (DirectInput): it then enumerates as a plain USB
//! HID device ("Logitech Dual Action", 046d:c216) that gilrs maps through the
//! SDL game-controller database. In X mode the pad does not enumerate as a
//! usable HID gamepad at all.
//!
//! The public surface mirrors `gamepad.rs` (Linux) exactly — `open_path`,
//! `open_auto`, `poll`, `description` — so `record.rs` uses one code path
//! for both platforms. Button mapping follows the same convention as the
//! Linux module: the pad's printed labels A/B/X/Y map to the same-letter
//! SNES bits of the API.md §3.4 pad word. Analog sticks are ignored (the
//! SNES pad has none); the D-pad arrives as gilrs `DPad*` buttons.
//!
//! # L/R and the trigger fold
//!
//! `BUTTON_BITS` maps `Button::LeftTrigger`/`RightTrigger` (SDL
//! `leftshoulder`/`rightshoulder`, i.e. LB/RB) to L/R — correct whenever
//! gilrs actually resolved the pad through the SDL mapping database. When it
//! doesn't (`mapping_source()` is not `MappingSource::SdlMappings` — e.g. the
//! bundled SDL DB has no entry for this pad's exact HID version), gilrs falls
//! back to a default positional map under which the F310's LB/RB do not land
//! on `LeftTrigger`/`RightTrigger` at all, and `Button::LeftTrigger2`/
//! `RightTrigger2` are the pad's physical Back/Start buttons instead of lower
//! triggers.
//!
//! `TRIGGER_FOLD_BITS` additionally folds `LeftTrigger2`/`RightTrigger2` into
//! L/R (the SNES pad has no lower triggers, so this is strictly more usable
//! on a real SDL-mapped pad), but **only** when `mapping_source()` is
//! `SdlMappings` — determined once at open and cached as `sdl_mapped`.
//! Applying the fold under the default mapping would misroute physical
//! Back/Start into L/R, so it must stay gated.
//!
//! # `--pad-debug` and the `SDL_GAMECONTROLLERCONFIG` escape hatch
//!
//! Interactive sessions started with `--pad-debug` print, on open, the pad's
//! name, UUID (lowercase hex), and `mapping_source()`; every subsequent
//! button/axis event is then printed to stderr as it's polled. If
//! `mapping_source()` comes back as anything other than `SdlMappings`, a
//! warning is printed at open pointing at this diagnostic.
//!
//! The fix in that case requires no code change: `Gilrs::new()` already
//! merges mappings from the standard `SDL_GAMECONTROLLERCONFIG` environment
//! variable (gilrs 0.11.2 defaults `add_env_mappings` to `true`). With the
//! UUID from `--pad-debug` in hand, set an exact-UUID SDL mapping line in
//! that env var before launching `ramdiff record` (see `tools/record-ramdiff`
//! for the wrapper hook) and the pad will resolve through the SDL DB path,
//! unlocking the trigger fold above.

use gilrs::{Button, EventType, GamepadId, Gilrs, MappingSource};
use std::path::Path;

/// Substrings (lowercase) a preferred device name contains — same hints as
/// the Linux backend. gilrs can also surface non-gamepad HID devices (e.g.
/// multi-axis controllers), so a hint match wins over enumeration order;
/// with no match the first device is still used.
const NAME_HINTS: [&str; 4] = ["f310", "dual action", "logitech gamepad", "gamepad"];

/// (gilrs button, API.md §3.4 pad bit). gilrs uses SDL positional names:
/// on the F310's Xbox-style layout South=A, East=B, West=X, North=Y.
const BUTTON_BITS: [(Button, u16); 12] = [
    (Button::South, 1 << 0),         // A
    (Button::East, 1 << 1),          // B
    (Button::West, 1 << 2),          // X
    (Button::North, 1 << 3),         // Y
    (Button::LeftTrigger, 1 << 4),   // LB -> L
    (Button::RightTrigger, 1 << 5),  // RB -> R
    (Button::DPadUp, 1 << 6),        // Up
    (Button::DPadDown, 1 << 7),      // Down
    (Button::DPadLeft, 1 << 8),      // Left
    (Button::DPadRight, 1 << 9),     // Right
    (Button::Start, 1 << 10),        // Start
    (Button::Select, 1 << 11),       // Back -> Select
];

/// Lower-trigger fallback for pads whose SDL mapping puts LT/RT in the
/// `*Trigger2` slots. Applied in `poll()` only when `sdl_mapped` is true —
/// see the module doc "L/R and the trigger fold" section.
const TRIGGER_FOLD_BITS: [(Button, u16); 2] = [
    (Button::LeftTrigger2, 1 << 4),  // LT -> L
    (Button::RightTrigger2, 1 << 5), // RT -> R
];

/// An open gilrs-backed gamepad bound to one device.
pub struct Gamepad {
    gilrs: Gilrs,
    id: GamepadId,
    /// Set once the device disconnected; the pad then reads as 0.
    dead: bool,
    /// Human-readable identity for logs: "<name> (gilrs)".
    pub description: String,
    /// `--pad-debug`: print every button/axis event to stderr in `poll()`.
    pad_debug: bool,
    /// Cached at open time: `mapping_source() == MappingSource::SdlMappings`.
    /// Gates `TRIGGER_FOLD_BITS` — see the module doc.
    sdl_mapped: bool,
}

impl Gamepad {
    /// Explicit device paths are an evdev concept; not supported here.
    pub fn open_path(_path: &Path, _pad_debug: bool) -> Result<Self, String> {
        Err("--gamepad <path> is not supported on macOS; \
             omit the flag to auto-detect via IOKit HID"
            .to_owned())
    }

    /// Bind the first connected gamepad. `Ok(None)` when none is present
    /// (a pad plugged in after launch is not picked up, matching the Linux
    /// backend's open-once semantics).
    ///
    /// IOKit device discovery is asynchronous: a pad that is already plugged
    /// in surfaces as a `Connected` event a few milliseconds after
    /// `Gilrs::new()`, so enumerating immediately would always miss it
    /// (observed: ~12ms on an M-series Mac). Pump events for a short window
    /// before concluding no pad is present.
    pub fn open_auto(pad_debug: bool) -> Result<Option<Self>, String> {
        let mut gilrs = Gilrs::new().map_err(|e| format!("gilrs init failed: {}", e))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        loop {
            while gilrs.next_event().is_some() {}
            let pads: Vec<(GamepadId, String)> = gilrs
                .gamepads()
                .map(|(id, g)| (id, g.name().to_owned()))
                .collect();
            let picked = pads
                .iter()
                .find(|(_, name)| {
                    let lower = name.to_lowercase();
                    NAME_HINTS.iter().any(|h| lower.contains(h))
                })
                .or_else(|| pads.first())
                .cloned();
            if let Some((id, name)) = picked {
                // Computed before the struct move below so the immutable
                // borrow of `gilrs` through `pad` ends first.
                let (mapping_source, uuid) = {
                    let pad = gilrs.gamepad(id);
                    (pad.mapping_source(), pad.uuid())
                };
                let sdl_mapped = mapping_source == MappingSource::SdlMappings;
                if pad_debug {
                    eprintln!(
                        "pad-debug: open name={:?} uuid={} mapping_source={:?}",
                        name,
                        uuid_hex(uuid),
                        mapping_source
                    );
                }
                if !sdl_mapped {
                    eprintln!(
                        "gamepad: warning: mapping_source is {:?}, not SdlMappings — the L/R \
                         trigger fold (LT/RT) is disabled to avoid misrouting Back/Start. \
                         Run with --pad-debug to see the pad's UUID, then set an exact-UUID \
                         SDL mapping via the SDL_GAMECONTROLLERCONFIG environment variable.",
                        mapping_source
                    );
                }
                return Ok(Some(Gamepad {
                    gilrs,
                    id,
                    dead: false,
                    description: format!("{} (gilrs)", name),
                    pad_debug,
                    sdl_mapped,
                }));
            }
            if std::time::Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Drain pending events and return the current pad word (0 if dead).
    pub fn poll(&mut self) -> u16 {
        // Pump the event queue even when dead so gilrs's internal state
        // stays consistent; the bound pad simply reads as released.
        while let Some(ev) = self.gilrs.next_event() {
            if self.pad_debug {
                debug_print_event(&ev.event);
            }
            if ev.id == self.id && matches!(ev.event, EventType::Disconnected) {
                eprintln!("gamepad: device disconnected; continuing keyboard-only");
                self.dead = true;
            }
        }
        if self.dead {
            return 0;
        }
        let pad = self.gilrs.gamepad(self.id);
        let mut bits = BUTTON_BITS
            .iter()
            .filter(|(button, _)| pad.is_pressed(*button))
            .fold(0u16, |acc, (_, bit)| acc | bit);
        if self.sdl_mapped {
            bits |= TRIGGER_FOLD_BITS
                .iter()
                .filter(|(button, _)| pad.is_pressed(*button))
                .fold(0u16, |acc, (_, bit)| acc | bit);
        }
        bits
    }
}

/// `--pad-debug`: print one line per button/axis event, including the raw
/// `Code` (its `Display` shows the native event code) alongside the decoded
/// `Button`/`Axis` variant.
fn debug_print_event(ev: &EventType) {
    match ev {
        EventType::ButtonPressed(button, code) => {
            eprintln!("pad-debug: ButtonPressed {:?} code={}", button, code);
        }
        EventType::ButtonReleased(button, code) => {
            eprintln!("pad-debug: ButtonReleased {:?} code={}", button, code);
        }
        EventType::ButtonChanged(button, value, code) => {
            eprintln!(
                "pad-debug: ButtonChanged {:?}={} code={}",
                button, value, code
            );
        }
        EventType::AxisChanged(axis, value, code) => {
            eprintln!("pad-debug: AxisChanged {:?}={} code={}", axis, value, code);
        }
        _ => {}
    }
}

/// Format a gilrs UUID as lowercase hex (no dashes — matches
/// `SDL_GAMECONTROLLERCONFIG`'s GUID column format).
fn uuid_hex(uuid: [u8; 16]) -> String {
    uuid.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards mapping drift between backends: the table must assign each of
    /// the twelve API.md §3.4 pad bits (0..=11) exactly once, like the Linux
    /// module's `button_bit` + hat handling does.
    #[test]
    fn button_bits_cover_each_pad_bit_exactly_once() {
        let mut seen = 0u16;
        for (_, bit) in BUTTON_BITS {
            assert_eq!(bit.count_ones(), 1);
            assert_eq!(seen & bit, 0, "bit {:#06x} assigned twice", bit);
            seen |= bit;
        }
        assert_eq!(seen, 0x0fff);
    }

    /// Guards the trigger fold: it must cover exactly L (bit 4) and R
    /// (bit 5) — the same bits `BUTTON_BITS` already assigns to LB/RB —
    /// and nothing else, so a gated `poll()` OR-in can never touch another
    /// bit.
    #[test]
    fn trigger_fold_bits_cover_exactly_l_and_r() {
        let mut seen = 0u16;
        for (_, bit) in TRIGGER_FOLD_BITS {
            assert_eq!(bit.count_ones(), 1);
            assert_eq!(seen & bit, 0, "bit {:#06x} assigned twice", bit);
            seen |= bit;
        }
        assert_eq!(seen, (1 << 4) | (1 << 5));
    }
}
