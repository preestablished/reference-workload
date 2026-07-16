# Critical And Important Issues

## Critical

None.

## Important

### Important: Child process can inherit the agent socket and is not killed/reaped on early test failure

- File: `crates/refwork-harness/tests/mock_agent.rs:241`
- File: `crates/refwork-harness/tests/mock_agent.rs:272`
- File: `crates/refwork-harness/tests/mock_agent.rs:299`

`seqpacket_pair` creates both socket fds without `SOCK_CLOEXEC`, and `spawn_harness` only closes the harness-side raw fd after `dup2`. If the socketpair fds are not allocated as `3` and `4`, the child can also inherit the agent-side fd across `exec`. The fixture also returns a bare `Child`, so any assertion or timeout before `wait_for_success` drops the handle without killing or reaping the harness. Those two behaviors combine badly: on an early panic, the child may keep its own copy of the agent socket open, never see EOF on fd 3, and continue running or remain as an unreaped process during the rest of the test run.

Suggested fix: make the socketpair close-on-exec by default, explicitly clear close-on-exec only for fd 3 in the child, and wrap the child in a guard that kills/reaps it unless the success path has already waited.

```rust
struct HarnessChild {
    child: Child,
}

impl HarnessChild {
    fn wait_for_success(mut self) {
        wait_for_success(&mut self.child);
    }
}

impl Drop for HarnessChild {
    fn drop(&mut self) {
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

fn seqpacket_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    assert_eq!(rc, 0, "socketpair failed: {}", io::Error::last_os_error());
    let left = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let right = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    (left, right)
}

fn spawn_harness(harness_fd: OwnedFd) -> HarnessChild {
    let raw_fd = harness_fd.as_raw_fd();
    let mut command = Command::new(env!("CARGO_BIN_EXE_refwork-harness"));
    command
        .arg("--fd3")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            if raw_fd != CONTROL_FD {
                if libc::dup2(raw_fd, CONTROL_FD) < 0 {
                    return Err(io::Error::last_os_error());
                }
                libc::close(raw_fd);
            }

            let flags = libc::fcntl(CONTROL_FD, libc::F_GETFD);
            if flags < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::fcntl(CONTROL_FD, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = command.spawn().expect("spawn refwork-harness");
    drop(harness_fd);
    HarnessChild { child }
}
```
