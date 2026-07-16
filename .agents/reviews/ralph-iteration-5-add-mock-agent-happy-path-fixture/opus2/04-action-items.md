## Action Items

### Critical
- None

### Important
- [ ] [crates/refwork-harness/tests/mock_agent.rs:241] Create the socketpair with close-on-exec semantics, explicitly preserve only fd 3 for the harness exec, and wrap the spawned harness in a drop guard that kills/reaps it on early fixture failure.

### Suggestions
- [ ] [crates/refwork-harness/tests/mock_agent.rs:35] Add a comment or helper around the intentional pre-Ready `Start` and `HashRequest` pipelining so future readers do not mistake it for an accidental protocol-order bug.
- [ ] [crates/refwork-harness/tests/mock_agent.rs:223] Retry interrupted sends and use `MSG_NOSIGNAL` in the test sender to mirror production control transport behavior.
- [ ] [README.md:10] Add `--locked` to the documented mock-agent fixture command.
