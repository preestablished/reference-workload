# Suggestions

### Use `MSG_NOSIGNAL` in the test sender

- File: `crates/refwork-harness/tests/mock_agent.rs:225`

The production transport already avoids SIGPIPE on closed peers. The test sender should follow the same pattern so harness regressions always surface as assertion failures with useful diagnostics.

```rust
let sent = unsafe {
    libc::send(
        fd,
        bytes.as_ptr().cast(),
        bytes.len(),
        libc::MSG_NOSIGNAL,
    )
};
```

### Avoid inheriting unrelated socket ends into the child process

- File: `crates/refwork-harness/tests/mock_agent.rs:241`
- File: `crates/refwork-harness/tests/mock_agent.rs:281`

`socketpair` currently creates both fds without close-on-exec. The child only needs fd 3 after `dup2`; inheriting the agent side can hide EOF behavior and make failures harder to reason about. Prefer `SOCK_CLOEXEC`, then explicitly make fd 3 inheritable in `pre_exec`.

```rust
let rc = unsafe {
    libc::socketpair(
        libc::AF_UNIX,
        libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
        0,
        fds.as_mut_ptr(),
    )
};

command.pre_exec(move || {
    if raw_fd != CONTROL_FD {
        if libc::dup2(raw_fd, CONTROL_FD) < 0 {
            return Err(io::Error::last_os_error());
        }
        libc::close(raw_fd);
    }
    if libc::fcntl(CONTROL_FD, libc::F_SETFD, 0) < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
});
```

### Include `--locked` in the README test command

- File: `README.md:11`

The fixture is intended as a reproducible workflow check, and the branch updates `Cargo.lock`. Including `--locked` in the documented command matches the validation style used elsewhere in this repo.

```sh
cargo test --locked -p refwork-harness --test mock_agent -- mock_agent_happy_path_1000_frames --nocapture
```
