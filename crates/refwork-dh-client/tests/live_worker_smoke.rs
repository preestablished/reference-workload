//! Live-worker smoke test: transport, codec, and error-mapping proof for
//! `refwork-dh-client` against a real `dh-workerd`, without a bootable
//! snapshot (plan: rom-operator-bridge phase3-followups-closeout step 04).
//!
//! Gated twice: `REFWORK_VM_TESTS=1` (the vm-gates lane convention) and
//! `REFWORK_DH_WORKERD_BIN` (path to a dh-workerd binary — in CI a job step
//! builds it from a pinned determinism-hypervisor rev; locally point it at
//! a clean-worktree build). Skips with a message when either is unset, so
//! plain `cargo test` is unaffected.
//!
//! The worker is launched with scratch paths and `--no-snapstore`; the
//! deployed worker's socket (`/run/dh/grpc.sock`) is never touched.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use refwork_dh_client::{DhClientError, WorkerEndpoint, WorkerSession};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("refwork-live-smoke-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

struct WorkerGuard {
    child: Child,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn_worker(bin: &str, dir: &TempDir) -> (WorkerGuard, PathBuf) {
    let uds = dir.path.join("grpc.sock");
    let cache = dir.path.join("image-cache");
    std::fs::create_dir_all(&cache).unwrap();
    // Every path flag is explicit: the binary's *defaults* are the deployed
    // paths. TCP/HTTP go to port 0 so parallel runs cannot collide.
    let child = Command::new(bin)
        .args([
            "serve",
            "--uds",
            uds.to_str().unwrap(),
            "--tcp",
            "127.0.0.1:0",
            "--http",
            "127.0.0.1:0",
            "--image-cache",
            cache.to_str().unwrap(),
            "--no-snapstore",
            "--skip-preflight",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dh-workerd");
    let mut guard = WorkerGuard { child };

    // Wait for the UDS to accept; fail fast if the worker exits.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = guard.child.try_wait().expect("try_wait dh-workerd") {
            panic!("dh-workerd exited during startup: {status}");
        }
        if uds.exists() && std::os::unix::net::UnixStream::connect(&uds).is_ok() {
            return (guard, uds);
        }
        assert!(
            Instant::now() < deadline,
            "dh-workerd did not open {} within 30s",
            uds.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn gate() -> Option<String> {
    if std::env::var("REFWORK_VM_TESTS").as_deref() != Ok("1") {
        eprintln!("skipping live-worker smoke: REFWORK_VM_TESTS!=1");
        return None;
    }
    match std::env::var("REFWORK_DH_WORKERD_BIN") {
        Ok(bin) if !bin.is_empty() => Some(bin),
        _ => {
            eprintln!(
                "skipping live-worker smoke: REFWORK_DH_WORKERD_BIN unset \
                 (path to a dh-workerd binary; build from a clean \
                 determinism-hypervisor worktree)"
            );
            None
        }
    }
}

#[test]
fn live_worker_transport_and_error_mapping() {
    let Some(bin) = gate() else { return };
    let dir = TempDir::new();
    let (guard, uds) = spawn_worker(&bin, &dir);

    let endpoint = WorkerEndpoint::parse(uds.to_str().unwrap());
    let mut session = WorkerSession::connect(&endpoint).expect("connect over scratch UDS");

    // Transport + codec proof: a real round-trip through tonic/prost.
    let info = session.worker_info().expect("GetWorkerInfo round-trips");
    assert!(
        info.slots_free <= info.slots_total && !info.worker_id.is_empty(),
        "worker info is shaped: {info:?}"
    );

    // Error-mapping proof: a bogus snapshot ref must come back as a
    // distinct, sanitized RPC error — under --no-snapstore this surfaces
    // as the worker's snapshot-path failure (not_found or a
    // snapstore-unavailable class), never a hang or a malformed response.
    let err = session
        .restore_snapshot(vec![0xEE; 32], Vec::new())
        .expect_err("bogus ref must fail");
    match &err {
        DhClientError::Rpc { code, message } => {
            assert!(
                !message.is_empty(),
                "rpc error must carry a message: {err:?}"
            );
            // Record the observed class for the evidence trail.
            eprintln!("bogus-ref restore mapped to: {code:?} / {}", err.code_str());
        }
        other => panic!("expected an Rpc error, got {other:?}"),
    }

    // Timeout proof: a stopped worker maps to a connect-class failure,
    // promptly, not a hang.
    drop(guard);
    let started = Instant::now();
    let reconnect = WorkerSession::connect(&WorkerEndpoint::parse(uds.to_str().unwrap()));
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(20),
        "dead-socket connect must fail promptly, took {elapsed:?}"
    );
    match reconnect {
        Err(DhClientError::Connect { .. }) => {}
        Err(other) => panic!("expected Connect error, got {other:?}"),
        Ok(mut session) => {
            // Some kernels accept the connect on a lingering socket file;
            // the first RPC must then fail instead.
            let err = session.worker_info().expect_err("dead worker cannot serve");
            eprintln!("dead-worker RPC mapped to: {}", err.code_str());
        }
    }
}
