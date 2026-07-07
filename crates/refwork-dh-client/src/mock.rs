//! In-process mock `HypervisorWorker` serving a synthetic staged fixture.
//!
//! CI has no KVM, no dh-workerd, and no game content, so the `vm-first-room`
//! and in-VM suite tests run against this mock: a deterministic little state
//! machine that honors the same contract surface the real worker enforces —
//! snapshot refs, leases, absolute-frame input scheduling, region layout
//! versions, the D7 framebuffer geometry, and `FailedPrecondition` errors
//! that name their offender. It is NOT a determinism substrate for M5
//! acceptance claims; it exists so the client/report/failure-mode logic is
//! meaningfully tested without the operator image (plan step 04 requirement).
//!
//! Determinism note: given the same fixture, restore ref, and injected
//! inputs, every run evolves identically (state hashes chain over guest
//! state only), so CI can exercise double-run and restore-continuity logic.

#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use tonic::{Request, Response, Status};

use crate::proto;
use proto::hypervisor_worker_server::{HypervisorWorker, HypervisorWorkerServer};

pub const FB_WIDTH: u32 = 256;
pub const FB_HEIGHT: u32 = 224;
pub const FB_STRIDE: u32 = 1024;
pub const FB_BYTES: usize = (FB_STRIDE * FB_HEIGHT) as usize;
pub const META_SIZE: usize = 4096;

// Meta-page offsets mirroring refwork-harness/src/meta.rs.
const META_STATUS_OFF: usize = 0x04;
const META_FRAME_OFF: usize = 0x08;
const META_LAST_PAD_OFF: usize = 0x10;
const META_STATUS_READY: u32 = 1;
const META_STATUS_RUNNING: u32 = 2;

/// Synthetic game fixture parameters.
#[derive(Debug, Clone)]
pub struct MockFixture {
    /// The READY root snapshot ref the mock accepts for cold restores.
    pub ready_snapshot_ref: [u8; 32],
    /// Absolute pv-pad FRAME_COUNTER at the READY boundary.
    pub ready_frame_counter: u32,
    pub wram_size: usize,
    /// Byte offset of the one-byte `room_id` feature in wram.
    pub room_offset: usize,
    pub room_initial: u8,
    pub room_target: u8,
    /// The room transitions after this many frames with a non-zero pad.
    pub transition_after_pads: u32,
    /// Negative-test mode: perturb guest state when restoring a saved
    /// mid-run snapshot (never the READY root), so the restore-continuity
    /// leg diverges from the uninterrupted baseline while double-run stays
    /// clean. Tests only; a suite that cannot fail proves nothing.
    pub corrupt_mid_run_restores: bool,
}

impl Default for MockFixture {
    fn default() -> Self {
        Self {
            ready_snapshot_ref: [0xAB; 32],
            ready_frame_counter: 0,
            wram_size: 131072,
            room_offset: 0x40,
            room_initial: 0,
            room_target: 1,
            transition_after_pads: 8,
            corrupt_mid_run_restores: false,
        }
    }
}

#[derive(Clone)]
struct GuestState {
    frame_counter: u32,
    room: u8,
    nonzero_pads_applied: u32,
    last_pad: u16,
    meta_frame: u64,
    running: bool,
}

impl GuestState {
    fn ready(fixture: &MockFixture) -> Self {
        Self {
            frame_counter: fixture.ready_frame_counter,
            room: fixture.room_initial,
            nonzero_pads_applied: 0,
            last_pad: 0,
            meta_frame: 0,
            running: false,
        }
    }

    fn state_hash(&self, fixture: &MockFixture) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&fixture.ready_snapshot_ref);
        hasher.update(&self.frame_counter.to_le_bytes());
        hasher.update(&[self.room]);
        hasher.update(&self.nonzero_pads_applied.to_le_bytes());
        hasher.update(&self.last_pad.to_le_bytes());
        hasher.update(&self.meta_frame.to_le_bytes());
        hasher.finalize().as_bytes().to_vec()
    }

    fn wram(&self, fixture: &MockFixture) -> Vec<u8> {
        let mut wram = vec![0u8; fixture.wram_size];
        wram[fixture.room_offset] = self.room;
        wram
    }

    fn meta(&self) -> Vec<u8> {
        let mut meta = vec![0u8; META_SIZE];
        meta[0..4].copy_from_slice(&1u32.to_le_bytes()); // META_VERSION
        let status = if self.running {
            META_STATUS_RUNNING
        } else {
            META_STATUS_READY
        };
        meta[META_STATUS_OFF..META_STATUS_OFF + 4].copy_from_slice(&status.to_le_bytes());
        meta[META_FRAME_OFF..META_FRAME_OFF + 8].copy_from_slice(&self.meta_frame.to_le_bytes());
        meta[META_LAST_PAD_OFF..META_LAST_PAD_OFF + 2]
            .copy_from_slice(&self.last_pad.to_le_bytes());
        meta
    }

    fn framebuffer(&self) -> Vec<u8> {
        // Deterministic pattern: a pure function of (room, frame_counter).
        let fill = self.room.wrapping_mul(16) ^ (self.frame_counter as u8);
        vec![fill; FB_BYTES]
    }
}

