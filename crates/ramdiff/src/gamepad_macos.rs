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

use gilrs::{Button, EventType, GamepadId, Gilrs};
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

/// An open gilrs-backed gamepad bound to one device.
pub struct Gamepad {
    gilrs: Gilrs,
    id: GamepadId,
    /// Set once the device disconnected; the pad then reads as 0.
    dead: bool,
    /// Human-readable identity for logs: "<name> (gilrs)".
    pub description: String,
}

impl Gamepad {
    /// Explicit device paths are an evdev concept; not supported here.
    pub fn open_path(_path: &Path) -> Result<Self, String> {
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
    pub fn open_auto() -> Result<Option<Self>, String> {
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
                return Ok(Some(Gamepad {
                    gilrs,
                    id,
                    dead: false,
                    description: format!("{} (gilrs)", name),
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
            if ev.id == self.id && matches!(ev.event, EventType::Disconnected) {
                eprintln!("gamepad: device disconnected; continuing keyboard-only");
                self.dead = true;
            }
        }
        if self.dead {
            return 0;
        }
        let pad = self.gilrs.gamepad(self.id);
        BUTTON_BITS
            .iter()
            .filter(|(button, _)| pad.is_pressed(*button))
            .fold(0u16, |acc, (_, bit)| acc | bit)
    }
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
}
