# Positive Notes

- `crates/refwork-harness/tests/mock_agent.rs:1` scopes the fixture to Linux, which is appropriate for inherited fd 3 plus `AF_UNIX/SOCK_SEQPACKET` behavior.
- `crates/refwork-harness/tests/mock_agent.rs:21` reuses `xtask::build_synth_rom()`, keeping the integration fixture tied to the same synthetic ROM used by other deterministic checks.
- `crates/refwork-harness/tests/mock_agent.rs:23` and `crates/refwork-harness/tests/mock_agent.rs:272` exercise the real binary over a real fd-3 socket rather than a mocked in-process channel.
- `crates/refwork-harness/tests/mock_agent.rs:146` verifies `GameLoaded` metadata, including the cartridge hash, mapper, and SRAM size.
- `crates/refwork-harness/tests/mock_agent.rs:167` and `crates/refwork-harness/tests/mock_agent.rs:195` validate region publication through `Ready`, including expected lengths and read-only descriptor flags.
- `crates/refwork-harness/tests/mock_agent.rs:50` and `crates/refwork-harness/tests/mock_agent.rs:106` compare harness hash reports against an independent direct `Core` execution, which is the right end-to-end signal for this fixture.
- `crates/refwork-harness/tests/mock_agent.rs:250` sets a receive timeout, preventing the most obvious blocking recv hang from stalling the test indefinitely.