struct Slot {
    token: Vec<u8>,
    state: GuestState,
    scheduled: BTreeMap<u32, u32>,
}

#[derive(Default)]
struct WorkerState {
    slots: BTreeMap<u64, Slot>,
    snapshots: BTreeMap<Vec<u8>, GuestState>,
    next_slot: u64,
}

/// `(feature_bytes, fb_lz4, fb_info)` from a capture-engine evaluation.
type CaptureOutput = (Vec<u8>, Vec<u8>, Option<proto::FbInfo>);

/// Mock worker service; wrap in [`HypervisorWorkerServer`] or use
/// [`spawn_uds`].
#[derive(Clone)]
pub struct MockWorker {
    fixture: MockFixture,
    state: Arc<Mutex<WorkerState>>,
}

impl MockWorker {
    pub fn new(fixture: MockFixture) -> Self {
        Self {
            fixture,
            state: Arc::new(Mutex::new(WorkerState::default())),
        }
    }

    fn region_len(&self, region: &str) -> Option<usize> {
        match region {
            "wram" => Some(self.fixture.wram_size),
            "framebuffer" => Some(FB_BYTES),
            "meta" => Some(META_SIZE),
            _ => None,
        }
    }

    fn read_region(
        &self,
        state: &GuestState,
        region: &str,
        layout_version: u32,
        offset: u64,
        len: u64,
    ) -> Result<Vec<u8>, Status> {
        let region_len = self.region_len(region).ok_or_else(|| {
            Status::failed_precondition(format!("region '{region}' not in the manifest"))
        })? as u64;
        if layout_version != 1 {
            return Err(Status::failed_precondition(format!(
                "region '{region}' layout_version {layout_version}: manifest has 1"
            )));
        }
        if offset + len > region_len {
            return Err(Status::invalid_argument(format!(
                "region '{region}' read {offset}+{len} exceeds {region_len} bytes"
            )));
        }
        let bytes = match region {
            "wram" => state.wram(&self.fixture),
            "framebuffer" => state.framebuffer(),
            "meta" => state.meta(),
            _ => unreachable!(),
        };
        Ok(bytes[offset as usize..(offset + len) as usize].to_vec())
    }

    fn capture(
        &self,
        state: &GuestState,
        spec: &proto::CaptureSpec,
    ) -> Result<CaptureOutput, Status> {
        let mut feature_bytes = Vec::new();
        for range in &spec.ranges {
            feature_bytes.extend(self.read_region(
                state,
                &range.region,
                range.layout_version,
                range.offset,
                u64::from(range.len),
            )?);
        }
        if spec.framebuffer {
            let pixels = state.framebuffer();
            let fb_lz4 = lz4_flex::compress_prepend_size(&pixels);
            let fb_info = proto::FbInfo {
                width: FB_WIDTH,
                height: FB_HEIGHT,
                stride: FB_STRIDE,
                format: proto::PixelFormat::Xrgb8888 as i32,
                frame_counter: state.frame_counter,
            };
            Ok((feature_bytes, fb_lz4, Some(fb_info)))
        } else {
            Ok((feature_bytes, Vec::new(), None))
        }
    }

    fn with_slot<T>(
        &self,
        lease: Option<&proto::Lease>,
        f: impl FnOnce(&MockFixture, &mut Slot) -> Result<T, Status>,
    ) -> Result<T, Status> {
        let lease = lease.ok_or_else(|| Status::invalid_argument("missing lease"))?;
        let mut state = self.state.lock().expect("mock state mutex poisoned");
        let slot = state
            .slots
            .get_mut(&lease.slot_id)
            .ok_or_else(|| Status::failed_precondition(format!("slot {} empty", lease.slot_id)))?;
        if slot.token != lease.token {
            return Err(Status::failed_precondition(format!(
                "stale lease for slot {}",
                lease.slot_id
            )));
        }
        f(&self.fixture, slot)
    }
}

