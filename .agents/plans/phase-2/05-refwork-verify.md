# 05 — `crates/refwork-verify`: input-script format, `play --script`, `map-check`

**Independent of 01–03**; shares the input-log format with 04. New workspace
member at `crates/refwork-verify/` per the README layout — host CLI, links
`refwork-emu` (+ `refwork-featuremap` for map decoding), outside the deny
gate.

This package delivers the M2 subset only: the host-side runner and
map-check. The full-stack determinism suite (double-run vs hypervisor,
snapshot/restore, `trace`) is M5/M6 — design the CLI as subcommands so those
slot in later without renaming anything.

**Seam discipline (Option-B insurance):** 08 calls this suite
"emulator-agnostic and the port's acceptance test" — that only holds if it
is enforced here. All core access goes through the public Core API surface
(`Core::new` / `run_one_frame` / `blit_completed_frame` / `frame_counter` /
`wram()` / `debug_peek` under `introspect`); that surface is the facade an
Option-B port must implement. Nothing in this crate may reach around it.

## Deliverable 1 — `refwork-script` micro-crate (**day-1 deliverable**)

The `.padlog` format is the hidden coupling between this package and 04 —
both consume it, so it cannot live inside either tool. Land
`crates/refwork-script` (format spec doc + parser/writer + round-trip
tests) as this package's **first PR, before 04 starts**; the dependency
graph is `refwork-script → {ramdiff, refwork-verify}`, and future M5 work
consumes it too. Keep it dependency-light; it stays outside the deny scope
unless `refwork-harness` ever grows a dependency on it (see 07).

- Semantics: **one u16 pad word per frame**, bit assignment exactly API.md
  §3.4 (A=0 … Select=11, bits 12–15 zero). Frame 0 is the first
  `run_one_frame` after `Core::new`. Nothing else — no timestamps, no
  events; this is the host twin of "the latch word per frame".
- Format: text, line-oriented, diff-able and hand-editable (acceptance says
  *hand-authored*): header line with magic + version + ROM BLAKE3
  (advisory), then either `N×HEXWORD` run-length lines or per-frame words.
  Comments with `#`. Canonical extension: `.padlog`.
- Forward note in the format doc: the hypervisor's DHILOG v1 (Phase-2
  hypervisor M5) is a different, binary format; a `padlog → PAD_SET`
  converter is trivial and lands with M4/M5 — do not build it now, just
  don't preclude it (frame-indexed absolute, not delta-encoded state).

## Deliverable 2 — `refwork-verify play`

```
refwork-verify play --rom <game.rom> --script <run.padlog>
    [--map feature-maps/demo-game.yaml]
    [--snap <frame>=<out.png|out.bin> ...]      # framebuffer checkpoint dumps
    [--watch <feature> ...]                      # print decoded feature per change
    [--hash-every N] [--frames N] [--report out.json]
```

- Runs the script to completion (or `--frames`), printing/collecting:
  per-frame chained hash option (the `blake3(wram ‖ fb)` + chain definition
  from `xtask/src/hash_chain.rs` moves into a new tiny **`refwork-hash`**
  host crate — blake3 dep, region-hash + chain functions, consumed by both
  xtask and refwork-verify so they can never disagree; deliberately *not* a
  module in `refwork-emu`, which must stay dependency-free for the guest
  build and inside the deny scope), feature-change events when `--map`
  given, fault report with frame number on any `Fault` (D9 surfacing — this
  is the bring-up loop's primary instrument for 06).
- `--continue-past-faults` — **lab-only reconnaissance mode**: on a fault,
  log it (fault, frame, PC context) and keep running instead of halting, so
  one lab run yields the *complete* inventory of missing features rather
  than one per run (D9 makes normal runs halt at the first fault, which
  serializes 03's on-demand lane against the gate clock). Post-fault state
  is garbage and the run is non-authoritative by construction — the flag
  prints a loud banner, is rejected by `map-check`/`double-run`, and is
  banned from acceptance runs and CI. This does not weaken D9: the core
  still faults; only this host tool's stop-policy changes.
- `--snap` writes the published-framebuffer bytes at the named frames —
  these are compared against operator-approved goldens **in the lab** (raw
  `.bin` compare is the gate; `.png` is for human eyes only and may use an
  image crate freely, host-side).
- JSON report: machine-readable summary (final frame, final chained hash,
  feature trajectory, fault if any) — 06's scripts and the lab runner
  consume it.

## Deliverable 3 — `refwork-verify map-check`

Per API.md §1.5: boots the game, runs a script, asserts an expected feature
trajectory.

```
refwork-verify map-check --rom <game.rom> --map feature-maps/demo-game.yaml \
    --script <run.padlog> --expect <expectations.yaml>
```

- `expectations.yaml` (this package defines it; keep it minimal): ordered
  assertions of the form `{ feature, at_frame | by_frame, equals | changes_to
  | delta }` plus optional `never: [{feature, equals}]` invariants. Decoding
  uses `refwork-featuremap` types: `feature_type` widths, `valid_when`
  gating (an assertion on a feature whose `valid_when` is false at that
  frame is an error in the expectations file, not a pass).
- Exit 0/1 with first-failure diagnostics (frame, expected, actual, raw
  bytes at offset). This command is M2's acceptance instrument ("scripted
  run asserts expected feature trajectory") and stays the registration-time
  map gate for every future map version.

## Deliverable 4 — `refwork-verify double-run`

Host-side double-run: run the script twice from fresh `Core::new`, compare
chained hashes (and first-divergent-frame search on mismatch via per-frame
hash comparison). This subsumes the ad-hoc xtask determinism test for
*game* workloads and is the tool 07's 100k-frame cross-arch gate invokes
(`--frames 100000` with a held-input or recorded script). Divergence output:
first divergent frame + which region (wram vs fb) — the M5 region+offset
window diagnostics can extend this later.

## Acceptance (package-local)

- Round-trip and RLE-edge unit tests for the `.padlog` parser/writer;
  property: `write(parse(x))` canonicalizes, `parse(write(log)) == log`.
- `play` on the synthetic ROM: scripted 600-frame run, chained hash equals
  `cargo xtask hash-chain` for the same input policy (proves the
  `refwork-hash` extraction worked).
- `map-check` positive + negative tests against the synthetic ROM with a
  tiny synthetic feature map (known counter address): a correct expectation
  passes; a wrong `changes_to` fails with the right frame number.
- `double-run` on the synthetic ROM green at 10k frames in CI; a
  deliberately injected divergence is caught — the "tests the tester"
  negative from the IMPLEMENTATION-PLAN testing table. **The injection
  lives in `refwork-verify` itself** (a test-only flag that perturbs the
  pad stream or hash input on run 2), *not* in `refwork-emu`: the deny gate
  text-scans `refwork-emu`/`refwork-harness`/`refwork-protocol` source for
  clock/RNG/float tokens regardless of cfg gates, so a
  `cfg(feature = "nondet-test")` wall-clock read in the core would fail
  every PR. See 07 item 5.
- A `map-check` or `double-run` invocation that is handed a
  `--continue-past-faults` artifact (or the flag itself) exits nonzero.
