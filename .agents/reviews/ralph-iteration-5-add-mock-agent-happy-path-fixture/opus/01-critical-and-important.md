# Critical and Important Issues

## Critical

None.

## Important

### Important: Rolling `HashRequest` window races the free-running harness

- File: `crates/refwork-harness/tests/mock_agent.rs:42`
- File: `crates/refwork-harness/tests/mock_agent.rs:52`
- File: `crates/refwork-harness/tests/mock_agent.rs:55`

The fixture keeps only a 32-frame request window ahead of the harness and refills that window after each received `HashReport`. The production frame loop free-runs and polls control nonblocking once per completed frame, so it can outrun the test process by more than 32 frames. Once that happens, a later refill request is stale, the harness faults/exits, and the test fails with `Broken pipe`.

I reproduced this locally with:

```sh
cargo test --locked -p refwork-harness --test mock_agent -- mock_agent_happy_path_1000_frames --nocapture
```

Observed failures included `send failed for HashRequest { frame: 278 }: Broken pipe`, `frame: 829`, and `frame: 852`.

Suggested fix: after observing `Ready`, send `Start` and queue all expected hash requests before entering the report-validation loop. That keeps the fixture deterministic while still honoring the setup handshake causality.

```rust
expect_game_loaded(agent_fd.as_raw_fd(), &rom);
let regions = expect_regions_until_ready(agent_fd.as_raw_fd());
assert_required_regions(&regions);

send_msg(agent_fd.as_raw_fd(), &CtlMsg::Start {});
for frame in 1..=FRAMES {
    send_msg(agent_fd.as_raw_fd(), &CtlMsg::HashRequest { frame });
}

let mut direct = DirectRun::new(&rom);
for frame in 1..=FRAMES {
    let report = expect_hash_report(agent_fd.as_raw_fd(), frame);
    let expected = direct.run_frame(frame);
    assert_eq!(report.wram, expected.wram, "wram hash mismatch at frame {frame}");
    assert_eq!(report.fb, expected.fb, "framebuffer hash mismatch at frame {frame}");
}

send_msg(agent_fd.as_raw_fd(), &CtlMsg::Shutdown {});
wait_for_success(&mut child);
```

### Important: Child process is not cleaned up if the test panics before `wait_for_success`

- File: `crates/refwork-harness/tests/mock_agent.rs:25`
- File: `crates/refwork-harness/tests/mock_agent.rs:77`
- File: `crates/refwork-harness/tests/mock_agent.rs:299`

The harness child is only killed on the timeout path inside `wait_for_success`. Any earlier assertion, recv timeout, hash mismatch, or send failure skips that function entirely. `std::process::Child` does not kill or wait on drop, so a failed test can leave a running `refwork-harness` process behind.

Suggested fix: wrap the child in a guard that kills and waits unless the success path has already reaped it.

```rust
struct HarnessChild {
    child: Child,
    reaped: bool,
}

impl Drop for HarnessChild {
    fn drop(&mut self) {
        if !self.reaped {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
    }
}

fn wait_for_success(harness: &mut HarnessChild) {
    for _ in 0..1_000 {
        match harness.child.try_wait().expect("poll child") {
            Some(status) if status.success() => {
                harness.reaped = true;
                return;
            }
            Some(status) => {
                harness.reaped = true;
                let stderr = read_child_stderr(&mut harness.child);
                panic!("harness exited with {status}: {stderr}");
            }
            None => unsafe { libc::usleep(10_000) },
        }
    }

    let _ = harness.child.kill();
    let _ = harness.child.wait();
    harness.reaped = true;
    let stderr = read_child_stderr(&mut harness.child);
    panic!("harness did not exit after Shutdown: {stderr}");
}
```
