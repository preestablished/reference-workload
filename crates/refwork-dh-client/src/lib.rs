//! Blocking client for the determinism-hypervisor worker gRPC API
//! (`determinism.hypervisor.v1.HypervisorWorker`).
//!
//! This is the repo's seam to the in-VM execution substrate: the
//! `vm-first-room` gate and the M5 in-VM determinism suite drive the worker
//! exclusively through this crate. The proto contract comes from the sibling
//! determinism-hypervisor checkout via `dh-proto` (path dep — the same
//! pattern rom-operator-bridge uses), so the wire types cannot drift from
//! the deployed worker's schema.
//!
//! The rest of the workspace is synchronous; this crate owns a private tokio
//! runtime and exposes blocking calls. Worker RPC errors are surfaced with
//! their gRPC code and message verbatim — the worker names offenders
//! precisely (layout versions, byte counts, snapshot refs) and its messages
//! are clean-room-safe by contract.

// tonic codegen (via dh-proto) precludes forbid(unsafe_code) transitively;
// this crate's own code carries no unsafe.
#![deny(unsafe_code)]

use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tower::service_fn;

pub use dh_proto::v1 as proto;

#[cfg(feature = "mock")]
pub mod mock;

use proto::hypervisor_worker_client::HypervisorWorkerClient;

const RPC_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Where the worker listens. A value starting with `http://` or `https://`
/// is a TCP endpoint; anything else is a UDS socket path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEndpoint {
    Uds(PathBuf),
    Http(String),
}

impl WorkerEndpoint {
    pub fn parse(value: &str) -> Self {
        if value.starts_with("http://") || value.starts_with("https://") {
            Self::Http(value.to_owned())
        } else {
            Self::Uds(PathBuf::from(value))
        }
    }
}

impl fmt::Display for WorkerEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uds(path) => write!(f, "{}", path.display()),
            Self::Http(uri) => write!(f, "{uri}"),
        }
    }
}

/// A worker RPC failure, preserving the gRPC code and the worker's message
/// verbatim (the worker's error text is clean-room-safe by contract).
#[derive(Debug)]
pub enum DhClientError {
    Connect {
        endpoint: String,
        message: String,
    },
    Rpc {
        code: tonic::Code,
        message: String,
    },
    /// The worker's reply was missing a field the contract requires.
    MalformedResponse {
        rpc: &'static str,
        message: String,
    },
}

impl DhClientError {
    /// Machine-readable code string for reports
    /// (e.g. `failed_precondition`, `not_found`, `connect`).
    pub fn code_str(&self) -> String {
        match self {
            Self::Connect { .. } => "connect".to_owned(),
            Self::Rpc { code, .. } => format!("{code:?}")
                .chars()
                .flat_map(|c| {
                    if c.is_uppercase() {
                        vec!['_', c.to_ascii_lowercase()]
                    } else {
                        vec![c]
                    }
                })
                .collect::<String>()
                .trim_start_matches('_')
                .to_owned(),
            Self::MalformedResponse { .. } => "malformed_response".to_owned(),
        }
    }
}

impl fmt::Display for DhClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect { endpoint, message } => {
                write!(f, "cannot connect to worker at {endpoint}: {message}")
            }
            Self::Rpc { code, message } => write!(f, "worker rpc failed ({code:?}): {message}"),
            Self::MalformedResponse { rpc, message } => {
                write!(f, "malformed {rpc} response: {message}")
            }
        }
    }
}

impl std::error::Error for DhClientError {}

impl From<tonic::Status> for DhClientError {
    fn from(status: tonic::Status) -> Self {
        Self::Rpc {
            code: status.code(),
            message: status.message().to_owned(),
        }
    }
}

pub type Result<T> = std::result::Result<T, DhClientError>;

/// Blocking session with one worker. Owns a private tokio runtime.
pub struct WorkerSession {
    rt: tokio::runtime::Runtime,
    client: HypervisorWorkerClient<Channel>,
}

