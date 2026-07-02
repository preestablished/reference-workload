//! `vm-suite` — the M5 in-VM determinism validation suite
//! (Phase 3 exit gate 1, `refwork-d7t.13`/`.14`).
//!
//! This is NOT the in-process `double-run` command (which stays as the fast
//! pre-VM check): every leg here executes inside the hypervisor worker and
//! verifies from the host side only — per-frame hashes come from CaptureSpec
//! `feature_bytes` + `fb_lz4` at FrameMark boundaries, with no guest round
//! trips in the verification path (the same rule the guest-sdk Ms4
//! acceptance enforced).
//!
//! Legs, per the phase plan ("boot -> N frames with a fixed log twice ->
//! per-frame RAM+framebuffer hashes identical; snapshot mid-game -> restore
//! -> continue -> identical to uninterrupted run; 20x zero-flake"):
//!
//! 1. **double-run**: two cold `RestoreSnapshot`s from the same READY ref,
//!    the same input log, hashing `wram` + `framebuffer` + `meta` counters
//!    at every FrameMark; the two hash sequences must be bitwise identical.
//! 2. **restore-continuity**: run to a mid-game frame, `TakeSnapshot`,
//!    `RestoreSnapshot` the new ref in a fresh slot, continue with the
//!    remaining schedule; the continued run's per-frame hashes must equal
//!    the uninterrupted run's from that frame on.
//!
//! `--nondet-test` (TEST-ONLY, mirroring `double-run`'s flag) perturbs one
//! pad word in the second cold run — a suite that cannot fail proves
//! nothing (`refwork-d7t.14`).
//!
//! The suite is target-agnostic: CI drives it against the staged synthetic
//! fixture (mock worker), the lab run drives the same code against the
//! deployed image — determinism claims that only hold for the fixture are
//! not M5, so the lab leg is what closes the milestone.

use std::path::{Path, PathBuf};

use serde::Serialize;

use refwork_dh_client::{decompress_fb_lz4, proto, DhClientError, WorkerEndpoint, WorkerSession};
use refwork_script::parse as parse_padlog;

const META_READ_LEN: u32 = 0x20;

pub struct VmSuiteOptions {
    /// Worker endpoint: UDS socket path, or `http://host:port`.
    pub worker: String,
    /// 64-hex BLAKE3 ref of the READY root snapshot.
    pub snapshot_ref: String,
    /// Input log (padlog). Synthetic in CI; operator-supplied at lab time.
    pub script: PathBuf,
    /// Region to hash as "RAM" (default `wram`).
    pub ram_region: String,
    /// Size of the RAM region in bytes.
    pub ram_size: u32,
    /// Frames per run (default: script length).
    pub frames: Option<u32>,
    /// Mid-game snapshot frame (default: frames / 2).
    pub snapshot_at: Option<u32>,
    /// Consecutive suite iterations (the 20x zero-flake stamp runs 20).
    pub iterations: u32,
    pub report: PathBuf,
    /// TEST-ONLY: perturb one pad word in the second cold run.
    pub nondet_test: bool,
    pub port: u32,
}

// ---- report ----

#[derive(Debug, Serialize)]
pub struct VmSuiteReport {
    pub schema_version: u32,
    pub command: &'static str,
    pub reference_workload_rev: String,
    pub worker: WorkerIdentity,
    pub snapshot_ref: String,
    pub padlog_blake3: String,
    pub ram_region: String,
    pub frames: u32,
    pub snapshot_at: u32,
    pub nondet_test: bool,
    pub iterations: Vec<IterationResult>,
    /// `pass` iff every iteration's both legs held (zero flakes).
    pub result: &'static str,
    pub failures: Vec<FailureEntry>,
}

