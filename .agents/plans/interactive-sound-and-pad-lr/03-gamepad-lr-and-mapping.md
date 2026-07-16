# Package 03 — Gamepad: L/R Fix, Pad-Debug Diagnostic, Full-Mapping Audit

Independent of packages 01/02.

## Problem statement (grounded)

The example game uses L and R; they do not register from the Logitech F310,
though every other button works. Yet **both** backends nominally map them:

- Linux evdev (`crates/ramdiff/src/gamepad.rs:49-71`): XInput codes 310/311
  and DirectInput codes 292/293 → bits 4/5.
- macOS gilrs (`crates/ramdiff/src/gamepad_macos.rs:27-40`):
  `Button::LeftTrigger`/`Button::RightTrigger` → bits 4/5, which is correct
  for SDL `leftshoulder`/`rightshoulder` (verified in gilrs 0.11.2
  `src/mapping/mod.rs:210` — `BTN_LT => add_button("leftshoulder", …,
  Button::LeftTrigger)`).

Interactive sessions currently run on the lab Mac (gilrs path, F310 in
DirectInput mode = "Logitech Dual Action" 046d:c216). Two plausible failure
modes:

1. **Shoulders surfacing as `LeftTrigger2`/`RightTrigger2`** (the SDL
   `lefttrigger`/`righttrigger` slots) instead of
   `LeftTrigger`/`RightTrigger`, on a pad that IS matched by the SDL DB.
   This best matches the reported symptom — "everything works except L/R".
2. **SDL mapping miss → gilrs default map.** gilrs resolves the SDL DB entry
   by a UUID that embeds the device *version* (gilrs-core 0.6.8
   `src/platform/macos/gamepad.rs:279-325`). The bundled DB has three
   macOS "Logitech Dual Action" entries with specific version nibbles
   (`SDL_GameControllerDB/gamecontrollerdb.txt:1008-1010`); a pad reporting
   any other version falls back to gilrs's usage-order default mapping,
   under which F310 D-mode LB/RB land on `Button::West`/`Button::Z`
   (gilrs-core `src/platform/macos/io_kit.rs:327-334` + gilrs
   `src/mapping/mod.rs` default map), RB-adjacent face buttons shift, and
   physical button 3 lands on the unmapped `BTN_C`. This mode would break
   more than just L/R, so it fits the symptom less well — but it cannot be
   excluded without the pad.

We cannot distinguish these without the physical pad, so this package ships
(a) a diagnostic to observe the truth, (b) mapping hardening for mode 1,
(c) a zero-code escape hatch that fixes mode 2, and (d) a hardware
verification gate. Note the asymmetry: **the trigger fold below fixes mode 1
only.** Under mode 2, `LeftTrigger2`/`RightTrigger2` are physical Back/Start
(HID usages 9/10), so folding them unconditionally would misroute
Back/Start into L/R — which is why the fold is gated on the mapping source,
and why mode 2's actual remedy is the exact-UUID SDL mapping line. The
on-hardware operator must not stop at "L/R respond via some button" —
verify all 12 buttons land on the right bits.

## Changes

### 1. `--pad-debug` diagnostic flag (interactive-only)

- CLI: `ramdiff record --interactive --pad-debug` (parse alongside
  `--gamepad`, `crates/ramdiff/src/main.rs:177-192`; usage text updated).
- Plumb a `pad_debug: bool` through `InteractiveOpts`
  (`crates/ramdiff/src/record.rs:202-217`) into both backends' constructors.
- macOS backend: on open, print the pad's name, **UUID** (hex of
  `gilrs::Gamepad::uuid()`), and **mapping source**
  (`gilrs::Gamepad::mapping_source()`); per event, print every
  `ButtonPressed/ButtonReleased/ButtonChanged/AxisChanged` with the
  `Button`/`Axis` variant **and the raw `Code`** (its `Display` shows the
  native event code). One line per event to stderr.
- Linux backend: per decoded event, print `ev_type/code/value` for `EV_KEY`
  and hat events, plus whether the code mapped to a pad bit.
- Exit path unchanged — it's the normal interactive session, just chatty.
  This flag is the tool the on-hardware gate uses; it also permanently
  de-mystifies future pad quirks.

### 2. Mapping hardening — macOS (`gamepad_macos.rs`)

