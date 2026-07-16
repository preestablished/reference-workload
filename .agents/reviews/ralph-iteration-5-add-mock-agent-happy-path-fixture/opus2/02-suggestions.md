# Suggestions

## Document or isolate the intentional control-message pipelining

- File: `crates/refwork-harness/tests/mock_agent.rs:35`

The fixture sends `Start` and the initial `HashRequest` window before it has read `GameLoaded`, `RegisterRegion`, or `Ready`. The receive order is still valid from the harness perspective, and it avoids a race where frame 1 completes before the first hash request is queued, but this looks like an accidental handshake violation on first read. Add a short comment or helper name that makes the intentional pipelining explicit.

```rust
// Queue Start and the initial hash window before draining Ready so frame 1's
// request is already present when the free-running frame loop begins. The
// harness still observes the messages in protocol order: Start, then hashes.
send_msg(agent_fd.as_raw_fd(), &CtlMsg::Start {});
for frame in 1..=REQUEST_WINDOW {
    send_msg(agent_fd.as_raw_fd(), &CtlMsg::HashRequest { frame });
}
```

## Make test sends mirror production retry behavior

- File: `crates/refwork-harness/tests/mock_agent.rs:223`

`send_msg` treats every negative `send` as an assertion failure. These datagrams are small, so this is unlikely to fail in practice, but retrying `EINTR` and using `MSG_NOSIGNAL` would match the production transport and reduce noise if the child exits early while the fixture is still sending.

```rust
fn send_msg(fd: RawFd, msg: &CtlMsg) {
    let bytes = refwork_protocol::encode(msg).expect("encode control message");
    loop {
        let sent = unsafe {
            libc::send(fd, bytes.as_ptr().cast(), bytes.len(), libc::MSG_NOSIGNAL)
        };
        if sent == bytes.len() as isize {
            return;
        }
        let err = io::Error::last_os_error();
        if sent < 0 && err.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        panic!("send failed for {msg:?}: {err}");
    }
}
```

## Include the long fixture in README with the locked command used in CI

- File: `README.md:10`

The local command works, but this repository has been using locked Cargo invocations for verification. Including `--locked` in the documented command makes the fixture instructions match the normal gate style and avoids accidentally updating `Cargo.lock` while someone is just trying to run the integration test.

```sh
cargo test --locked -p refwork-harness --test mock_agent -- mock_agent_happy_path_1000_frames --nocapture
```
