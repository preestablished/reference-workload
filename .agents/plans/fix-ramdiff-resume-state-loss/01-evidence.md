# Evidence log (diagnosis, 2026-07-15)

All paths under `~/m6-private/ramdiff/`. The emulator is deterministic by
design (`Core::new` is documented "Deterministic construction (D3) … No I/O of
any kind"; no `SystemTime`/`Instant`/`rand` anywhere in `refwork-emu/src`), so
replaying the same pad sequence on the same binary must reproduce the state.

## 1. The padlog contradicts the session metadata

`discovery-01/session.yaml` records 17 dumps, the last at **frame 77,146**
(`1-4 boss defeated`). The padlog holds only **8,605** pad lines:

```
$ wc -l discovery-01/interactive.padlog        # 8606 (incl. header)
$ sort discovery-01/interactive.padlog | uniq -c | sort -rn
   8578 0000
     27 0400      # 0x0400 = Start only
      1 padlog v1
```

A log ending at frame 8,604 cannot reach a state dumped at frame 77,146.
Moreover the content is *only* Start presses — no directional/jump/attack
input. Nobody beats a boss with 27 Start presses. The log is not the recording
of the boss run.

## 2. The boss run really happened, at full speed, in this session dir

Dump file mtimes in `discovery-01` span 01:11–01:33 on Jul 14 (22 min); dump
frames span 1,123 → 77,146. 77k frames / 22 min ≈ **58 fps** — a normal
full-speed interactive session. The dumps and `session.yaml` are the surviving
artifacts of the real run.

## 3. The log was rewritten ~1 hour after the run ended

- `discovery-01.bak-4` (identical dump set — it is the same session, rotated
  later): `interactive.padlog` mtime **02:31**, 7,345 lines, **all `0000`**.
- `discovery-01.failed-resume-20260714T024038Z`: 661 lines, all `0000`,
  written 02:38.
- Deployed binary `~/m6/.../target/release/ramdiff` mtime **02:40** (rebuilt
  right after; contains the "replaying … frames" strings).

So after the run ended at 01:33, something at ~02:29–02:31 rewrote the padlog
from scratch (header + zeros). 7,345 frames of zeros ≈ 2 minutes of an
untouched title screen — consistent with a "resume" attempt that actually
booted fresh while recording nothing.

## 4. The pre-rebuild source truncated unconditionally

The deployed source copy (`~/m6/preestablished/reference-workload`, not a git
repo) still shows the old `run_interactive`:

```rust
let mut log_file = std::fs::OpenOptions::new()
    .create(true).write(true).truncate(true)   // <-- always truncates
    .open(&opts.output_log) ...
```

and its `InteractiveOpts` has no `resume` field. Any interactive run against an
existing session dir with that build **destroyed the padlog on startup**. The
current git code only truncates when `resume` is false or the file is missing —
better, but a direct `ramdiff record --interactive` (without the wrapper's dir
rotation) still silently destroys an existing log today.

## 5. Older sessions confirm recording used to work

```
bak-1  10,827 lines: 0000/0200/0100/0400/0008/0002… (rich input, Jul 13)
bak-2  25,229 lines: rich input incl. combined words (Jul 14 00:38–01:02)
bak-3  16,719 lines: all 0000 (Jul 14 01:05–01:10)
bak-4   7,345 lines: all 0000 (the clobbered boss-session log)
```

Recording produced real input logs before Jul 14 ~01:05. Every all-zero log
postdates that; all are explainable as fresh-boot recordings and/or truncation
victims. There is no evidence of a "gamepad input not logged" bug in the
current code: the live loop logs the same merged `pad` word it feeds to
`run_one_frame` (`crates/ramdiff/src/record.rs`, live loop).

## Conclusion

- Resume-by-replay logic in the current git tree is *mechanically* correct but
  operates on a destroyed log, and nothing detects the destruction.
- The fix must (a) make log destruction impossible going forward, (b) make
  resume refuse — with an actionable message — when log and session metadata
  disagree, (c) verify replay against stored dumps to catch divergence, and
  (d) get the fixed binary onto the path the wrapper actually runs.
