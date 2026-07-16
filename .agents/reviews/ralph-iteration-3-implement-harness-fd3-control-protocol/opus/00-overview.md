# Review Overview

- Branch: `ralph/iteration-3-implement-harness-fd3-control-protocol`
- Date: 2026-06-21
- Reviewer: Claude Opus
- Verdict: `REQUEST_CHANGES`

This branch adds the fd-3 control transport, ROM-loading helper, setup runner, and production binary entrypoint for the package-02 setup state machine. The shape is close: one datagram maps to one `CtlMsg`, the runner enforces the expected setup order, and the unit tests cover several protocol faults. I am requesting changes because post-READY malformed input can leave the published meta page reporting `ready`, the production binary drops the successful setup result and exits immediately after `Start`, and the optional SRAM path can publish a region while reporting and wiring the loaded cartridge as if SRAM does not exist.

## Stats

- Files changed: 9
- Lines added: 886
- Lines removed: 3
- Commits: 1
- Commit list: `d5eb53a ralph: iteration 3 checkpoint - add harness fd3 setup control`
