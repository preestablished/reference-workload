# Decision: Kernel And Agent Enter The Image Differently

Date: 2026-07-02. Decided by Matt (operator) with Claude (coding agent),
while unblocking the real package-04 image
(`.agents/plans/phase3-m4-first-room-unblock/`, step 03). Supersedes the
`pinned-placeholder` scheme both locks carried since package 04 landed.

## The Split

The two guest-sdk-owned inputs to the workload image are deliberately
treated differently:

| Input | Treatment | Lock | Verified how |
|---|---|---|---|
| Kernel (`bzImage`) | **Hash-pinned artifact handoff** | `image/kernel.lock` (`status = "pinned-artifact"`) | `xtask image build` reads `../guest-sdk/image/build/bzImage` and refuses on BLAKE3 mismatch with the lock |
| Agent (`detguest-agent`) | **Built from the sibling at a pinned rev** | `image/guest-sdk.lock` (`status = "pinned-rev"`) | `xtask image build` refuses unless `git -C ../guest-sdk rev-parse HEAD` equals the lock rev, then runs the musl release build |

## Why The Asymmetry

- **The kernel is expensive and rev-stable.** guest-sdk owns a complete
  deterministic kernel pipeline (its `image/KERNEL.md` + `image/build.sh
  kernel`): Linux 6.12.93 LTS pinned by tarball SHA256, a
  determinism-tuned config, and a provenance file whose `build_key` rolls
  every build input. Kernel builds are heavy and toolchain-sensitive;
  importing that infrastructure here would duplicate knowledge guest-sdk
  already maintains. Consuming the artifact and pinning its BLAKE3 (plus
  recording `kernel_version` and the provenance `build_key`) keeps bumps
  deliberate and the provenance chain intact — the placeholder lock's
  `deterministic_build_required` policy is discharged by guest-sdk's own
  provenance, recorded in the lock as `deterministic_build_discharged_by`.
- **The agent is cheap and rev-coupled.** It is a seconds-cheap cargo
  target (`cargo build --locked --release --target
  x86_64-unknown-linux-musl -p detguest-agent`) — the exact recipe
  guest-sdk's own Ms4 acceptance uses. Building it from the pinned rev at
  image-build time means it cannot drift from the rev the lock names, and
  there is no artifact-publishing pipeline to invent on the guest-sdk
  side. The harness's `detguest-sdk` path dep tracks the same checkout, so
  agent and SDK are always built from one rev.

## Consequences

- `xtask image build` no longer takes a required `--agent-bin`; the flag
  remains as a test/escape hatch that skips the rev check.
- `image/boot.toml` was rewritten to the agent's real schema
  (`boot_toml_version = 1`, `[[unit]]` + `[unit.control]` with
  `refwork-ctl`, `[[expected_region]]` name + layout_version — modeled on
  guest-sdk's `boot.toml.m9-refwork-contract`). The old speculative
  `schema_version = 1` shape predated the real agent parser. Sizes and
  formats stay in `expected-regions.toml`; the agent gates READY on
  name + layout_version only.
- The kernel cmdline is owned by determinism-hypervisor (ARCHITECTURE.md
  §2.3): the worker forces the canonical deterministic baseline for
  `BzImageBoot`, so neither this repo nor the lock states a cmdline.
- `image double-build` gates the guest-sdk sibling's cleanliness scoped to
  `crates/`, `Cargo.toml`, and `Cargo.lock` (the agent's and SDK's build
  inputs). The kernel artifact under guest-sdk's `image/build/` is not
  git-tracked there — the BLAKE3 pin in `kernel.lock` is what gates it.
- Bumping: kernel = rebuild in guest-sdk, then update
  `kernel_version`/`build_key`/`blake3` together in `kernel.lock`;
  agent = update `rev` in `guest-sdk.lock`. Never bump either implicitly.
- `builder.lock` (container toolchain pin) intentionally stays a
  placeholder; it is a separate concern from this split.

## Regression Guards

- `xtask/tests/image_inputs.rs::kernel_and_guest_sdk_locks_are_real_pins`
  fails if either lock regresses to a placeholder or loses its pin fields.
- `xtask/tests/image_inputs.rs::boot_toml_matches_the_agent_schema` pins
  the boot.toml schema shape.
- The build-time validator (`validate_boot_toml`) enforces the agent
  schema on every `image build`/`validate`.