- **Fold trigger variants, gated on mapping source**: add a separate
  fallback table (`TRIGGER_FOLD_BITS`) mapping `Button::LeftTrigger2` →
  bit 4 (L) and `Button::RightTrigger2` → bit 5 (R), applied in `poll()`
  **only when `mapping_source()` is the SDL DB** (`MappingSource::SdlMappings`
  — verify the exact variant name in gilrs 0.11.2 at implementation time).
  On an SDL-mapped pad the SNES has no lower triggers, so LB+LT → L and
  RB+RT → R is strictly more usable and cannot conflict. Under the default
  (unmatched) mapping the fold MUST NOT apply: there, `*Trigger2` are
  physical Back/Start (usages 9/10) and folding would misroute them.
  Keep the main `BUTTON_BITS` coverage test (`gamepad_macos.rs:129-138`)
  exactly as is ("each of the 12 bits exactly once"), and add a test that
  `TRIGGER_FOLD_BITS` covers only bits 4 and 5.
- **Mapping escape hatch (zero code)**: gilrs already honors the standard
  `SDL_GAMECONTROLLERCONFIG` env var — `Gilrs::new()` applies env mappings
  by default (gilrs 0.11.2 `src/gamepad.rs:671-677`,
  `GilrsBuilder::add_env_mappings` defaults true). No builder change and no
  custom env var needed. With `--pad-debug` output (UUID) in hand, the
  operator writes an exact-UUID SDL mapping line into
  `SDL_GAMECONTROLLERCONFIG` — no rebuild. Document the workflow in the
  module doc.
- **Warn on default mapping**: if `mapping_source()` is not the SDL DB, print
  a one-line warning suggesting `--pad-debug` + `SDL_GAMECONTROLLERCONFIG`.

### 3. Mapping hardening — Linux (`gamepad.rs`)

- Fold DirectInput lower triggers into L/R: codes **294 (LT) → bit 4** and
  **295 (RT) → bit 5** (D-mode button order: 288 X, 289 A, 290 B, 291 Y,
  292 LB, 293 RB, 294 LT, 295 RT, 296 Back, 297 Start). XInput mode: also
  accept `BTN_TL2`/`BTN_TR2` (312/313) → bits 4/5 for the same reason.
- Add/extend unit tests for the new codes (existing test style,
  `gamepad.rs:246-332`).

### 4. Full-mapping audit (the "all buttons sensible" ask)

Confirm and document the complete 12-button table in one place (module doc of
each backend + the CLI doc header, `main.rs:30-54`): A/B/X/Y by printed
label, LB/RB (and LT/RT folded) → L/R, D-pad → D-pad, Start → Start,
Back/Select → Select. The tables already cover all 12 bits; the audit is:
tests assert full coverage on both backends (Linux has no coverage test
today — add one mirroring `gamepad_macos.rs:129-138`), and the on-hardware
gate exercises every button.

## Tests

- Linux: new codes 294/295/312/313 map to bits 4/5; coverage test asserting
  all 12 bits reachable via `button_bit` + hat handling.
- macOS: updated coverage test per above; `BUTTON_BITS` duplicate-audit.
- `--pad-debug` plumbing: parse test for the flag (arg-parsing already has
  precedent in `main.rs` tests if any; otherwise smoke-parse in a unit test).

## Acceptance Criteria

0. `cargo test --locked -p ramdiff --features interactive` green on macOS.
1. `--pad-debug` prints name, UUID, mapping source, and per-event
   Button+Code lines on the lab Mac with the F310 connected.
2. On hardware (package 04 gate): every one of the 12 SNES buttons
   registers from the F310 (D mode) — verified via pad-debug lines AND
   in-game L/R behavior in the example game.
3. If hardware shows the default-mapping failure mode, an exact-UUID
   `SDL_GAMECONTROLLERCONFIG` line fixes it without code changes; capture
   the working line in `tools/record-ramdiff` (export the env var there) so
   sessions get it automatically. (Wrapper change is in-repo and contains no
   commercial ROM naming — UUIDs are fine.)
4. Keyboard fallback (Q/W for L/R) still works and is documented unchanged.

## Out Of Scope

- Analog stick support (SNES has none; both backends deliberately ignore
  sticks).
- Remappable keybindings/config files beyond the SDL-mapping env escape
  hatch.
- XInput mode support on macOS (the F310 does not enumerate as HID in X mode
  there; D mode remains the documented requirement,
  `gamepad_macos.rs:3-7`, `tools/record-ramdiff:81-101`).
