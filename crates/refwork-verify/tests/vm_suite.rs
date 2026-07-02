//! `vm-suite` staged-fixture tests (plan step 05, `refwork-d7t.13`/`.14`).
//!
//! Run against the in-process mock `HypervisorWorker` — these prove the
//! suite's state machine, hashing, restore-continuity plumbing, and (via
//! the deliberate perturbation) that the suite CAN fail. The M5
//! determinism claim itself comes from the lab run against the real
//! worker + image; the mock is not a determinism substrate.

use std::path::PathBuf;

use refwork_dh_client::mock::{spawn_uds, MockFixture};
use refwork_verify::vm_suite::{vm_suite, write_suite_report, VmSuiteOptions};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "refwork-vm-suite-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A pad stream with changes (so injection and pv-pad hold semantics are
/// both exercised): 4 idle, 6 pressing, 3 idle, 3 pressing.
const SUITE_SCRIPT: &str = "padlog v1\n4x0000\n6x0001\n3x0000\n3x0002\n";

fn options(dir: &TempDir, uds: &std::path::Path) -> VmSuiteOptions {
    let script = dir.path.join("run.padlog");
    std::fs::write(&script, SUITE_SCRIPT).unwrap();
    VmSuiteOptions {
        worker: uds.to_string_lossy().into_owned(),
        snapshot_ref: "ab".repeat(32), // MockFixture::default().ready_snapshot_ref
        script,
        ram_region: "wram".to_owned(),
        ram_size: 131072,
        frames: None,
        snapshot_at: None, // frames/2 = 8, inside the pressing stretch
        iterations: 2,
        report: dir.path.join("suite.json"),
        nondet_test: false,
        port: 0,
    }
}

#[test]
fn suite_passes_on_the_deterministic_fixture() {
    let dir = TempDir::new("pass");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let opts = options(&dir, &uds);
    let report = vm_suite(&opts).unwrap();

    assert!(
        report.passed(),
        "failures: {:?}, iterations: {:?}",
        report.failures,
        report
            .iterations
            .iter()
            .map(|i| (i.double_run.ok, i.restore_continuity.ok))
            .collect::<Vec<_>>()
    );
    assert_eq!(report.iterations.len(), 2);
    assert_eq!(report.frames, 16);
    assert_eq!(report.snapshot_at, 8);
    // Same fixture + same log => identical trajectories across iterations.
    assert_eq!(
        report.iterations[0].trajectory_blake3,
        report.iterations[1].trajectory_blake3
    );

    let hash = write_suite_report(&report, &opts.report).unwrap();
    assert_eq!(hash.len(), 64);
    assert!(opts.report.with_extension("json.b3").exists());
}

#[test]
fn negative_test_perturbed_input_must_fail_double_run() {
    let dir = TempDir::new("negative");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let mut opts = options(&dir, &uds);
    opts.iterations = 1;
    opts.nondet_test = true;
    let report = vm_suite(&opts).unwrap();

    assert!(!report.passed(), "a suite that cannot fail proves nothing");
    let iteration = &report.iterations[0];
    assert!(!iteration.double_run.ok);
    // The perturbation lands at snapshot_at (frame 8) — divergence is
    // detected at that frame's hash.
    assert_eq!(iteration.double_run.first_divergent_frame, Some(8));
    // The uninterrupted reference and its own continuation stay consistent.
    assert!(iteration.restore_continuity.ok);
}

#[test]
fn unknown_ready_ref_is_a_restore_failure() {
    let dir = TempDir::new("badref");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let mut opts = options(&dir, &uds);
    opts.snapshot_ref = "ee".repeat(32);
    let report = vm_suite(&opts).unwrap();

    assert!(!report.passed());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].stage, "restore");
    assert_eq!(report.failures[0].code, "not_found");
    assert_eq!(report.failures[0].iteration, 1);
}
