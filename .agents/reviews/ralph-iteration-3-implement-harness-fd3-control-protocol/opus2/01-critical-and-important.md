# Critical And Important Issues

## Critical

None found.

## Important

### 1. Closed peer can terminate the harness with SIGPIPE instead of returning a deterministic control error

- Severity: Important
- File/lines: `crates/refwork-harness/src/ctl.rs:166-171`

`SeqpacketFd::send_datagram` calls `libc::send(..., flags = 0)`. On Linux connection-oriented sockets, including `socketpair(AF_UNIX, SOCK_SEQPACKET)`, sending after the peer has closed can raise SIGPIPE and terminate the process before Rust sees `EPIPE`. That bypasses the intended `ControlError::Io` path and makes closed-peer behavior nondeterministic from the runner's point of view.

Suggested fix:

```rust
#[cfg(target_os = "linux")]
const SEND_FLAGS: libc::c_int = libc::MSG_NOSIGNAL;
#[cfg(not(target_os = "linux"))]
const SEND_FLAGS: libc::c_int = 0;

let n = unsafe {
    libc::send(
        self.fd.as_raw_fd(),
        bytes.as_ptr().cast(),
        bytes.len(),
        SEND_FLAGS,
    )
};
```

Add a real `socketpair(AF_UNIX, SOCK_SEQPACKET)` test that drops the peer before a harness send and asserts `send_msg` returns a `BrokenPipe`/`EPIPE`-style error instead of killing the test process.

### 2. Bad datagrams after `Ready` leave `meta.status` as ready

- Severity: Important
- File/lines: `crates/refwork-harness/src/runner.rs:139-144`, `crates/refwork-harness/src/runner.rs:152-164`, `crates/refwork-harness/src/runner.rs:231-235`

After regions and meta are published, `expect_start` only calls `mark_meta_fault` when a valid but out-of-order `CtlMsg` is received. If the agent sends a malformed datagram or an oversize datagram while the harness is waiting for `Start`, `recv_agent_msg` emits a `Fault { code: BadProto }` on the control socket and returns, but the meta page remains `Ready`. That violates the intended observable failure state for faults after the meta region exists.

Suggested fix:

```rust
fn expect_start<T>(
    channel: &mut ControlChannel<T>,
    regions: &mut HarnessRegions,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    let msg = match channel.recv_msg() {
        Ok(msg) => msg,
        Err(ControlError::Oversize { len }) => {
            mark_meta_fault(regions, FaultCode::BadProto);
            let detail = format!("oversize control datagram: {len} bytes");
            send_fault(channel, FaultCode::BadProto, &detail)?;
            return Err(SetupError::BadProto { detail });
        }
        Err(ControlError::Decode(err)) => {
            mark_meta_fault(regions, FaultCode::BadProto);
            let detail = err.to_string();
            send_fault(channel, FaultCode::BadProto, &detail)?;
            return Err(SetupError::BadProto { detail });
        }
        Err(err) => return Err(SetupError::Control(err)),
    };

    match msg {
        CtlMsg::Start {} => Ok(()),
        actual => {
            mark_meta_fault(regions, FaultCode::ProtocolOrder);
            protocol_order(channel, "Start", actual)
        }
    }
}
```

Add tests that inspect the returned/dropped meta bytes for post-`Ready` malformed and oversize datagrams, not just the outbound `Fault` message.

### 3. Peer-controlled fault detail can exceed `MAX_DATAGRAM` and prevent the `Fault` from being sent

- Severity: Important
- File/lines: `crates/refwork-harness/src/game.rs:44-45`, `crates/refwork-harness/src/runner.rs:177-182`, `crates/refwork-harness/src/runner.rs:277-294`

`send_fault` forwards arbitrary detail text into `CtlMsg::Fault`, and `ControlChannel::send_msg` then applies the protocol crate's `MAX_DATAGRAM` limit. Several details include peer-controlled content: `GameLoadError::Read` includes the full `LoadGame.dev_path`, and `protocol_order` formats the full unexpected `CtlMsg` via `Debug`. A near-limit but valid incoming `LoadGame` can therefore make the outbound fault too large, causing `EncodeError::Oversize` and suppressing the deterministic `BadGame` or `ProtocolOrder` fault that the state machine is trying to emit.

Suggested fix:

```rust
const MAX_FAULT_DETAIL_BYTES: usize = 512;

fn bounded_fault_detail(detail: &str) -> String {
    let mut end = detail.len().min(MAX_FAULT_DETAIL_BYTES);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end < detail.len() {
        format!("{}...", &detail[..end])
    } else {
        detail.into()
    }
}

fn send_fault<T>(
    channel: &mut ControlChannel<T>,
    code: FaultCode,
    detail: &str,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    channel.send_msg(&CtlMsg::Fault {
        frame: 0,
        code,
        detail: bounded_fault_detail(detail),
    })?;
    Ok(())
}
```

Add regression tests with a very long `LoadGame.dev_path` that fails to open and with an out-of-order long `LoadGame` at the wrong state; both should still emit a decodable `Fault`.
