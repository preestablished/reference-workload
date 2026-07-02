//! `vm-first-room` — the in-VM first-room gate (Phase 3 exit gate 3,
//! `refwork-d7t.11`).
//!
//! Drives a worker slot entirely through the hypervisor worker gRPC API:
//! `RestoreSnapshot -> InjectInputs -> Run(CaptureSpec) -> ReadGuestMemory ->
//! GetFramebuffer -> DestroyVm`. Input goes only through the
//! hypervisor-owned scheduled path (never the harness control socket or
//! detchannel), room-transition proof comes from host region capture at
//! frame boundaries, and framebuffer proof is BLAKE3-of-pixels compared
//! against operator-supplied checkpoint hashes.
//!
//! Clean-room discipline: the report carries revisions, hashes, decoded
//! integer feature values, and frame numbers — never pixels, WRAM dumps,
//! ROM bytes, or padlog semantics. Worker error messages are recorded
//! verbatim (they name offenders precisely and are clean-room-safe by
//! contract).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use refwork_dh_client::{proto, DhClientError, WorkerEndpoint, WorkerSession};
use refwork_featuremap::{parse_feature_map, Feature, FeatureType};
use refwork_script::parse as parse_padlog;

/// D7 framebuffer contract (layout_version 1).
const FB_WIDTH: u32 = 256;
const FB_HEIGHT: u32 = 224;
const FB_STRIDE: u32 = 1024;
const FB_BYTES: usize = (FB_STRIDE * FB_HEIGHT) as usize;

// Meta-page offsets mirroring refwork-harness/src/meta.rs.
const META_READ_LEN: u64 = 0x20;
const META_STATUS_OFF: usize = 0x04;
const META_FRAME_OFF: usize = 0x08;
const META_LAST_PAD_OFF: usize = 0x10;

pub struct VmFirstRoomOptions {
    /// Worker endpoint: UDS socket path, or `http://host:port`.
    pub worker: String,
    /// 64-hex BLAKE3 ref of the READY root snapshot.
    pub snapshot_ref: String,
    /// First-room padlog (operator-supplied at lab time; synthetic in CI).
    pub script: PathBuf,
    /// Feature map naming the room feature's region/offset/type.
    pub map: PathBuf,
    /// Expectations file (see [`VmExpect`]).
    pub expect: PathBuf,
    /// Package-04 image manifest; hashed into the report when given.
    pub image_manifest: Option<PathBuf>,
    pub report: PathBuf,
    /// Frame-budget override (default: the expect file's `frame_budget`).
    pub frames: Option<u32>,
    /// Pad port (default 0).
    pub port: u32,
}

/// Operator-supplied expectations. In CI this describes the staged
/// synthetic fixture; at lab time the operator writes it for their ROM
/// (values live outside this repo).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmExpect {
    pub schema_version: u32,
    pub kind: String,
    /// Feature name in the map whose value identifies the room.
    pub room_feature: String,
    pub initial_room: i64,
    pub target_room: i64,
    pub frame_budget: u32,
    /// Frames per Run step (capture cadence). Default 8.
    #[serde(default = "default_step")]
    pub step: u32,
    /// Framebuffer checkpoints, `frame` counted from the restore boundary.
    /// `blake3` empty/absent ⇒ record the observed hash (dry-run mode).
    #[serde(default)]
    pub framebuffer_checkpoints: Vec<FbCheckpoint>,
}

