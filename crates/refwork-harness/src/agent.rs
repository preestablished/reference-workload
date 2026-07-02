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

/// Register `wram`, `framebuffer`, and `meta` with the agent under the D7
/// contract (`layout_version 1` for all three; the framebuffer flag drives
/// the hypervisor's geometry derivation).
///
/// Call after the regions are mapped and `meta` is initialized, before the
/// harness reports `Ready` — READY must not precede live regions.
pub fn publish_regions(regions: &HarnessRegions) -> Result<AgentPublication, AgentPublishError> {
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
        let outcome =
            unsafe { detguest_sdk::register_region(name, 1, gva as *const u8, len, flags) };
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
}
