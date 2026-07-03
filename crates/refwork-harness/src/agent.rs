//! GS-5/GS-6 join point: publish the harness regions to the host through
//! the real `detguest-sdk::register_region` path (mlock + prefault in this
//! process, registration with the agent over `/run/detguest/agent.sock`,
//! agent-owned manifest write).
//!
//! The harness always attempts the real path. Outside the VM — unit tests,
//! `refwork-verify play`, any run without a detchannel — the SDK reports
//! [`detguest_sdk::RegionError::AgentUnavailable`] and the harness continues
//! in standalone mode: region publication to the HOST is simply absent, and
//! the fd-3 protocol behaves exactly as before. Under the agent, a
//! registration failure is a real fault (`FaultCode::RegionRegFailed`)
//! surfaced before `Ready` — the boot contract gates READY on live regions
//! (`ready_after = "regions-registered-and-start-sent"`).
//!
//! Handle lifetime: dropping an SDK handle marks its manifest entry DEAD, so
//! successful registration deliberately leaks the handles
//! (`std::mem::forget`) — the regions themselves are process-lifetime
//! mappings once setup succeeds (see `regions.rs`). If setup fails after
//! registration the process exits immediately, and the agent observes the
//! workload's death; no unregister is attempted on that path.

use crate::regions::HarnessRegions;
use detguest_sdk::RegionFlags;

/// Outcome of attempting host publication.
#[derive(Debug, PartialEq, Eq)]
pub enum AgentPublication {
    /// All required regions are registered with the agent.
    Registered { count: usize },
    /// No agent (no detchannel): standalone mode, nothing published.
    Standalone,
}

/// A hard registration failure under the agent (not `AgentUnavailable`).
#[derive(Debug)]
pub struct AgentPublishError {
    pub region: &'static str,
    pub detail: String,
}

impl std::fmt::Display for AgentPublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "agent registration failed for region `{}`: {}",
            self.region, self.detail
        )
    }
}

impl std::error::Error for AgentPublishError {}

/// Initialize the guest SDK. Harmless outside the VM (no detchannel ⇒ the
/// SDK stays uninitialized and later registration reports standalone).
pub fn init_sdk() {
    let _ = detguest_sdk::init();
}

/// Crate-private seam over `detguest_sdk::register_region` so the hard-fault
/// path is testable: the production impl is the SDK call, verbatim; tests
/// substitute a registrar that fails with a non-`AgentUnavailable` error.
pub(crate) trait RegionRegistrar {
    type Handle;

    /// # Safety
    ///
    /// Same contract as [`detguest_sdk::register_region`]: `ptr`/`len` name a
    /// page-aligned mapping that stays valid and pinned for the handle's
    /// lifetime.
    unsafe fn register(
        &self,
        name: &'static str,
        layout_version: u32,
        ptr: *const u8,
        len: usize,
        flags: RegionFlags,
    ) -> Result<Self::Handle, detguest_sdk::RegionError>;
}

struct SdkRegistrar;

impl RegionRegistrar for SdkRegistrar {
    type Handle = detguest_sdk::RegionHandle;

    unsafe fn register(
        &self,
        name: &'static str,
        layout_version: u32,
        ptr: *const u8,
        len: usize,
        flags: RegionFlags,
    ) -> Result<Self::Handle, detguest_sdk::RegionError> {
        // SAFETY: forwarded contract; upheld by the caller of `register`.
        unsafe { detguest_sdk::register_region(name, layout_version, ptr, len, flags) }
    }
}

/// Register `wram`, `framebuffer`, and `meta` with the agent under the D7
/// contract (`layout_version 1` for all three; the framebuffer flag drives
/// the hypervisor's geometry derivation).
///
/// Call after the regions are mapped and `meta` is initialized, before the
/// harness reports `Ready` — READY must not precede live regions.
pub fn publish_regions(regions: &HarnessRegions) -> Result<AgentPublication, AgentPublishError> {
    publish_regions_with(&SdkRegistrar, regions)
}

pub(crate) fn publish_regions_with<R: RegionRegistrar>(
    registrar: &R,
    regions: &HarnessRegions,
) -> Result<AgentPublication, AgentPublishError> {
    let entries: [(&'static str, u64, usize, RegionFlags); 3] = [
        (
            "wram",
            regions.wram().gva(),
            regions.wram().len(),
            RegionFlags::empty(),
        ),
        (
            "framebuffer",
            regions.framebuffer().gva(),
            regions.framebuffer().len(),
            RegionFlags::FRAMEBUFFER,
        ),
        (
            "meta",
            regions.meta().gva(),
            regions.meta().len(),
            RegionFlags::empty(),
        ),
    ];

    let mut handles = Vec::with_capacity(entries.len());
    for (name, gva, len, flags) in entries {
        // SAFETY: each pointer/len pair names a page-aligned mapping owned
        // by `HarnessRegions` (mmap MAP_LOCKED|MAP_POPULATE). On the success
        // path the mapping is kept for process lifetime (`ActiveEmuRegions`
        // never unmaps); on failure the process exits before the mapping is
        // reused. The pointer never relocates — `PublishedRegion` moves do
        // not move the mapping.
        let outcome = unsafe { registrar.register(name, 1, gva as *const u8, len, flags) };
        match outcome {
            Ok(handle) => handles.push(handle),
            Err(detguest_sdk::RegionError::AgentUnavailable) => {
                // Not under the agent. Handles registered so far (there are
                // none: availability is uniform within a process) would be
                // dropped/unregistered here.
                return Ok(AgentPublication::Standalone);
            }
            Err(err) => {
                return Err(AgentPublishError {
                    region: name,
                    detail: format!("{err:?}"),
                });
            }
        }
    }

    let count = handles.len();
    // Manifest entries go DEAD when handles drop; these regions are
    // host-readable for the rest of the process, so leak deliberately.
    for handle in handles {
        std::mem::forget(handle);
    }
    Ok(AgentPublication::Registered { count })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outside the VM there is no detchannel: the real-path attempt must
    /// degrade to standalone, not fail — existing hosts of the harness
    /// (tests, refwork-verify) rely on this.
    #[test]
    fn publication_without_agent_is_standalone() {
        let regions = HarnessRegions::required().expect("map regions");
        let outcome = publish_regions(&regions).expect("standalone is not an error");
        assert_eq!(outcome, AgentPublication::Standalone);
    }

    struct FailingRegistrar;

    impl RegionRegistrar for FailingRegistrar {
        type Handle = ();

        unsafe fn register(
            &self,
            _name: &'static str,
            _layout_version: u32,
            _ptr: *const u8,
            _len: usize,
            _flags: RegionFlags,
        ) -> Result<Self::Handle, detguest_sdk::RegionError> {
            Err(detguest_sdk::RegionError::NotPinned)
        }
    }

    /// A non-`AgentUnavailable` registration failure under the agent is a
    /// hard error naming the region, never a silent standalone downgrade.
    #[test]
    fn hard_registration_failure_is_an_error_not_standalone() {
        let regions = HarnessRegions::required().expect("map regions");
        let err = publish_regions_with(&FailingRegistrar, &regions)
            .expect_err("hard failure must not degrade to standalone");
        assert_eq!(err.region, "wram");
        assert!(err.detail.contains("NotPinned"), "detail: {}", err.detail);
    }
}