#[tonic::async_trait]
impl HypervisorWorker for MockWorker {
    async fn restore_snapshot(
        &self,
        request: Request<proto::RestoreSnapshotRequest>,
    ) -> Result<Response<proto::RestoreSnapshotResponse>, Status> {
        let request = request.into_inner();
        let hash = request.snapshot.map(|s| s.hash).unwrap_or_default();
        let mut state = self.state.lock().expect("mock state mutex poisoned");
        let guest = if hash == self.fixture.ready_snapshot_ref {
            GuestState::ready(&self.fixture)
        } else if let Some(saved) = state.snapshots.get(&hash) {
            let mut restored = saved.clone();
            if self.fixture.corrupt_mid_run_restores {
                restored.nonzero_pads_applied = restored.nonzero_pads_applied.wrapping_add(1);
            }
            restored
        } else {
            return Err(Status::not_found(format!(
                "unknown snapshot ref {}",
                hex(&hash)
            )));
        };
        state.next_slot += 1;
        let slot_id = state.next_slot;
        let token = blake3::hash(&slot_id.to_le_bytes()).as_bytes()[..16].to_vec();
        let response = proto::RestoreSnapshotResponse {
            lease: Some(proto::Lease {
                slot_id,
                token: token.clone(),
            }),
            config: None,
            state_hash: Some(proto::StateHash {
                hash: guest.state_hash(&self.fixture),
            }),
            frame_counter: guest.frame_counter,
        };
        state.slots.insert(
            slot_id,
            Slot {
                token,
                state: guest,
                scheduled: BTreeMap::new(),
            },
        );
        Ok(Response::new(response))
    }

    async fn inject_inputs(
        &self,
        request: Request<proto::InjectInputsRequest>,
    ) -> Result<Response<proto::InjectInputsResponse>, Status> {
        let request = request.into_inner();
        self.with_slot(request.lease.as_ref(), |_fixture, slot| {
            let mut accepted = 0u32;
            for event in &request.events {
                let at_frame = match event.at {
                    Some(proto::scheduled_event::At::AtFrame(frame)) => frame,
                    _ => {
                        return Err(Status::invalid_argument(
                            "mock worker only supports at_frame scheduling",
                        ))
                    }
                };
                if at_frame <= slot.state.frame_counter {
                    return Err(Status::invalid_argument(format!(
                        "at_frame {at_frame} is not in the future (frame_counter {})",
                        slot.state.frame_counter
                    )));
                }
                let pad = match &event.event {
                    Some(proto::scheduled_event::Event::PadSet(pad)) => pad,
                    _ => {
                        return Err(Status::invalid_argument(
                            "mock worker only supports PadSet events",
                        ))
                    }
                };
                slot.scheduled.insert(at_frame, pad.buttons);
                accepted += 1;
            }
            Ok(Response::new(proto::InjectInputsResponse {
                scheduled: accepted,
            }))
        })
    }

    async fn run(
        &self,
        request: Request<proto::RunRequest>,
    ) -> Result<Response<proto::RunResponse>, Status> {
        let request = request.into_inner();
        let capture_spec = request.capture;
        let budget = match request.until {
            Some(proto::run_request::Until::FrameBudget(n)) => n,
            _ => {
                return Err(Status::invalid_argument(
                    "mock worker only supports frame_budget runs",
                ))
            }
        };
        let worker = self.clone();
        self.with_slot(request.lease.as_ref(), move |fixture, slot| {
            for _ in 0..budget {
                slot.state.frame_counter += 1;
                slot.state.meta_frame += 1;
                slot.state.running = true;
                // pv-pad semantics: the most recent scheduled pad at or
                // before this frame is the live pad state; with nothing
                // scheduled, the pad value persists — including across
                // snapshot/restore (DHSNAP PADD carries it).
                let pad = slot
                    .scheduled
                    .range(..=slot.state.frame_counter)
                    .next_back()
                    .map(|(_, buttons)| *buttons)
                    .unwrap_or(u32::from(slot.state.last_pad));
                slot.state.last_pad = pad as u16;
                if pad != 0 {
                    slot.state.nonzero_pads_applied += 1;
                    if slot.state.nonzero_pads_applied >= fixture.transition_after_pads {
                        slot.state.room = fixture.room_target;
                    }
                }
            }
            let (feature_bytes, fb_lz4, fb_info) = match &capture_spec {
                Some(spec) => worker.capture(&slot.state, spec)?,
                None => (Vec::new(), Vec::new(), None),
            };
            Ok(Response::new(proto::RunResponse {
                reason: proto::StopReason::BudgetReached as i32,
                icount: u64::from(slot.state.frame_counter) * 1000,
                vns: 0,
                state_hash: Some(proto::StateHash {
                    hash: slot.state.state_hash(fixture),
                }),
                frames_elapsed: u64::from(budget),
                sdk_event: None,
                feature_bytes,
                fb_lz4,
                fb_info,
            }))
        })
    }

