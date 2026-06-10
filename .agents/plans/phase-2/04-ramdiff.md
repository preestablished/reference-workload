# 04 — `crates/ramdiff`: RAM-address discovery MVP

**Independent of 01–03** (records against whatever the core can run; useful
work starts immediately with the synthetic ROM, real use begins in 06).
Design source: ARCHITECTURE.md §5 (workflow + type sketch are normative).

`ramdiff` is a **host-side CLI** — outside the deny gate's scope (threads/
floats permitted, though none are needed), but it links `refwork-emu`
directly with `features = ["introspect"]`. New workspace member at
`crates/ramdiff/` per the README layout.

## Deliverables

1. **Session model** (`src/session.rs`) — per ARCHITECTURE §5's type
   sketch: `Session { dir, dumps: Vec<DumpMeta>, candidates: CandidateSet }`,
   `DumpMeta { label, frame, file, region }`, persisted as
   `session.yaml` + raw `.bin` dumps (full 128 KiB WRAM copies) in a session
   directory. `CandidateSet { width, offsets }` — a plain sorted
   `Vec<u32>`/bitvec is fine for 128 KiB regions; the sketch's RoaringBitmap
   is an option, not a requirement.
2. **`ramdiff record`** (`src/record.rs`) — runs `refwork-emu` host-side:
   - `--rom <file.rom>` + `--script <input log>` (format from package 05 —
     shared crate or module so the two tools never drift): scripted
     deterministic replay with `--mark <frame>=<label>` dump points and
     `--dump-every N` for per-frame trajectories.
   - `--interactive`: opens a window, blits `blit_completed_frame` output
     (XRGB8888, 256×224), maps keyboard → the §3.4 pad bitmask, hotkey
     dumps WRAM with a prompted label, and **records the per-frame pad
     words to an input log on exit**. This is also how first-room scripts
     get authored for 06 — interactivity is explicitly sanctioned for host
     tools (ARCHITECTURE §5 step 2). Keep the windowing dependency minimal
     (`minifb` or `softbuffer`+`winit`-class crate; pick the smallest that
     gives a pixel buffer + key events; pin it).
   - Frame pacing in interactive mode only: host-side sleep to ~60 fps is
     fine here (host tool, not the core).
3. **`ramdiff search`** (`src/filter.rs`) — set-algebra narrowing over the
   persisted candidate set, composing across invocations (each command
   intersects and rewrites `session.yaml`): `--changed A B`,
   `--unchanged A B`, `--inc A B`, `--dec A B`, `--value N --in A`,
   `--delta D A B`, `--width u8|u16le`. `A`/`B` are dump labels.
4. **`ramdiff candidates`** — list survivors with hexdump context lines.
5. **`ramdiff watch --addr <region>:<offset> --rom … --script …`** — replay
   and print the decoded value per frame (with `--width`), so the operator
   confirms semantics (changes exactly on room transitions?) and decides
   `stability` / `discretize`. `--interactive` variant reuses the record
   window with a live value overlay or terminal column.
6. **`ramdiff emit --map feature-maps/demo-game.yaml --name … --offset …
   --type … --stability …`** — append an entry in canonical schema form via
   `refwork-featuremap`'s serde types (link the crate — do not re-implement
   the schema), then run its validator on the result.
7. Accept platform-captured dumps: any 128 KiB raw `.bin` with a manual
   `session.yaml` entry works (ARCHITECTURE §5's `detctl region dump` path
   arrives in later phases; the format contract is just "raw region bytes").

## Acceptance (package-local)

- Unit tests for every filter on synthetic dump fixtures (known planted
  values at known offsets; composition narrows to exactly the plant).
- Round-trip test: `record --script` on the synthetic ROM with two marks →
  `search --changed` → `candidates` finds the synthetic ROM's known
  frame-counter address; `emit` appends a valid entry that
  `refwork-featuremap validate` accepts (against a scratch copy, not the
  real demo map).
- `record --interactive` smoke-tested manually on the synthetic ROM (window
  opens, input registers, dump lands, input log replays identically via
  `record --script` — replay hash equality is the test).
- CI: unit + scripted tests run headless (no window). Interactive paths are
  feature-gated or runtime-flagged so CI never needs a display.
