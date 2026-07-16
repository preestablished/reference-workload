# Review Overview

- Branch: `ralph/iteration-3-implement-harness-fd3-control-protocol`
- Date: 2026-06-21
- Reviewer: Claude Opus (2nd reviewer)
- Verdict: `REQUEST_CHANGES`
- Stats: 9 files changed, 886 insertions, 3 deletions, 1 commit (`d5eb53a`)

This branch adds the fd-3 control transport, reusable setup runner, game loader, and production binary entrypoint for the harness. The pure in-memory setup state machine is clear and covered by useful unit tests, and `cargo test -p refwork-harness --locked` plus `cargo test -p refwork-protocol --locked` both pass locally. I am requesting changes because the production transport can still terminate via SIGPIPE instead of returning a controlled setup error, post-`Ready` bad-protocol failures do not consistently mark the published meta page faulted, and peer-influenced fault details can exceed `MAX_DATAGRAM` and suppress the deterministic `Fault` response.