    async fn read_guest_memory(
        &self,
        request: Request<proto::ReadGuestMemoryRequest>,
    ) -> Result<Response<proto::ReadGuestMemoryResponse>, Status> {
        let request = request.into_inner();
        let worker = self.clone();
        self.with_slot(request.lease.as_ref(), move |_fixture, slot| {
            if !request.ranges.is_empty() {
                return Err(Status::invalid_argument(
                    "mock worker only supports region_ranges reads",
                ));
            }
            let mut chunks = Vec::new();
            for range in &request.region_ranges {
                chunks.push(worker.read_region(
                    &slot.state,
                    &range.region,
                    range.layout_version,
                    range.offset,
                    range.len,
                )?);
            }
            Ok(Response::new(proto::ReadGuestMemoryResponse {
                chunks,
                icount: u64::from(slot.state.frame_counter) * 1000,
            }))
        })
    }

    async fn get_framebuffer(
        &self,
        request: Request<proto::GetFramebufferRequest>,
    ) -> Result<Response<proto::GetFramebufferResponse>, Status> {
        let request = request.into_inner();
        self.with_slot(request.lease.as_ref(), |_fixture, slot| {
            Ok(Response::new(proto::GetFramebufferResponse {
                width: FB_WIDTH,
                height: FB_HEIGHT,
                stride: FB_STRIDE,
                format: proto::PixelFormat::Xrgb8888 as i32,
                frame_counter: slot.state.frame_counter,
                icount: u64::from(slot.state.frame_counter) * 1000,
                pixels: slot.state.framebuffer(),
            }))
        })
    }

    async fn take_snapshot(
        &self,
        request: Request<proto::TakeSnapshotRequest>,
    ) -> Result<Response<proto::TakeSnapshotResponse>, Status> {
        let request = request.into_inner();
        let capture_spec = request.capture;
        let worker = self.clone();
        let (guest, response) = self.with_slot(request.lease.as_ref(), move |fixture, slot| {
            let (feature_bytes, fb_lz4, fb_info) = match &capture_spec {
                Some(spec) => worker.capture(&slot.state, spec)?,
                None => (Vec::new(), Vec::new(), None),
            };
            let state_hash = slot.state.state_hash(fixture);
            // Snapshot ref: hash of the state hash — stable and distinct
            // from the state hash itself.
            let snapshot_ref = blake3::hash(&state_hash).as_bytes().to_vec();
            let response = proto::TakeSnapshotResponse {
                snapshot: Some(proto::SnapshotRef { hash: snapshot_ref }),
                input_log_id: vec![0; 32],
                icount: u64::from(slot.state.frame_counter) * 1000,
                vns: 0,
                state_hash: Some(proto::StateHash { hash: state_hash }),
                dirty_pages: 0,
                machine_config_hash: vec![0; 32],
                determinism_class: None,
                feature_bytes,
                fb_lz4,
                fb_info,
                frame_counter: slot.state.frame_counter,
            };
            Ok((slot.state.clone(), response))
        })?;
        let ref_hash = response
            .snapshot
            .as_ref()
            .expect("snapshot ref set above")
            .hash
            .clone();
        self.state
            .lock()
            .expect("mock state mutex poisoned")
            .snapshots
            .insert(ref_hash, guest);
        Ok(Response::new(response))
    }

    async fn destroy_vm(
        &self,
        request: Request<proto::DestroyVmRequest>,
    ) -> Result<Response<proto::DestroyVmResponse>, Status> {
        let request = request.into_inner();
        let lease = request
            .lease
            .ok_or_else(|| Status::invalid_argument("missing lease"))?;
        let mut state = self.state.lock().expect("mock state mutex poisoned");
        match state.slots.remove(&lease.slot_id) {
            Some(slot) if slot.token == lease.token => {
                Ok(Response::new(proto::DestroyVmResponse {}))
            }
            Some(slot) => {
                state.slots.insert(lease.slot_id, slot);
                Err(Status::failed_precondition(format!(
                    "stale lease for slot {}",
                    lease.slot_id
                )))
            }
            None => Err(Status::failed_precondition(format!(
                "slot {} empty",
                lease.slot_id
            ))),
        }
    }

