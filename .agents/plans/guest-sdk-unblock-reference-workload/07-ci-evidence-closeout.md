# 07 - CI, Evidence, And Closeout

**Purpose:** turn packages 02-06 into durable gates and handoff artifacts that
guest-sdk can cite when closing external blockers.

## Deliverables

1. `xtask audit-syms`:
   - Wire the package-03 implementation:
     `cargo run --locked -p xtask -- audit-syms --bin target/.../refwork-harness`.
   - Scan the release harness binary for clock, sleep, timer, thread, pthread,
     async runtime, RNG, and other banned symbols from ARCHITECTURE.md D1-D4.
   - Use `nm`, `readelf`, or platform-available equivalents from Rust. If a
     platform tool is missing, fail with a clear message rather than silently
     passing.
   - Keep source-token deny (`cargo xtask deny`) as a separate gate; symbol audit
     catches linked artifact drift.
2. Per-PR CI additions:
   - `cargo test -p refwork-harness`.
   - Mock-agent happy path and abuse tests from package 03.
   - `cargo run -p xtask -- deny`.
   - Release `refwork-harness` build plus `xtask audit-syms`.
3. Nightly or manual CI additions:
   - `cargo run --locked -p xtask -- image double-build` once package 04 is
     implemented.
   - Lab-only `refwork-verify vm-first-room` once package 05 dependencies exist.
   - Lab-only `refwork-verify suite --full` once package 06 dependencies exist.
   - 20-run Intel zero-flake job before stamping an image green.
4. Evidence notes:
   - `m2-floor-evidence.md` from package 01.
   - `m3-mock-agent-evidence.md`: command, CI run, fixture path, protocol abuse
     coverage.
   - `m4-image-handoff-evidence.md`: manifest path/hash, double-build result,
     expected-region list, `boot.toml`, region names/sizes.
   - `m4-in-vm-first-room-evidence.md`: lab report path/hash, READY proof,
     room transition, framebuffer checkpoint hashes.
   - `m5-suite-evidence.md`: 20-run report hashes and green-stamp data.
   - Every lab evidence note must name the owner, runner label or machine,
     artifact root, external repo revisions, exact command, and report hash.
5. Guest-sdk handoff:
   - Provide stable artifact paths for:
     - mock-agent fixture command and code path;
     - `workload-image.yaml`;
     - `boot.toml`;
     - expected-region list;
     - pad layout;
     - region names/sizes;
     - suite report hash.
   - Record which guest-sdk blocker each artifact closes.

## Evidence Matrix

| Blocker | Required evidence | Package |
|---|---|---|
| `guest-sdk-ext-refwork-m3-mock-agent` | Mock-agent fixture path, CI run, abuse-test list, release harness audit | 03, 07 |
| `guest-sdk-ext-refwork-m4-image-handoff` | WorkloadImage manifest, `boot.toml`, expected-region list, region sizes, pad layout, image double-build | 04, 07 |
| `guest-sdk-ext-refwork-m5-full-suite` | 20-run zero-flake full-suite report with double-run, snapshot/restore, in-guest/host hash cross-checks | 06, 07 |

## CI Placement Guidance

- Keep synthetic-ROM tests in normal GitHub CI.
- Keep operator-ROM tests on lab/self-hosted lanes only.
- Do not upload large lab artifacts to public CI if they contain game-derived
  bytes. Upload compact JSON reports with hashes and lab paths instead.
- `audit-syms` should run on the same target artifact that goes into the image,
  not just a debug host binary.

## Closeout Checklist

- [ ] Package 01 evidence note exists.
- [ ] Package 02 harness binary implemented and deny-clean.
- [ ] Package 03 mock-agent tests in CI.
- [ ] `xtask audit-syms` implemented and run on release harness.
- [ ] Package 04 image build, validate, and double-build implemented.
- [ ] Package 05 in-VM first-room lab report recorded.
- [ ] Package 06 full suite 20-run report recorded.
- [ ] Guest-sdk handoff paths collected in a final comment or issue note.
- [ ] Docs updated with as-built commands, not just plan text.

## Stop Conditions

- If an evidence item only exists in chat or terminal scrollback, it does not
  close a blocker. Put a path, CI URL, or report hash in the evidence notes.
- If a lab run used a different image, guest-sdk rev, hypervisor rev, or ROM hash
  than the report claims, discard the evidence and rerun.
- If a gate is flaky, do not green-stamp the image.