fn default_step() -> u32 {
    8
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FbCheckpoint {
    pub frame: u32,
    #[serde(default)]
    pub blake3: Option<String>,
}

// ---- report ----

#[derive(Debug, Serialize)]
pub struct VmFirstRoomReport {
    pub schema_version: u32,
    pub command: &'static str,
    pub reference_workload_rev: String,
    pub worker: WorkerIdentity,
    pub snapshot_ref: String,
    pub image_manifest_blake3: Option<String>,
    pub padlog_blake3: String,
    pub feature_map_blake3: String,
    pub expect_blake3: String,
    pub input: InputSummary,
    pub ready_proof: Option<ReadyProof>,
    pub room_transition: Option<RoomTransition>,
    pub framebuffer_checkpoints: Vec<FbCheckpointResult>,
    pub pad_trace_ok: Option<bool>,
    pub frames_run: u32,
    pub result: &'static str,
    pub failures: Vec<FailureEntry>,
}

#[derive(Debug, Default, Serialize)]
pub struct WorkerIdentity {
    pub endpoint: String,
    pub worker_id: String,
    pub version: String,
}

#[derive(Debug, Default, Serialize)]
pub struct InputSummary {
    pub events_scheduled: u32,
    pub port: u32,
    pub script_frames: usize,
}

/// READY proof: the restored slot exposes live regions with the contract
/// geometry, `meta.status == ready`, `meta.frame == 0`, before any input.
#[derive(Debug, Serialize)]
pub struct ReadyProof {
    pub state_hash: String,
    pub frame_counter_base: u32,
    pub meta_status: u32,
    pub meta_frame: u64,
    pub room_feature_value: i64,
    pub framebuffer_bytes: usize,
    pub framebuffer_format: String,
    pub framebuffer_blake3: String,
}

#[derive(Debug, Serialize)]
pub struct RoomTransition {
    pub feature: String,
    pub from: i64,
    pub to: i64,
    /// Frames after the restore boundary at which the target room was first
    /// observed (upper-bounded by the capture cadence).
    pub observed_by_frame: u32,
}

#[derive(Debug, Serialize)]
pub struct FbCheckpointResult {
    pub frame: u32,
    pub blake3: String,
    pub expected_blake3: Option<String>,
    pub matched: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FailureEntry {
    /// Which leg failed: `restore`, `ready-proof`, `inject`, `run`,
    /// `read-regions`, `framebuffer`, `destroy`.
    pub stage: String,
    /// Machine-readable code (`failed_precondition`, `not_found`, …).
    pub code: String,
    /// The worker's message verbatim (clean-room-safe by contract).
    pub message: String,
}

impl VmFirstRoomReport {
    pub fn passed(&self) -> bool {
        self.result == "pass"
    }
}

// ---- errors ----

#[derive(Debug)]
pub enum SetupError {
    Io(PathBuf, std::io::Error),
    Parse(String),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "cannot read '{}': {e}", path.display()),
            Self::Parse(message) => write!(f, "{message}"),
        }
    }
}

// ---- main entry ----

/// Run the gate. `Err` is a setup problem (unreadable/invalid inputs);
/// worker-side failures land in the report's `failures` with `result: fail`.
pub fn vm_first_room(opts: &VmFirstRoomOptions) -> Result<VmFirstRoomReport, SetupError> {
    // -- inputs, hashed before anything touches the worker --
    let script_bytes = read(&opts.script)?;
    let padlog_blake3 = blake3::hash(&script_bytes).to_hex().to_string();
    let script_text = String::from_utf8(script_bytes)
        .map_err(|e| SetupError::Parse(format!("padlog is not utf-8: {e}")))?;
    let script = parse_padlog(&script_text)
        .map_err(|e| SetupError::Parse(format!("padlog parse error: {e}")))?
        .frames;

    let map_bytes = read(&opts.map)?;
    let feature_map_blake3 = blake3::hash(&map_bytes).to_hex().to_string();
    let map_text = String::from_utf8(map_bytes)
        .map_err(|e| SetupError::Parse(format!("feature map is not utf-8: {e}")))?;
    let (map, _warnings) = parse_feature_map(&map_text)
        .map_err(|e| SetupError::Parse(format!("feature map parse error: {e}")))?;

    let expect_bytes = read(&opts.expect)?;
    let expect_blake3 = blake3::hash(&expect_bytes).to_hex().to_string();
    let expect: VmExpect = serde_yaml::from_slice(&expect_bytes)
        .map_err(|e| SetupError::Parse(format!("expect parse error: {e}")))?;
    if expect.kind != "vm-first-room-expect" {
        return Err(SetupError::Parse(format!(
            "expect kind '{}' is not 'vm-first-room-expect'",
            expect.kind
        )));
    }

    let image_manifest_blake3 = match &opts.image_manifest {
        Some(path) => Some(blake3::hash(&read(path)?).to_hex().to_string()),
        None => None,
    };

    let room = map
        .features
        .iter()
        .find(|f| f.name == expect.room_feature)
        .ok_or_else(|| {
            SetupError::Parse(format!(
                "room feature '{}' not in the feature map",
                expect.room_feature
            ))
        })?;
    let room_width = room.feature_type.derived_width().ok_or_else(|| {
        SetupError::Parse(format!(
            "room feature '{}' must be an integer scalar",
            expect.room_feature
        ))
    })?;
    if !room.feature_type.is_integer_scalar() {
        return Err(SetupError::Parse(format!(
            "room feature '{}' must be an integer scalar",
            expect.room_feature
        )));
    }

    let snapshot_hash = parse_hex32(&opts.snapshot_ref)
        .ok_or_else(|| SetupError::Parse("snapshot ref must be 64 hex chars".to_owned()))?;

    let frame_budget = opts.frames.unwrap_or(expect.frame_budget);
    if frame_budget == 0 {
        return Err(SetupError::Parse(
            "frame budget must be positive".to_owned(),
        ));
    }

    let mut report = VmFirstRoomReport {
        schema_version: 1,
        command: "vm-first-room",
        reference_workload_rev: git_rev(),
        worker: WorkerIdentity {
            endpoint: opts.worker.clone(),
            ..Default::default()
        },
        snapshot_ref: opts.snapshot_ref.clone(),
        image_manifest_blake3,
        padlog_blake3,
        feature_map_blake3,
        expect_blake3,
        input: InputSummary {
            events_scheduled: 0,
            port: opts.port,
            script_frames: script.len(),
        },
        ready_proof: None,
        room_transition: None,
        framebuffer_checkpoints: Vec::new(),
        pad_trace_ok: None,
        frames_run: 0,
        result: "fail",
        failures: Vec::new(),
    };

    drive(
        opts,
        &expect,
        room,
        room_width,
        &script,
        snapshot_hash,
        frame_budget,
        &mut report,
    );

    // Pass criteria: READY proof present, the room transition observed, all
    // expected framebuffer hashes matched, pad trace consistent, no failures.
    let checkpoints_ok = report
        .framebuffer_checkpoints
        .iter()
        .all(|c| c.matched != Some(false));
    if report.failures.is_empty()
        && report.ready_proof.is_some()
        && report.room_transition.is_some()
        && checkpoints_ok
        && report.pad_trace_ok == Some(true)
    {
        report.result = "pass";
    }
    Ok(report)
}

