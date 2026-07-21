# Implementation — Mid-Frame HDMAEN Enable

## Settled semantics (dual-review resolved — do not re-litigate)

The reference check is DONE. snes9x (ppu.cpp:1527, the verified-correct
reference for this scene) handles `$420C` as
`PPU.HDMA = Byte & ~PPU.HDMAEnded;` — a mid-frame enable activates the
channel **without any re-initialization**: no A1T→A2A copy, no table-entry
load, no do_transfer set. `S9xStartHDMA` at V=0 is the only init site.
Games that enable mid-frame stage the channel state themselves through the
writable `$43x8/9` (A2A) and `$43xA` (NTRL) registers — which refwork
already implements (`dma.rs:135-143`, roundtrip-tested). The observed
f1364 state (a2a==a1t, ntrl==0x01 on all three channels with init never
having run) is exactly this staging: the classic NTRL=1 "primer"
(decrement 1→0 at the first serviced line → load the first entry).
Channels whose table hit its terminator this frame must NOT be
re-enableable until the next frame init (`~HDMAEnded`; snes9x's comment
cites Yoshi's Island, Genjyu Ryodan, Mortal Kombat, Tales of Phantasia).

The diagnosis experiment (A1T→A2A + eager entry load) rendered correctly
ONLY because this title's staging makes the copy a no-op and the eager
load a one-line-early primer; it would clobber game-staged state on
resume-dependent titles. It is the wrong fix. Discard it.

## The fix (crates/refwork-emu/src/bus.rs)

1. New per-channel terminated tracking, e.g. `hdma_ended: u8` bitmask on
   the bus (or a `terminated: bool` in `HdmaState`): cleared for all
   channels in `init_hdma`; set at BOTH terminator sites — the
   `load_hdma_entry` count-byte-0 branch (~:549-554) and the
   `execute_hdma` reload path when the next entry terminates (~:639-642).
2. `$420C` handler (committed HEAD ~:1292):

```rust
0x420C => {
    let newly = value & !self.hdmaen & !self.hdma_ended;
    self.hdmaen = value; // raw mask stored; init_hdma re-reads it at V=0
    for ch_idx in 0..8 {
        if newly & (1 << ch_idx) != 0 {
            // Mid-frame enable resumes the channel with its current
            // (game-staged) a2a/ntrl/das — no re-init, per hardware
            // and snes9x ($420C: HDMA = value & !ended). First service
            // is the next line's execute_hdma (~1 line late vs real
            // hardware's next HDMA point on the same line — documented
            // line-granularity simplification).
            self.hdma.state[ch_idx].active = true;
            self.hdma.state[ch_idx].do_transfer = false;
        }
    }
}
```

   Channels newly CLEARED simply stop via the existing mask check in
   `execute_hdma` (:594) with all state retained — resume on re-enable is
   therefore automatic. The `wrapping_sub` countdown path (:637-646)
   already mirrors snes9x's tail loop once the channel is active.

Guest-timing note: refwork's HDMA performs no cycle accounting (no
`add_mclk` in `execute_hdma`), so this fix does NOT alter guest-visible
timing — only rendered output and live $43x8-A state. Host icount (perf
epochs) does change (the channels now do work). Out of scope: $420B
conflict fault interaction, HDMA cycle stealing, sub-line timing.

## EMU_VERSION

Bump to `"refwork-emu 0.2.1"` (doc note: HDMA mid-frame enable, same
2026-07-16 epoch, still pre-re-baseline). Costless, and disambiguates
recordings made under pre-fix 0.2.0 (including the 2026-07-21 session).

## Tests (crates/refwork-emu, in-crate, synthetic — no game data)

Mechanism (verified implementable): follow the existing HDMA test style —
`make_hdma_bus` helper at `bus.rs:1883` with tables in WRAM; "lines" are
driven by calling `init_hdma()` / `execute_hdma()` directly; the mid-frame
write goes through `bus.write(0x00420C, mask)`; staging goes through
`bus.write(0x004308/9/A, ...)`; observe effects via the WMDATA/wram trick
the existing tests use rather than peeking PPU state.

1. **Mid-frame enable with staged primer**: mask 0 at `init_hdma()`; run
   N lines; stage A2A + NTRL=1 via $43x8/9/A; write $420C; assert no
   transfer on the already-serviced line, primer decrement on the next
   serviced line, first data transfer on the line after (pins the
   documented one-line-late simplification).
2. **Frame-start path unchanged**: existing init_hdma tests stay green.
3. **Cleared-then-re-enabled RESUMES (not restarts)**: enable at frame
   start, run lines, clear the bit (assert transfers and counter stop,
   state frozen), re-set the bit → transfers continue exactly where they
   left off. This test must FAIL under init-at-enable semantics.
4. **Resume-from-mid-table A2A**: A1T=$X with a distinctive first entry;
   stage A2A=$X+5 (a later entry) + NTRL primer; enable mid-frame; assert
   transfers come from the later entry and the first entry's data never
   appears. Also FAILS under init-at-enable.
5. **Terminated channel stays dead**: run a channel to its table
   terminator; re-write its $420C bit the same frame → no transfers, no
   ntrl wraparound (guards the 0.wrapping_sub(1) garbage path); next
   `init_hdma()` revives it normally.
6. **Regression suite**: full `-p refwork-emu` matrix (default, audio,
   audio+introspect), clippy, xtask deny.

## Verification

- During implementation, use the introspect scaffold (uncommitted) to
  trace the game's IRQ-handler $43xx writes in the cinematic window,
  confirming the staging story (A2A/NTRL written by the game each frame).
  Settles the remaining ~10% inference to observed fact.
- Replay the operator's 2026-07-21 `discovery-01` padlog (1,608 frames —
  distinct from the canonical 45,230-frame m6 discovery-01) headless:
  frames ~1300-1600 render the letterboxed cinematic with text. Same
  scene content as the experiment's `badplace3` renders; a constant
  ≤1-line phase shift vs those renders is expected and acceptable.
- "The bad place" WRAM dump at frame 1364: expected to byte-match the
  replay because the fix is guest-timing-invisible (no HDMA cycle
  accounting) and the game likely never reads $43x8-A back mid-frame. If
  it does NOT match, triage: game reads live HDMA registers (expected
  divergence, fine — document) vs input-fidelity regression (stop).
- Reference cross-check (scene content, not frame-exact, per bead
  refwork-65f): snes9x harness replay of the same padlog shows the same
  scene elements in this window.
- Determinism gates: xtask determinism (600f), 10k if cheap; the change
  alters fb hashes (and host icount) by design under the open epoch —
  note in refwork-1n8, which migrates the CANONICAL 45,230-frame session,
  not this one.
- Live: operator plays to the cinematic in `record-ramdiff` and sees it.
- Prior-window sanity: the attract-mode sky window (2026-07-19 analysis)
  must still match the reference (it contained no HDMA; expect unchanged).

## Landing

- Single commit: fix + tests + doc comment. Bead: (new fix bead).
- Update `refwork-1n8` (epoch rollout) notes: PPU HDMA fix included in the
  epoch; migration/corpus re-freeze must happen after this lands.
- Revert the experiment hack; the introspect scaffold stays uncommitted
  (or is proposed later through its own clean-room review).
- `EMU_VERSION` → 0.2.1 per the section above (reviewer-recommended;
  same epoch, disambiguates pre-fix recordings).
