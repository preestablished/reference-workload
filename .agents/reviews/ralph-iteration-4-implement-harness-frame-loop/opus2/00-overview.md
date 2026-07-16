# Review Overview

- Branch: `ralph/iteration-4-implement-harness-frame-loop`
- Date: 2026-06-21
- Reviewer: Claude Opus (2nd reviewer)
- Overall verdict: REQUEST_CHANGES

This branch adds the harness steady-state frame loop after fd-3 setup: it activates published regions into the emulator core, runs one frame at a time, polls platform input once per frame, publishes completed frame/meta state, processes one control message at each frame boundary, emits hash reports for the last completed frame, handles shutdown, and faults invalid steady-state protocol traffic.

Stats: 6 files changed, 704 insertions, 14 deletions, 1 commit (`73f7eb7 ralph: iteration 4 checkpoint - add harness frame loop`).

Local verification performed: `cargo test --locked -p refwork-harness` passed with 36 tests.