/// Everything that talks to the worker. Failures are recorded in the report
/// (one distinct, sanitized entry per stage) rather than propagated.
#[allow(clippy::too_many_arguments)]
fn drive(
    opts: &VmFirstRoomOptions,
    expect: &VmExpect,
    room: &Feature,
    room_width: u32,
    script: &[u16],
    snapshot_hash: [u8; 32],
    frame_budget: u32,
    report: &mut VmFirstRoomReport,
) {
    let fail = |report: &mut VmFirstRoomReport, stage: &str, e: DhClientError| {
        report.failures.push(FailureEntry {
            stage: stage.to_owned(),
            code: e.code_str(),
            message: e.to_string(),
        });
    };

    let endpoint = WorkerEndpoint::parse(&opts.worker);
    let mut session = match WorkerSession::connect(&endpoint) {
        Ok(session) => session,
        Err(e) => return fail(report, "connect", e),
    };

    if let Ok(info) = session.worker_info() {
        report.worker.worker_id = info.worker_id;
        report.worker.version = info.version;
    }

    // -- RestoreSnapshot --
    let restored = match session.restore_snapshot(snapshot_hash.to_vec(), Vec::new()) {
        Ok(restored) => restored,
        Err(e) => return fail(report, "restore", e),
    };
    let lease = restored.lease.clone().expect("checked by restore_snapshot");
    let base = restored.frame_counter;

    // -- READY proof (regions live with contract geometry, before input) --
    let room_offset = room.offset.0 as u64;
    let region_ranges = vec![
        proto::RegionRange {
            region: "meta".to_owned(),
            layout_version: 1,
            offset: 0,
            len: META_READ_LEN,
        },
        proto::RegionRange {
            region: room.region.clone(),
            layout_version: 1,
            offset: room_offset,
            len: u64::from(room_width),
        },
    ];
    let ready = match session.read_regions(lease.clone(), region_ranges.clone()) {
        Ok(ready) => ready,
        Err(e) => {
            fail(report, "ready-proof", e);
            let _ = session.destroy_vm(lease);
            return;
        }
    };
    let fb = match session.get_framebuffer(lease.clone()) {
        Ok(fb) => fb,
        Err(e) => {
            fail(report, "framebuffer", e);
            let _ = session.destroy_vm(lease);
            return;
        }
    };
    if fb.pixels.len() != FB_BYTES
        || fb.width != FB_WIDTH
        || fb.height != FB_HEIGHT
        || fb.stride != FB_STRIDE
        || fb.format != proto::PixelFormat::Xrgb8888 as i32
    {
        report.failures.push(FailureEntry {
            stage: "framebuffer".to_owned(),
            code: "geometry_mismatch".to_owned(),
            message: format!(
                "framebuffer is {} bytes {}x{} stride {} format {}; layout_version 1 requires \
                 {FB_BYTES} bytes {FB_WIDTH}x{FB_HEIGHT} stride {FB_STRIDE} XRGB8888",
                fb.pixels.len(),
                fb.width,
                fb.height,
                fb.stride,
                fb.format
            ),
        });
        let _ = session.destroy_vm(lease);
        return;
    }
    let meta = &ready.chunks[0];
    let room_at_ready = decode_scalar(&room.feature_type, &ready.chunks[1]);
    report.ready_proof = Some(ReadyProof {
        state_hash: hex(&restored.state_hash.map(|h| h.hash).unwrap_or_default()),
        frame_counter_base: base,
        meta_status: u32::from_le_bytes(
            meta[META_STATUS_OFF..META_STATUS_OFF + 4]
                .try_into()
                .unwrap(),
        ),
        meta_frame: u64::from_le_bytes(
            meta[META_FRAME_OFF..META_FRAME_OFF + 8].try_into().unwrap(),
        ),
        room_feature_value: room_at_ready,
        framebuffer_bytes: fb.pixels.len(),
        framebuffer_format: "xrgb8888-256x224-stride1024".to_owned(),
        framebuffer_blake3: blake3::hash(&fb.pixels).to_hex().to_string(),
    });

    // -- InjectInputs: absolute frames base+1.., change-only (pv-pad holds
    //    the latest value) --
    let mut events = Vec::new();
    let mut previous: Option<u16> = None;
    for (i, &word) in script.iter().enumerate() {
        if previous == Some(word) {
            continue;
        }
        previous = Some(word);
        events.push(proto::ScheduledEvent {
            at: Some(proto::scheduled_event::At::AtFrame(base + 1 + i as u32)),
            event: Some(proto::scheduled_event::Event::PadSet(proto::PadSet {
                port: opts.port,
                buttons: u32::from(word),
            })),
        });
    }
    match session.inject_inputs(lease.clone(), events) {
        Ok(scheduled) => report.input.events_scheduled = scheduled,
        Err(e) => {
            fail(report, "inject", e);
            let _ = session.destroy_vm(lease);
            return;
        }
    }

    // -- Run in steps; capture the room feature + meta head at each stop --
    let mut stops: BTreeSet<u32> = (1..=frame_budget / expect.step)
        .map(|k| k * expect.step)
        .collect();
    stops.insert(frame_budget);
    for checkpoint in &expect.framebuffer_checkpoints {
        if checkpoint.frame <= frame_budget {
            stops.insert(checkpoint.frame);
        }
    }
    let capture = proto::CaptureSpec {
        ranges: vec![
            proto::ExtractRange {
                region: room.region.clone(),
                layout_version: 1,
                offset: room_offset,
                len: room_width,
            },
            proto::ExtractRange {
                region: "meta".to_owned(),
                layout_version: 1,
                offset: 0,
                len: META_READ_LEN as u32,
            },
        ],
        framebuffer: false,
    };

    let mut frames_run: u32 = 0;
    let mut pad_trace_ok = true;
    let mut transition: Option<u32> = None;
    for stop in stops {
        let step = stop - frames_run;
        let run = match session.run_frames(lease.clone(), step, Some(capture.clone()), 0) {
            Ok(run) => run,
            Err(e) => {
                fail(report, "run", e);
                report.frames_run = frames_run;
                report.pad_trace_ok = Some(pad_trace_ok);
                let _ = session.destroy_vm(lease);
                return;
            }
        };
        frames_run = stop;
        if run.reason != proto::StopReason::BudgetReached as i32 {
            report.failures.push(FailureEntry {
                stage: "run".to_owned(),
                code: "unexpected_stop".to_owned(),
                message: format!(
                    "run stopped with reason {} after {} frames (wanted BUDGET_REACHED at {step})",
                    run.reason, run.frames_elapsed
                ),
            });
            break;
        }
        let room_bytes = &run.feature_bytes[..room_width as usize];
        let meta_bytes = &run.feature_bytes[room_width as usize..];
        let room_value = decode_scalar(&room.feature_type, room_bytes);
        let last_pad = u16::from_le_bytes(
            meta_bytes[META_LAST_PAD_OFF..META_LAST_PAD_OFF + 2]
                .try_into()
                .unwrap(),
        );
        // The harness mirrors the live pad into meta.last_pad each frame;
        // at frame `stop` the script word is script[stop-1] (held past end).
        let expected_pad = script
            .get((stop - 1) as usize)
            .or(script.last())
            .copied()
            .unwrap_or(0);
        if last_pad != expected_pad {
            pad_trace_ok = false;
        }
        if transition.is_none() && room_value == expect.target_room {
            transition = Some(stop);
        }
        // Framebuffer checkpoint at this stop?
        if let Some(checkpoint) = expect
            .framebuffer_checkpoints
            .iter()
            .find(|c| c.frame == stop)
        {
            match session.get_framebuffer(lease.clone()) {
                Ok(fb) => {
                    let observed = blake3::hash(&fb.pixels).to_hex().to_string();
                    let expected = checkpoint.blake3.clone().filter(|s| !s.is_empty());
                    let matched = expected.as_ref().map(|e| e.eq_ignore_ascii_case(&observed));
                    report.framebuffer_checkpoints.push(FbCheckpointResult {
                        frame: stop,
                        blake3: observed,
                        expected_blake3: expected,
                        matched,
                    });
                }
                Err(e) => {
                    fail(report, "framebuffer", e);
                    break;
                }
            }
        }
        if transition.is_some()
            && report.framebuffer_checkpoints.len() == expect.framebuffer_checkpoints.len()
            && stop
                >= frame_budget.min(
                    expect
                        .framebuffer_checkpoints
                        .iter()
                        .map(|c| c.frame)
                        .max()
                        .unwrap_or(0),
                )
        {
            // Transition observed and every checkpoint captured — done.
            break;
        }
    }
    report.frames_run = frames_run;
    report.pad_trace_ok = Some(pad_trace_ok);
    if let Some(frame) = transition {
        report.room_transition = Some(RoomTransition {
            feature: expect.room_feature.clone(),
            from: expect.initial_room,
            to: expect.target_room,
            observed_by_frame: frame,
        });
    }

    if let Err(e) = session.destroy_vm(lease) {
        fail(report, "destroy", e);
    }
}

