## Action Items

### Critical
- None

### Important
- [ ] [crates/refwork-harness/tests/mock_agent.rs:42] Queue hash requests deterministically instead of refilling a 32-frame window that the free-running harness can outrun.
- [ ] [crates/refwork-harness/tests/mock_agent.rs:25] Add a child-process guard so panics before `wait_for_success` cannot leave `refwork-harness` running.

### Suggestions
- [ ] [crates/refwork-harness/tests/mock_agent.rs:225] Use `MSG_NOSIGNAL` for test sends to match production closed-peer behavior.
- [ ] [crates/refwork-harness/tests/mock_agent.rs:241] Create socketpair fds close-on-exec and explicitly preserve only fd 3 for the harness child.
- [ ] [README.md:11] Add `--locked` to the documented mock-agent test command.
