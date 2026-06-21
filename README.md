# reference-workload

## Harness Mock-Agent Fixture

The guest-sdk happy-path fixture lives at
`crates/refwork-harness/tests/mock_agent.rs`.

Run it with:

```sh
cargo test -p refwork-harness --test mock_agent -- mock_agent_happy_path_1000_frames --nocapture
```

The fixture launches `refwork-harness --fd3` with a real Linux
`AF_UNIX/SOCK_SEQPACKET` fd 3, drives the postcard `CtlMsg` handshake against
the synthetic ROM, records `wram`, `framebuffer`, and `meta` region
publication, checks 1000 per-frame `HashReport`s against a direct
`refwork-emu` run over the same zero-pad sequence, then sends `Shutdown` and
requires a clean harness exit.