#[derive(Debug, Default, Serialize)]
pub struct WorkerIdentity {
    pub endpoint: String,
    pub worker_id: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct IterationResult {
    pub iteration: u32,
    pub double_run: LegResult,
    pub restore_continuity: LegResult,
    /// BLAKE3 over the iteration's per-frame hash sequence — lets the
    /// evidence note pin the whole trajectory with one value.
    pub trajectory_blake3: String,
}

#[derive(Debug, Serialize)]
pub struct LegResult {
    pub ok: bool,
    pub first_divergent_frame: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct FailureEntry {
    /// `connect`, `restore`, `inject`, `run`, `snapshot`, `destroy`.
    pub stage: String,
    pub iteration: u32,
    pub code: String,
    /// Worker message verbatim (clean-room-safe by contract).
    pub message: String,
}

impl VmSuiteReport {
    pub fn passed(&self) -> bool {
        self.result == "pass"
    }
}

pub use crate::vm_first_room::SetupError;

// ---- driver ----

struct SuiteError {
    stage: &'static str,
    inner: DhClientError,
}

fn suite_err(stage: &'static str) -> impl Fn(DhClientError) -> SuiteError {
    move |inner| SuiteError { stage, inner }
}

pub fn vm_suite(opts: &VmSuiteOptions) -> Result<VmSuiteReport, SetupError> {
    let script_bytes =
        std::fs::read(&opts.script).map_err(|e| SetupError::Io(opts.script.clone(), e))?;
    let padlog_blake3 = blake3::hash(&script_bytes).to_hex().to_string();
    let script_text = String::from_utf8(script_bytes)
        .map_err(|e| SetupError::Parse(format!("padlog is not utf-8: {e}")))?;
    let script = parse_padlog(&script_text)
        .map_err(|e| SetupError::Parse(format!("padlog parse error: {e}")))?
        .frames;

    let frames = opts.frames.unwrap_or(script.len() as u32);
    if frames < 2 {
        return Err(SetupError::Parse(
            "vm-suite needs at least 2 frames".to_owned(),
        ));
    }
    let snapshot_at = opts.snapshot_at.unwrap_or(frames / 2);
    if snapshot_at == 0 || snapshot_at >= frames {
        return Err(SetupError::Parse(format!(
            "--snapshot-at must be in 1..{frames}"
        )));
    }
    if opts.iterations == 0 {
        return Err(SetupError::Parse(
            "--iterations must be positive".to_owned(),
        ));
    }
    let snapshot_hash = parse_hex32(&opts.snapshot_ref)
        .ok_or_else(|| SetupError::Parse("snapshot ref must be 64 hex chars".to_owned()))?;

    let mut report = VmSuiteReport {
        schema_version: 1,
        command: "vm-suite",
        reference_workload_rev: git_rev(),
        worker: WorkerIdentity {
            endpoint: opts.worker.clone(),
            ..Default::default()
        },
        snapshot_ref: opts.snapshot_ref.clone(),
        padlog_blake3,
        ram_region: opts.ram_region.clone(),
        frames,
        snapshot_at,
        nondet_test: opts.nondet_test,
        iterations: Vec::new(),
        result: "fail",
        failures: Vec::new(),
    };

    let endpoint = WorkerEndpoint::parse(&opts.worker);
    let mut session = match WorkerSession::connect(&endpoint) {
        Ok(session) => session,
        Err(e) => {
            report.failures.push(FailureEntry {
                stage: "connect".to_owned(),
                iteration: 0,
                code: e.code_str(),
                message: e.to_string(),
            });
            return Ok(report);
        }
    };
    if let Ok(info) = session.worker_info() {
        report.worker.worker_id = info.worker_id;
        report.worker.version = info.version;
    }

    // TEST-ONLY perturbation: flip the low pad bit at the mid frame of the
    // second cold run (script index N-1 is frame N).
    let mut perturbed = script.clone();
    if opts.nondet_test {
        let index = (snapshot_at as usize - 1).min(perturbed.len() - 1);
        perturbed[index] ^= 0x0001;
    }

    let mut zero_flake = true;
    for iteration in 1..=opts.iterations {
        let outcome = run_iteration(
            &mut session,
            snapshot_hash,
            &script,
            if opts.nondet_test {
                &perturbed
            } else {
                &script
            },
            frames,
            snapshot_at,
            opts,
        );
        match outcome {
            Ok(result) => {
                zero_flake &= result.double_run.ok && result.restore_continuity.ok;
                report.iterations.push(IterationResult {
                    iteration,
                    ..result
                });
            }
            Err(e) => {
                report.failures.push(FailureEntry {
                    stage: e.stage.to_owned(),
                    iteration,
                    code: e.inner.code_str(),
                    message: e.inner.to_string(),
                });
                return Ok(report);
            }
        }
    }
    if zero_flake && report.failures.is_empty() {
        report.result = "pass";
    }
    Ok(report)
}

/// One suite iteration: a reference run, a second cold run (double-run
/// leg), and a snapshot/restore continuation (restore-continuity leg).
fn run_iteration(
    session: &mut WorkerSession,
    snapshot_hash: [u8; 32],
    script: &[u16],
    second_script: &[u16],
    frames: u32,
    snapshot_at: u32,
    opts: &VmSuiteOptions,
) -> Result<IterationResult, SuiteError> {
    // Leg 0: uninterrupted reference run, keeping the mid-game snapshot.
    let (reference, mid_ref) = cold_run(
        session,
        snapshot_hash,
        script,
        frames,
        Some(snapshot_at),
        opts,
    )?;

    // Leg 1 (double-run): a second cold run; compare every frame hash.
    let (second, _) = cold_run(session, snapshot_hash, second_script, frames, None, opts)?;
    let double_run = compare(&reference, &second, 1);

    // Leg 2 (restore-continuity): restore the mid-game snapshot and run the
    // remaining frames; compare against the reference from snapshot_at+1 on.
    let mid_ref = mid_ref.expect("cold_run returns the requested snapshot");
    let continued = continue_run(session, &mid_ref, script, frames, snapshot_at, opts)?;
    let restore_continuity = compare(
        &reference[snapshot_at as usize..],
        &continued,
        snapshot_at + 1,
    );

    let mut trajectory = blake3::Hasher::new();
    for hash in &reference {
        trajectory.update(hash);
    }
    Ok(IterationResult {
        iteration: 0, // caller overwrites
        double_run,
        restore_continuity,
        trajectory_blake3: trajectory.finalize().to_hex().to_string(),
    })
}

/// Per-frame hash trajectory plus the mid-game snapshot ref when requested.
type ColdRunOutput = (Vec<[u8; 32]>, Option<Vec<u8>>);

/// Restore the READY root, schedule `script`, run `frames` frames one
/// FrameMark at a time, hashing state at each mark. Optionally takes a
/// mid-game snapshot after `snapshot_at` frames and returns its ref.
fn cold_run(
    session: &mut WorkerSession,
    snapshot_hash: [u8; 32],
    script: &[u16],
    frames: u32,
    snapshot_at: Option<u32>,
    opts: &VmSuiteOptions,
) -> Result<ColdRunOutput, SuiteError> {
    let restored = session
        .restore_snapshot(snapshot_hash.to_vec(), Vec::new())
        .map_err(suite_err("restore"))?;
    let lease = restored.lease.clone().expect("checked by restore_snapshot");
    let base = restored.frame_counter;

    inject(session, lease.clone(), script, base, opts.port).map_err(suite_err("inject"))?;

    let mut hashes = Vec::with_capacity(frames as usize);
    let mut mid = None;
    for frame in 1..=frames {
        let run = session
            .run_frames(lease.clone(), 1, Some(capture_spec(opts)), 0)
            .map_err(suite_err("run"))?;
        hashes.push(frame_hash(&run.feature_bytes, &run.fb_lz4));
        if snapshot_at == Some(frame) {
            let snapshot = session
                .take_snapshot(lease.clone(), None)
                .map_err(suite_err("snapshot"))?;
            mid = snapshot.snapshot.map(|s| s.hash);
        }
    }
    session.destroy_vm(lease).map_err(suite_err("destroy"))?;
    Ok((hashes, mid))
}

/// Restore a mid-game snapshot and run the remaining frames. The input
/// schedule re-injects the tail of the script at absolute frames — the
/// snapshot's own sealed log covers the past, not the future.
fn continue_run(
    session: &mut WorkerSession,
    mid_ref: &[u8],
    script: &[u16],
    frames: u32,
    snapshot_at: u32,
    opts: &VmSuiteOptions,
) -> Result<Vec<[u8; 32]>, SuiteError> {
    let restored = session
        .restore_snapshot(mid_ref.to_vec(), Vec::new())
        .map_err(suite_err("restore"))?;
    let lease = restored.lease.clone().expect("checked by restore_snapshot");
    let base = restored.frame_counter;

    // Remaining schedule: script words for frames snapshot_at+1.. land at
    // absolute frames base+1.. (base is the restored FRAME_COUNTER, which
    // equals the reference run's counter at snapshot_at).
    let tail: Vec<u16> = script.iter().copied().skip(snapshot_at as usize).collect();
    // pv-pad holds the last value: seed the pad state at the boundary with
    // the word live at snapshot_at, then apply the tail. The word live at
    // the boundary persists in the restored guest, so only inject changes
    // relative to it.
    let boundary_word = script
        .get(snapshot_at as usize - 1)
        .or(script.last())
        .copied()
        .unwrap_or(0);
    inject_with_boundary(
        session,
        lease.clone(),
        &tail,
        base,
        boundary_word,
        opts.port,
    )
    .map_err(suite_err("inject"))?;

    let mut hashes = Vec::with_capacity((frames - snapshot_at) as usize);
    for _ in snapshot_at + 1..=frames {
        let run = session
            .run_frames(lease.clone(), 1, Some(capture_spec(opts)), 0)
            .map_err(suite_err("run"))?;
        hashes.push(frame_hash(&run.feature_bytes, &run.fb_lz4));
    }
    session.destroy_vm(lease).map_err(suite_err("destroy"))?;
    Ok(hashes)
}

fn inject(
    session: &mut WorkerSession,
    lease: proto::Lease,
    script: &[u16],
    base: u32,
    port: u32,
) -> Result<(), DhClientError> {
    inject_with_boundary(session, lease, script, base, 0, port)
}

/// Change-only injection: pv-pad holds the most recent value, so only emit
/// events where the word differs from the previous one (`boundary_word`
/// before the first frame).
fn inject_with_boundary(
    session: &mut WorkerSession,
    lease: proto::Lease,
    script: &[u16],
    base: u32,
    boundary_word: u16,
    port: u32,
) -> Result<(), DhClientError> {
    let mut events = Vec::new();
    let mut previous = boundary_word;
    for (i, &word) in script.iter().enumerate() {
        if word == previous {
            continue;
        }
        previous = word;
        events.push(proto::ScheduledEvent {
            at: Some(proto::scheduled_event::At::AtFrame(base + 1 + i as u32)),
            event: Some(proto::scheduled_event::Event::PadSet(proto::PadSet {
                port,
                buttons: u32::from(word),
            })),
        });
    }
    if events.is_empty() {
        return Ok(());
    }
    session.inject_inputs(lease, events).map(|_| ())
}

fn capture_spec(opts: &VmSuiteOptions) -> proto::CaptureSpec {
    proto::CaptureSpec {
        ranges: vec![
            proto::ExtractRange {
                region: opts.ram_region.clone(),
                layout_version: 1,
                offset: 0,
                len: opts.ram_size,
            },
            proto::ExtractRange {
                region: "meta".to_owned(),
                layout_version: 1,
                offset: 0,
                len: META_READ_LEN,
            },
        ],
        framebuffer: true,
    }
}

/// Per-frame hash: BLAKE3 over RAM bytes ++ meta head ++ decompressed
/// framebuffer pixels (pixels, not fb_lz4, so the hash is independent of
/// the compressor).
fn frame_hash(feature_bytes: &[u8], fb_lz4: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(feature_bytes);
    if !fb_lz4.is_empty() {
        if let Ok(pixels) = decompress_fb_lz4(fb_lz4) {
            hasher.update(&pixels);
        }
    }
    *hasher.finalize().as_bytes()
}

fn compare(reference: &[[u8; 32]], other: &[[u8; 32]], first_frame: u32) -> LegResult {
    if reference.len() != other.len() {
        return LegResult {
            ok: false,
            first_divergent_frame: Some(first_frame + reference.len().min(other.len()) as u32),
        };
    }
    for (i, (a, b)) in reference.iter().zip(other.iter()).enumerate() {
        if a != b {
            return LegResult {
                ok: false,
                first_divergent_frame: Some(first_frame + i as u32),
            };
        }
    }
    LegResult {
        ok: true,
        first_divergent_frame: None,
    }
}

/// Write the report and its BLAKE3 alongside (`<report>.b3`).
pub fn write_suite_report(report: &VmSuiteReport, path: &Path) -> Result<String, SetupError> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| SetupError::Parse(format!("report serialization failed: {e}")))?;
    std::fs::write(path, &json).map_err(|e| SetupError::Io(path.to_path_buf(), e))?;
    let hash = blake3::hash(json.as_bytes()).to_hex().to_string();
    let b3_path = path.with_extension("json.b3");
    std::fs::write(&b3_path, format!("{hash}\n")).map_err(|e| SetupError::Io(b3_path, e))?;
    Ok(hash)
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