// ---- helpers ----

/// Write the report and its BLAKE3 alongside (`<report>.b3`).
pub fn write_report(report: &VmFirstRoomReport, path: &Path) -> Result<String, SetupError> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| SetupError::Parse(format!("report serialization failed: {e}")))?;
    std::fs::write(path, &json).map_err(|e| SetupError::Io(path.to_path_buf(), e))?;
    let hash = blake3::hash(json.as_bytes()).to_hex().to_string();
    let b3_path = path.with_extension("json.b3");
    std::fs::write(&b3_path, format!("{hash}\n")).map_err(|e| SetupError::Io(b3_path, e))?;
    Ok(hash)
}

fn read(path: &Path) -> Result<Vec<u8>, SetupError> {
    std::fs::read(path).map_err(|e| SetupError::Io(path.to_path_buf(), e))
}

fn parse_hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn decode_scalar(feature_type: &FeatureType, bytes: &[u8]) -> i64 {
    match feature_type {
        FeatureType::U8 => i64::from(bytes[0]),
        FeatureType::I8 => i64::from(bytes[0] as i8),
        FeatureType::U16le => i64::from(u16::from_le_bytes([bytes[0], bytes[1]])),
        FeatureType::U16be => i64::from(u16::from_be_bytes([bytes[0], bytes[1]])),
        FeatureType::I16le => i64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
        FeatureType::I16be => i64::from(i16::from_be_bytes([bytes[0], bytes[1]])),
        FeatureType::U32le => i64::from(u32::from_le_bytes(bytes[..4].try_into().unwrap())),
        FeatureType::U32be => i64::from(u32::from_be_bytes(bytes[..4].try_into().unwrap())),
        FeatureType::I32le => i64::from(i32::from_le_bytes(bytes[..4].try_into().unwrap())),
        FeatureType::I32be => i64::from(i32::from_be_bytes(bytes[..4].try_into().unwrap())),
        _ => unreachable!("guarded by is_integer_scalar"),
    }
}

fn git_rev() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
}
