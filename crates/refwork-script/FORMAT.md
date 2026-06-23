# `.padlog` — scripted input log format, version 1

One u16 pad word per frame, in the platform bit order of API.md §3.4
(bit 0..11 = A B X Y L R Up Down Left Right Start Select; bits 12–15
reserved, must be zero). Frame 0 is the first `run_one_frame` after
`Core::new`. Nothing else is recorded — no timestamps, no events; this is
the host twin of "the latch word per frame" (D6).

This is the `console16-12btn-v1` pad layout, layout version 1. Button
names are exact and mixed-case: `A`, `B`, `X`, `Y`, `L`, `R`, `Up`,
`Down`, `Left`, `Right`, `Start`, `Select`. `UP`, `DOWN`, `LEFT`,
`RIGHT`, `START`, and `SELECT` are not aliases. Bits 12–15 are reserved
and any set reserved bit is a parse error, never masked.

The format is text, line-oriented, diff-able and hand-editable (M2
acceptance calls for a *hand-authored* script).

## Grammar

- **Header** (required, first non-blank, non-comment line):

  ```
  padlog v1 [rom=<64 lowercase hex chars>]
  ```

  `rom=` is the BLAKE3 of the ROM the script was recorded against. It is
  **advisory**: consumers may warn on mismatch but must not refuse to run
  (scripts are deliberately portable across ROM builds during bring-up).

- **Frame lines**, one of:
  - `HHHH` — a single frame holding pad word `0xHHHH`;
  - `NxHHHH` — `N` (decimal, ≥ 1) consecutive frames of `0xHHHH`
    (run-length form).

  Words are exactly 4 hex digits (case-insensitive on input), value
  ≤ `0x0FFF` (bits 12–15 zero — a violation is a parse error, not a mask).
  The total frame count (run-lengths included) is capped at 10,000,000
  (~46 hours); exceeding it is a parse error, so a hostile log cannot
  demand an unbounded allocation.

- **Comments**: `#` to end of line, anywhere. Blank lines ignored.

## Canonical form

The writer emits: lowercase hex, run-length lines for runs > 1, single-word
lines otherwise, `rom=` only when known, no comments, one trailing newline.
`parse(write(log)) == log` for every valid log; `write(parse(text))`
canonicalizes any valid text.

## Example

```
padlog v1 rom=9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08
# hold nothing through power-on
180x0000
# press Start for 2 frames at the title screen
2x0400
600x0000
```

## Forward note (M4/M5)

The hypervisor's DHILOG v1 (Phase-2 hypervisor M5) is a different, binary
format. A `padlog → PAD_SET` converter is trivial and lands with M4/M5 —
this format stays frame-indexed and absolute (not delta-encoded state)
precisely so that conversion remains a map, not a re-simulation. Do not add
events or relative encodings to v1.
