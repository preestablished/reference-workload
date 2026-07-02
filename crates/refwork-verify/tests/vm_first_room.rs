//! `vm-first-room` staged-fixture tests (plan step 04 CI requirement).
//!
//! These run against the in-process mock `HypervisorWorker`
//! (`refwork_dh_client::mock`) — no KVM, no dh-workerd, no game content —
//! and exercise the full client path over a real UDS gRPC connection: the
//! gate state machine, report generation, and the distinct sanitized
//! failure modes. The real-worker leg of `refwork-d7t.11` is the lab dry
//! run, not this test.

use std::path::{Path, PathBuf};

use refwork_dh_client::mock::{spawn_uds, MockFixture, FB_BYTES};
use refwork_verify::vm_first_room::{vm_first_room, write_report, VmFirstRoomOptions};

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
            "refwork-vm-first-room-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Feature map describing the mock fixture: one-byte `room_id` at wram
/// offset 0x40 (see `MockFixture::default`).
const FIXTURE_MAP: &str = r#"
schema_version: 1
kind: feature-map
meta:
  name: staged-fixture
  workload: refwork-demo
  game_revision: "mock"
  version: 1
regions:
  - { name: wram, size: 131072 }
features:
  - name: room_id
    region: wram
    offset: 0x40
    type: u8
    semantics: room_id
    description: "Mock room index"
    stability: stable
    discretize: { kind: identity }
"#;

/// 16 frames holding a non-zero pad word; the default mock fixture
/// transitions after 8 non-zero pad frames.
const FIXTURE_SCRIPT: &str = "padlog v1\n16x0001\n";

fn fixture_expect(checkpoint_16_blake3: Option<&str>) -> String {
    let fb_line = match checkpoint_16_blake3 {
        Some(hash) => format!("  - {{ frame: 16, blake3: \"{hash}\" }}\n"),
        None => "  - { frame: 16 }\n".to_owned(),
    };
    format!(
        "schema_version: 1\n\
         kind: vm-first-room-expect\n\
         room_feature: room_id\n\
         initial_room: 0\n\
         target_room: 1\n\
         frame_budget: 16\n\
         step: 4\n\
         framebuffer_checkpoints:\n\
         {fb_line}"
    )
}

fn ready_ref_hex() -> String {
    // MockFixture::default().ready_snapshot_ref is [0xAB; 32].
    "ab".repeat(32)
}

fn options(dir: &TempDir, uds: &Path, expect: &str) -> VmFirstRoomOptions {
    VmFirstRoomOptions {
        worker: uds.to_string_lossy().into_owned(),
        snapshot_ref: ready_ref_hex(),
        script: dir.file("first-room.padlog", FIXTURE_SCRIPT),
        map: dir.file("map.yaml", FIXTURE_MAP),
        expect: dir.file("expect.yaml", expect),
        image_manifest: None,
        report: dir.path.join("report.json"),
        frames: None,
        port: 0,
    }
}

/// Expected mock framebuffer hash at frame 16 with the room transitioned:
/// fill byte = room * 16 ^ frame_counter = 16 ^ 16 = 0.
fn mock_fb_blake3_at_frame_16() -> String {
    let fill = 1u8.wrapping_mul(16) ^ 16u8;
    blake3::hash(&vec![fill; FB_BYTES]).to_hex().to_string()
}

#[test]
fn staged_fixture_first_room_gate_passes() {
    let dir = TempDir::new("pass");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let expected_fb = mock_fb_blake3_at_frame_16();
    let opts = options(&dir, &uds, &fixture_expect(Some(&expected_fb)));
    let report = vm_first_room(&opts).unwrap();

    assert!(
        report.passed(),
        "expected pass, got failures: {:?}",
        report.failures
    );
    let ready = report.ready_proof.as_ref().unwrap();
    assert_eq!(ready.meta_status, 1, "meta.status must be ready");
    assert_eq!(ready.meta_frame, 0);
    assert_eq!(ready.room_feature_value, 0);
    assert_eq!(ready.framebuffer_bytes, FB_BYTES);
    let transition = report.room_transition.as_ref().unwrap();
    assert_eq!(transition.from, 0);
    assert_eq!(transition.to, 1);
    assert_eq!(transition.observed_by_frame, 8);
    assert_eq!(report.pad_trace_ok, Some(true));
    assert_eq!(report.framebuffer_checkpoints.len(), 1);
    assert_eq!(report.framebuffer_checkpoints[0].matched, Some(true));

    // Report + BLAKE3 sidecar land on disk.
    let hash = write_report(&report, &opts.report).unwrap();
    let written = std::fs::read_to_string(&opts.report).unwrap();
    assert_eq!(blake3::hash(written.as_bytes()).to_hex().to_string(), hash);
    assert!(opts.report.with_extension("json.b3").exists());
    // Clean-room shape: no pixel dumps — the largest string field is a hash.
    assert!(written.len() < 8192, "report should be small: {written}");
}

#[test]
fn dry_run_records_observed_framebuffer_hash() {
    let dir = TempDir::new("dry");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let opts = options(&dir, &uds, &fixture_expect(None));
    let report = vm_first_room(&opts).unwrap();

    assert!(report.passed());
    let checkpoint = &report.framebuffer_checkpoints[0];
    assert_eq!(checkpoint.expected_blake3, None);
    assert_eq!(checkpoint.matched, None);
    assert_eq!(checkpoint.blake3, mock_fb_blake3_at_frame_16());
}

#[test]
fn unknown_snapshot_ref_is_a_distinct_restore_failure() {
    let dir = TempDir::new("badref");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let mut opts = options(&dir, &uds, &fixture_expect(None));
    opts.snapshot_ref = "cd".repeat(32);
    let report = vm_first_room(&opts).unwrap();

    assert!(!report.passed());
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].stage, "restore");
    assert_eq!(report.failures[0].code, "not_found");
    assert!(report.failures[0].message.contains("unknown snapshot ref"));
}

#[test]
fn region_not_in_manifest_is_a_ready_proof_failure() {
    let dir = TempDir::new("badregion");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let mut opts = options(&dir, &uds, &fixture_expect(None));
    opts.map = dir.file(
        "bad-map.yaml",
        &FIXTURE_MAP
            .replace("name: wram, size: 131072", "name: vram, size: 131072")
            .replace("region: wram", "region: vram"),
    );
    let report = vm_first_room(&opts).unwrap();

    assert!(!report.passed());
    assert_eq!(report.failures[0].stage, "ready-proof");
    assert_eq!(report.failures[0].code, "failed_precondition");
    assert!(report.failures[0].message.contains("vram"));
}

#[test]
fn missing_transition_fails_without_worker_errors() {
    let dir = TempDir::new("notransition");
    let uds = dir.path.join("worker.sock");
    let _mock = spawn_uds(MockFixture::default(), &uds).unwrap();

    let mut opts = options(&dir, &uds, &fixture_expect(None));
    // All-zero pads: the fixture never transitions.
    opts.script = dir.file("idle.padlog", "padlog v1\n16x0000\n");
    let report = vm_first_room(&opts).unwrap();

    assert!(!report.passed());
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert!(report.room_transition.is_none());
}

#[test]
fn dead_worker_is_a_connect_failure() {
    let dir = TempDir::new("dead");
    let uds = dir.path.join("nobody-home.sock");
    let opts = options(&dir, &uds, &fixture_expect(None));
    let report = vm_first_room(&opts).unwrap();

    assert!(!report.passed());
    assert_eq!(report.failures[0].stage, "connect");
    assert_eq!(report.failures[0].code, "connect");
}
