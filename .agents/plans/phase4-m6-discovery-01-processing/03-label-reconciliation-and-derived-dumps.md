# Package 03 — Label Reconciliation & Replay-Derived Dumps (KNOWN GAP 1, Agent-Only)

## Goal

Bridge the mismatch between what was captured (trajectory-style labels at 16
frames) and what `tools/m6-discovery-analyze.sh` requires (the controlled
Run-A label set `baseline start-a1 ... dead`; missing labels are a hard
`exit 1`, lines 199–208). **Decision this package records:** reconcile by
(i) deriving additional labeled WRAM dumps from deterministic replay
(package 01 proved byte-exact fidelity, so replay-derived dumps are as good
as hand-captured ones), and (ii) adding a plan-driven analyzer alongside the
existing tool. A second operator session is NOT needed for the core feature
set; only credits/dead/reload evidence needs the operator (package 05,
STOP #1).

Rejected alternatives, for the record: editing `REQUIRED_LABELS` in place
(silently repurposes a tool that SESSION-DAY-RUNBOOK §§1–2 documents for the
Run-A contract); a full second mini-session for the original label set
(unnecessary operator time — the 45,230-frame padlog already contains nearly
every needed state change).

## The 16 anchors (public-safe roles; real label strings stay in `session.yaml`)

| frame | role |
|---|---|
| 1242 | menu A (title/attract) |
| 1539 | menu B (mode-select highlighted) |
| 3800 | overworld hub, world 1 |
| 5230 | stage W1-S1 entry |
| 5375 | damage event in W1-S1 |
| 12730 | capacity pickup (max-resource +1) |
| 14452 | stage W1-S2 entry |
| 19276 | W1-S2 midboss begin |
| 23130 | W1-S2 midboss defeated |
| 24358 | stage W1-S3 entry |
| 29241 | stage W1-S4 entry |
| 30101 | W1-S4 boss begin |
| 37080 | W1-S4 boss defeated |
| 39085 | overworld hub, world 2 |
| 39966 | stage W2-S1 entry |
| 41511 | W2-S1, equipment upgrade active |

## Steps

### 1. Locate supplementary frames from the padlog (idle/movement/event windows)

Frame `i` is line `i+2` of `interactive.padlog` (header line 1; verified).

Idle runs (≥60 consecutive all-zero pad words) inside a stage window, for the
volatile-noise exclusion pair and the position idle pair:

```sh
mkdir -p "$PR/discovery"
awk 'NR>1 { f=NR-2; if ($0=="0000") { if (run==0) start=f; run++ }
     else { if (run>=60) print start, f-1, run; run=0 } }
     END { if (run>=60) print start, f-1, run }' "$SESS/interactive.padlog" \
  > "$PR/discovery/idle-runs.txt"
```

Movement frames (Left bit 8 = `0x0100`, Right bit 9 = `0x0200` per API.md
§3.4; words are 4-hex-digit pad masks). NOTE: this host's `awk` is BSD awk —
`strtonum()`/`and()` are gawk-only and gawk is not installed — so use python3
(already a package-01 prerequisite). The sustained-hold rule (direction held
30+ consecutive frames) is baked into the command; it prints hold RUNS, and
the agent then selects runs falling **within one stage window** manually
against the anchor table:

```sh
python3 - "$SESS/interactive.padlog" > "$PR/discovery/move-frames.txt" <<'EOF'
import sys
frames = open(sys.argv[1]).read().splitlines()[1:]  # drop "padlog v1"; frame i = frames[i]
start = cur = None; n = 0
def flush():
    if cur and n >= 30: print(start, start + n - 1, cur, n)
for f, w in enumerate(frames):
    v = int(w, 16)
    d = 'right' if v & 0x0200 else ('left' if v & 0x0100 else None)
    if d == cur and d is not None: n += 1
    else:
        flush(); start, cur, n = (f, d, 1) if d else (None, None, 0)
flush()
EOF
```

(Each output line: run-start frame, run-end frame, direction, length. Runs
shorter than 30 frames are filtered out by the script itself.)

Choose (record choices in `$PR/discovery/frame-plan.yaml`, step 2):

- `idle-a`, `idle-b`: two frames 20+ apart inside one idle run within
  W1-S1 (window 5230–12730) — the stability-sanity exclusion pair.
- `move-b`: a frame after sustained horizontal movement in the same room as
  `idle-a` (same stage, no transition between them).
- Event-window grid marks to shrink wide pairs (upgrade pickup somewhere in
  39966→41511; capacity pickup around 12730; boss-flag sets inside
  19276→23130 and 30101→37080): e.g.
  `seq 40000 250 41500 | awk '{printf " --mark %d=w%d", $1, $1}'`.

### 2. Author the private frame plan

`$PR/discovery/frame-plan.yaml` — the single private source of truth mapping
comparison roles → (session, label/frame). Public-safe working feature names
(these become the map draft's names in package 05; they mirror
`feature-maps/demo-game.yaml` structure):

| working feature | width | discriminating pairs (from anchors + derived) |
|---|---|---|
| `game_mode` | u8 | changed 1242↔3800, 3800↔5230; unchanged 5230↔14452 |
| `area_id` (world/overworld index) | u8 | unchanged 3800-era vs W1 stages; changed 3800↔39085, 5230↔39966 |
| `room_id` (stage/level id) | u16le | changed 5230↔14452, 14452↔24358, 24358↔29241, 29241↔39966; unchanged 5230↔5375 |
| `player_x`/`player_y` | u16le | unchanged idle-a↔idle-b; changed idle-a↔move-b (axis disambiguation per the existing tool's NOTE — hexdump context or an axis-isolated derived pair) |
| `health` | u8 & u16le | dec 5230↔5375; unchanged idle-a↔idle-b |
| `max_health` | u8 & u16le | inc across the 12730 window (grid pair straddling the pickup) |
| `upgrade_flags` (equipment state) | u8 & u16le | changed across the 39966→41511 window (narrowed by grid marks); unchanged 5230↔14452 |
| `boss_flags`/progress flags | u8 & u16le | changed 19276↔23130 AND 30101↔37080; unchanged 5230↔14452 |
| `credits_flag` | — | **not derivable** — recorded gap, consumed by package 05 STOP #1 |
| `game_mode` dead value | — | **not derivable** (no death in trajectory) — same gap |

Widths are hypotheses — run both where marked; the game may store values in
either width (or BCD — if both widths come up empty, retry with `--delta`
/`--value` filters per package 04 failure modes).

### 3. Produce the derived session by one replay pass

```sh
DRV="$PR/ramdiff/derived-01"
target/release/ramdiff record --rom "$ROM" --script "$SESS/interactive.padlog" \
  --session "$DRV" \
  --mark <idle-a>=idle-a --mark <idle-b>=idle-b --mark <move-b>=move-b \
  $(cat "$PR/discovery/grid-marks.txt") \
  --frames 45230
```

(All grid marks in the same single pass; replay is ~45k frames and cheap.
More passes are fine later if a window needs a finer grid — new session dirs,
never touching `discovery-01`.)

### 4. Plan-driven analyzer: `tools/m6-discovery-analyze2.sh`

New file (do NOT modify the existing tool — its Run-A contract stays
documented). Same structure and helpers as `tools/m6-discovery-analyze.sh`
(reuse `lookup_dump_file`, `extract_offsets`, `parse_count`,
`new_feature_session`, the exclusion-set logic, and the report/emit-draft
format — cite the original in the header), with these deltas:

- Takes `--plan <frame-plan.yaml>` instead of hardcoded `REQUIRED_LABELS`:
  the plan file declares the exclusion pair and, per feature: name, width,
  emit type/semantics/discretize, `check_exclusion`, source session + label
  for each referenced dump, and the ordered filter clauses
  (`changed|unchanged|inc|dec|value|delta` — exactly the `ramdiff search`
  filter surface).
- Scratch per-feature sessions symlink dumps from **multiple** source
  sessions (`discovery-01` + `derived-01` + any later derived dirs) — extend
  `new_feature_session` to take `label:source-dir` pairs.
- Identical privacy contract: stdout gets counts and warnings only; offsets,
  decoded values, and hexdump context go only to the private report file.
- Same authoring-time verification bar as the original (header lines 30–40):
  `bash -n`, plus a synthetic 2-dump planted-byte self-test against the real
  `target/release/ramdiff` binary, run from a scratch directory.

## Acceptance criteria

- `frame-plan.yaml` exists under `$PR/discovery/` and covers every row of the
  step-2 table (including the two recorded gaps).
- `derived-01` session exists; its `session.yaml` lists every planned derived
  dump; zero replay faults.
- `tools/m6-discovery-analyze2.sh`: `bash -n` clean; synthetic self-test
  passes; a dry run against the real plan produces a private report with a
  candidate count line for ≥8 working features.
- `discovery-01/session.yaml` is byte-identical to its package-01 state
  (`candidates.offsets` still `[]`) — searching happened only in scratch dirs.

## On failure

- No qualifying idle run: lower the threshold to 30 frames, or use a menu
  screen window (1242–1539) for the exclusion pair, noting that menu-time
  noise differs from in-level noise in the report.
- Replay fault during derivation: back to package 01's gate (something
  changed — rebuild? wrong ROM?); do not proceed on a partial derived
  session.
- A window's grid pair shows dozens of changed offsets everywhere (screen
  transition garbage): move the grid marks to quiescent frames on each side
  of the event (idle moments before/after), not mid-transition.
