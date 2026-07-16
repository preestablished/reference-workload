# Suggestions

### Add production-boundary socketpair tests

- File/lines: `crates/refwork-harness/src/ctl.rs:80-90`, `crates/refwork-harness/src/ctl.rs:142-188`, `crates/refwork-harness/src/runner.rs:310-340`

The current runner tests use `ScriptTransport`, which is good for state-machine coverage but does not exercise Linux `SOCK_SEQPACKET` truncation, `EPIPE`, or signal behavior. Add a small test-only fd wrapper or constructor around `SeqpacketFd` so tests can use `socketpair(AF_UNIX, SOCK_SEQPACKET)` for exact `MAX_DATAGRAM`, `MAX_DATAGRAM + 1`, malformed datagrams, and closed-peer sends.

```rust
#[cfg(test)]
impl SeqpacketFd {
    pub(crate) fn from_owned_fd(fd: OwnedFd) -> io::Result<Self> {
        validate_seqpacket(fd.as_raw_fd())?;
        Ok(Self { fd })
    }
}
```

### Validate fd 3 is AF_UNIX, not only SOCK_SEQPACKET

- File/lines: `crates/refwork-harness/src/ctl.rs:117-139`

`validate_seqpacket` currently checks `SO_TYPE` only. The plan says fd 3 is `socketpair(AF_UNIX, SOCK_SEQPACKET)`, so checking `SO_DOMAIN == AF_UNIX` would catch accidental SCTP/other seqpacket sockets early.

```rust
#[cfg(target_os = "linux")]
fn validate_unix_domain(fd: RawFd) -> io::Result<()> {
    let mut domain: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&domain) as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_DOMAIN,
            (&mut domain as *mut libc::c_int).cast(),
            &mut len,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    if domain != libc::AF_UNIX {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "fd 3 is not AF_UNIX"));
    }
    Ok(())
}
```

### Keep SRAM metadata and registered regions from drifting

- File/lines: `crates/refwork-harness/src/game.rs:70-74`, `crates/refwork-harness/src/runner.rs:195-197`, `crates/refwork-harness/src/runner.rs:243-247`

`SetupConfig` can request an SRAM region, but `loaded_game_from_rom` always reports `sram_size: 0`, so a non-default config would emit `GameLoaded { sram_size: 0 }` and later register an `sram` region. Either keep SRAM disabled until ROM/manifest metadata exists, or make the configured SRAM length the setup source of truth and test that `GameLoaded.sram_size` matches the registered SRAM region.

```rust
let mut game = load_game_or_fault(channel, loader, &dev_path)?;
if let Some(sram_len) = config.sram_len {
    game.sram_size = u32::try_from(sram_len).map_err(|_| SetupError::BadProto {
        detail: format!("sram length {sram_len} does not fit u32"),
    })?;
}
```

### Make the next frame-loop handoff harder to misuse

- File/lines: `crates/refwork-harness/src/runner.rs:31-34`, `crates/refwork-harness/src/main.rs:31-34`

`SetupResult` is the right shape for the next bead, but the production binary currently discards it immediately after `Start`. When the frame loop lands, prefer a consuming handoff such as `run_frame_loop(setup)` rather than exposing a path where regions can be dropped right after the agent believes setup succeeded.

```rust
let setup = run_setup(&mut channel, &mut loader, SetupConfig::default())?;
run_frame_loop(setup, platform)?;
```

### Assert returned setup state, not only emitted messages

- File/lines: `crates/refwork-harness/src/runner.rs:399-429`

The happy-path test checks outbound message order, but it does not inspect the returned `SetupResult`. Add assertions that `result.game.cart_hash` matches `GameLoaded.cart_hash`, the returned meta bytes are still `Ready`, and `regions.activate_for_emu()` remains possible after `Start`.
