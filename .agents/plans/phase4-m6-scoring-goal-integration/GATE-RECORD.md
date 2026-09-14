# M6 Entry-Gate Record

## 2026-09-13 — gate first passed (interim branch)

Branch: **raw-session** (hand-play artifact = the `discovery-02-main`
recording via `~/.agents/projects/reference-workload/handplay-session`;
57,064 frames, 34 labeled dumps, emulator epoch refwork-emu 0.2.3).
Scope: interim form of Phase 4 exit gate 3 per
`.agents/decisions/2026-09-13-interim-goal-and-score-shaping.md` — goal =
world-1-clear latch; fires-on-credits UNDECLARABLE. Items 4–5 of the M6
packet (corpus budget run, exploration smoke) remain gated on the worker
stack (STOP #2); the full-corpus branch stays unreachable until the in-VM
capture runs.

Scorer build SHA: none (trace-only evaluation; no index-evaluating scorer
client exists yet — see GATE3-CLAIMS.md).

Private pair v3 (under the private root, never tracked):
- feature-map.yaml v3: blake3 `b811fabc28f7daddce8dd44b011bec0e788adc5fa8a76de4e67fd266534081a4` (24 features, 27 packed bytes)
- scoring-program.yaml v3: blake3 `b11b07420d75b87ce57c0e1901fa9f2e39cef5a03d5f3c7f6a58a8dbc564f0da`
- layout.json: blake3 `blake3:39b8946911334bb7f07d17b6e6cecf955661d7ac9ff4d8bb57a54e4be58f231a`
- Validation: featuremap validate PASS; map-check PASS on 3 padlogs
  (main 759 assertions + 1 never, death 250, timer 181 + 1 never); layout
  review PASS (24 ranges, order/bounds/total_len/no-demo-offset checks).

`tools/m6-gate-check.sh` output, verbatim (home directory abbreviated):

```
PASS     scorer-M3                    documented evidence (scorer DB lost): ~/git/preestablished/state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md marks state-scorer-0gy (M3) closed
PASS     refwork-czi                  closed
PASS     refwork-20v                  closed
PASS     hand-play-artifact           branch=raw-session at ~/.agents/projects/reference-workload/handplay-session
── info (not gating) ──
info: scorer-M4 (items 4-5 need it): no M4-titled bead in state-scorer
info: hypervisor leak bead rom-operator-bridge-l1w — check rom-operator-bridge repo before scheduling the smoke
info: no user bridge unit visible — confirm stack up before items 4-5
── result: 4 passed, 0 not-passed ──
```

## 2026-09-13 — post-WP5 (gate-3 trajectory produced, trace-only)

Branch unchanged: **raw-session**. Scorer build SHA: none — evaluated
**trace-only** (see GATE3-CLAIMS.md; live scorer evaluation pending the
state-scorer plan's `trajeval`). Scope: Phase 4 exit gate 3 declared in its
interim form only (world-1-clear latch; fires-on-credits UNDECLARABLE);
items 4–5 still gated on the worker stack.

`tools/m6-gate-check.sh` output, verbatim (home directory abbreviated):

```
PASS     scorer-M3                    documented evidence (scorer DB lost): ~/git/preestablished/state-scorer/.agents/requests/phase4-m1-m4-first-boss-scoring/04-resolution.md marks state-scorer-0gy (M3) closed
PASS     refwork-czi                  closed
PASS     refwork-20v                  closed
PASS     hand-play-artifact           branch=raw-session at ~/.agents/projects/reference-workload/handplay-session
── info (not gating) ──
info: scorer-M4 (items 4-5 need it): no M4-titled bead in state-scorer
info: hypervisor leak bead rom-operator-bridge-l1w — check rom-operator-bridge repo before scheduling the smoke
info: no user bridge unit visible — confirm stack up before items 4-5
── result: 4 passed, 0 not-passed ──
```
