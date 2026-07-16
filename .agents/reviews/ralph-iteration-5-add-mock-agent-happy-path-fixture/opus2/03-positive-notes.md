# Positive Notes

- `crates/refwork-harness/tests/mock_agent.rs:23` and `crates/refwork-harness/tests/mock_agent.rs:241` use a real `AF_UNIX/SOCK_SEQPACKET` socketpair rather than a mocked transport, which gives useful coverage for fd-3 inheritance and datagram boundary behavior.

- `crates/refwork-harness/tests/mock_agent.rs:24` and `crates/refwork-harness/tests/mock_agent.rs:250` put a receive timeout on the agent side, so a missing harness response fails the test instead of blocking forever in `recv`.

- `crates/refwork-harness/tests/mock_agent.rs:146` verifies `GameLoaded` metadata against the exact synthetic ROM bytes, including the cart hash, mapper, and zero-SRAM expectation.

- `crates/refwork-harness/tests/mock_agent.rs:167` and `crates/refwork-harness/tests/mock_agent.rs:195` collect all region registrations before `Ready` and assert required region sizes plus read-only publication descriptors.

- `crates/refwork-harness/tests/mock_agent.rs:50` and `crates/refwork-harness/tests/mock_agent.rs:106` compare each harness hash report to an independent direct emulator run instead of hard-coding golden hashes, which keeps the fixture maintainable as the synthetic ROM evolves.

- `crates/refwork-harness/tests/mock_agent.rs:53` maintains a bounded request window, exercising steady-state hash handling over many frames without filling the socket with all 1000 requests up front.

- `crates/refwork-harness/tests/mock_agent.rs:299` checks that `Shutdown` produces a successful child exit and includes captured stderr on failure.