impl WorkerSession {
    pub fn connect(endpoint: &WorkerEndpoint) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| DhClientError::Connect {
                endpoint: endpoint.to_string(),
                message: format!("tokio runtime: {e}"),
            })?;
        let connect_err = |e: tonic::transport::Error| DhClientError::Connect {
            endpoint: endpoint.to_string(),
            message: e.to_string(),
        };
        let channel = match endpoint {
            WorkerEndpoint::Uds(path) => {
                let uds_path = path.clone();
                rt.block_on(async {
                    Endpoint::try_from("http://[::]:0")
                        .map_err(connect_err)?
                        .connect_timeout(CONNECT_TIMEOUT)
                        .timeout(RPC_TIMEOUT)
                        .connect_with_connector(service_fn(move |_uri: tonic::transport::Uri| {
                            let path = uds_path.clone();
                            async move {
                                let stream = UnixStream::connect(path).await?;
                                Ok::<_, std::io::Error>(TokioIo::new(stream))
                            }
                        }))
                        .await
                        .map_err(connect_err)
                })?
            }
            WorkerEndpoint::Http(uri) => rt.block_on(async {
                Endpoint::from_shared(uri.clone())
                    .map_err(connect_err)?
                    .connect_timeout(CONNECT_TIMEOUT)
                    .timeout(RPC_TIMEOUT)
                    .connect()
                    .await
                    .map_err(connect_err)
            })?,
        };
        Ok(Self {
            rt,
            client: HypervisorWorkerClient::new(channel),
        })
    }

    pub fn worker_info(&mut self) -> Result<proto::GetWorkerInfoResponse> {
        let response = self
            .rt
            .block_on(self.client.get_worker_info(proto::GetWorkerInfoRequest {}))?;
        Ok(response.into_inner())
    }

    /// `snapshot_hash` is the 32-byte BLAKE3 snapshot ref. An empty
    /// `entropy_seed` continues the snapshot's PRNG stream.
    pub fn restore_snapshot(
        &mut self,
        snapshot_hash: Vec<u8>,
        entropy_seed: Vec<u8>,
    ) -> Result<proto::RestoreSnapshotResponse> {
        let response = self
            .rt
            .block_on(self.client.restore_snapshot(proto::RestoreSnapshotRequest {
                snapshot: Some(proto::SnapshotRef {
                    hash: snapshot_hash,
                }),
                entropy_seed,
            }))?
            .into_inner();
        if response.lease.is_none() {
            return Err(DhClientError::MalformedResponse {
                rpc: "RestoreSnapshot",
                message: "missing lease".to_owned(),
            });
        }
        Ok(response)
    }

    pub fn inject_inputs(
        &mut self,
        lease: proto::Lease,
        events: Vec<proto::ScheduledEvent>,
    ) -> Result<u32> {
        let response = self
            .rt
            .block_on(self.client.inject_inputs(proto::InjectInputsRequest {
                lease: Some(lease),
                events,
            }))?;
        Ok(response.into_inner().scheduled)
    }

    /// Run until the frame-boundary exit of the Nth FrameMark, then pause.
    /// `hard_icount_cap = 0` uses the worker default.
    pub fn run_frames(
        &mut self,
        lease: proto::Lease,
        frame_budget: u32,
        capture: Option<proto::CaptureSpec>,
        hard_icount_cap: u64,
    ) -> Result<proto::RunResponse> {
        let response = self.rt.block_on(self.client.run(proto::RunRequest {
            lease: Some(lease),
            until: Some(proto::run_request::Until::FrameBudget(frame_budget)),
            hard_icount_cap,
            capture,
        }))?;
        Ok(response.into_inner())
    }

    pub fn read_regions(
        &mut self,
        lease: proto::Lease,
        region_ranges: Vec<proto::RegionRange>,
    ) -> Result<proto::ReadGuestMemoryResponse> {
        let response = self.rt.block_on(self.client.read_guest_memory(
            proto::ReadGuestMemoryRequest {
                lease: Some(lease),
                ranges: Vec::new(),
                region_ranges,
            },
        ))?;
        Ok(response.into_inner())
    }

    pub fn get_framebuffer(
        &mut self,
        lease: proto::Lease,
    ) -> Result<proto::GetFramebufferResponse> {
        let response = self.rt.block_on(
            self.client
                .get_framebuffer(proto::GetFramebufferRequest { lease: Some(lease) }),
        )?;
        Ok(response.into_inner())
    }

    pub fn take_snapshot(
        &mut self,
        lease: proto::Lease,
        capture: Option<proto::CaptureSpec>,
    ) -> Result<proto::TakeSnapshotResponse> {
        let response = self
            .rt
            .block_on(self.client.take_snapshot(proto::TakeSnapshotRequest {
                lease: Some(lease),
                seal_input_log: Some(true),
                capture,
            }))?;
        Ok(response.into_inner())
    }

    pub fn destroy_vm(&mut self, lease: proto::Lease) -> Result<()> {
        self.rt.block_on(
            self.client
                .destroy_vm(proto::DestroyVmRequest { lease: Some(lease) }),
        )?;
        Ok(())
    }
}

/// Decompress a capture-engine `fb_lz4` payload
/// (`lz4_flex::compress_prepend_size` on the worker side).
pub fn decompress_fb_lz4(fb_lz4: &[u8]) -> std::result::Result<Vec<u8>, String> {
    lz4_flex::decompress_size_prepended(fb_lz4).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_parse_distinguishes_uds_and_http() {
        assert_eq!(
            WorkerEndpoint::parse("/run/dh/grpc.sock"),
            WorkerEndpoint::Uds(PathBuf::from("/run/dh/grpc.sock"))
        );
        assert_eq!(
            WorkerEndpoint::parse("http://127.0.0.1:7400"),
            WorkerEndpoint::Http("http://127.0.0.1:7400".to_owned())
        );
    }

    #[test]
    fn error_code_str_is_snake_case() {
        let err = DhClientError::Rpc {
            code: tonic::Code::FailedPrecondition,
            message: "framebuffer region is 4096 bytes; layout_version 1 requires 229376".into(),
        };
        assert_eq!(err.code_str(), "failed_precondition");
        let err = DhClientError::Rpc {
            code: tonic::Code::NotFound,
            message: "unknown snapshot ref".into(),
        };
        assert_eq!(err.code_str(), "not_found");
    }

    #[test]
    fn fb_lz4_roundtrip() {
        let pixels = vec![7u8; 1024];
        let compressed = lz4_flex::compress_prepend_size(&pixels);
        assert_eq!(decompress_fb_lz4(&compressed).unwrap(), pixels);
    }
}