    async fn get_worker_info(
        &self,
        _request: Request<proto::GetWorkerInfoRequest>,
    ) -> Result<Response<proto::GetWorkerInfoResponse>, Status> {
        Ok(Response::new(proto::GetWorkerInfoResponse {
            worker_id: "mock-staged-fixture".to_owned(),
            slots_total: 4,
            slots_free: 4,
            class: None,
            version: "mock-0.1".to_owned(),
            build_profile: "mock".to_owned(),
        }))
    }

    // ---- surface not exercised by the staged fixture ----

    async fn create_vm(
        &self,
        _request: Request<proto::CreateVmRequest>,
    ) -> Result<Response<proto::CreateVmResponse>, Status> {
        Err(Status::unimplemented("mock worker: CreateVm"))
    }

    async fn fork(
        &self,
        _request: Request<proto::ForkRequest>,
    ) -> Result<Response<proto::ForkResponse>, Status> {
        Err(Status::unimplemented("mock worker: Fork"))
    }

    async fn pause(
        &self,
        _request: Request<proto::PauseRequest>,
    ) -> Result<Response<proto::PauseResponse>, Status> {
        Err(Status::unimplemented("mock worker: Pause"))
    }

    async fn quiesce(
        &self,
        _request: Request<proto::QuiesceRequest>,
    ) -> Result<Response<proto::QuiesceResponse>, Status> {
        Err(Status::unimplemented("mock worker: Quiesce"))
    }

    type StreamGuestEventsStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::GuestEvent, Status>>;

    async fn stream_guest_events(
        &self,
        _request: Request<proto::StreamGuestEventsRequest>,
    ) -> Result<Response<Self::StreamGuestEventsStream>, Status> {
        Err(Status::unimplemented("mock worker: StreamGuestEvents"))
    }

    type VerifyReplayStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::VerifyReplayProgress, Status>>;

    async fn verify_replay(
        &self,
        _request: Request<proto::VerifyReplayRequest>,
    ) -> Result<Response<Self::VerifyReplayStream>, Status> {
        Err(Status::unimplemented("mock worker: VerifyReplay"))
    }

    type RunWithFrameCaptureStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::FrameCaptureEvent, Status>>;

    async fn run_with_frame_capture(
        &self,
        _request: Request<proto::RunWithFrameCaptureRequest>,
    ) -> Result<Response<Self::RunWithFrameCaptureStream>, Status> {
        Err(Status::unimplemented("mock worker: RunWithFrameCapture"))
    }

    async fn list_slots(
        &self,
        _request: Request<proto::ListSlotsRequest>,
    ) -> Result<Response<proto::ListSlotsResponse>, Status> {
        Err(Status::unimplemented("mock worker: ListSlots"))
    }

    type WatchSlotsStream =
        tokio_stream::wrappers::ReceiverStream<Result<proto::SlotEvent, Status>>;

    async fn watch_slots(
        &self,
        _request: Request<proto::WatchSlotsRequest>,
    ) -> Result<Response<Self::WatchSlotsStream>, Status> {
        Err(Status::unimplemented("mock worker: WatchSlots"))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Handle to a mock worker serving on a UDS path. Dropping it shuts the
/// server down.
pub struct MockHandle {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MockHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Serve `fixture` on `uds_path` from a background thread. Returns once the
/// socket is accepting connections.
pub fn spawn_uds(fixture: MockFixture, uds_path: &Path) -> std::io::Result<MockHandle> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<std::io::Result<()>>();
    let path = uds_path.to_path_buf();
    let thread = std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ready_tx.send(Err(e));
                return;
            }
        };
        rt.block_on(async move {
            let listener = match tokio::net::UnixListener::bind(&path) {
                Ok(listener) => listener,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            let incoming = tokio_stream::wrappers::UnixListenerStream::new(listener);
            let _ = tonic::transport::Server::builder()
                .add_service(HypervisorWorkerServer::new(MockWorker::new(fixture)))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });
    });
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(MockHandle {
            shutdown: Some(shutdown_tx),
            thread: Some(thread),
        }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(std::io::Error::other("mock worker thread died during bind")),
    }
}
