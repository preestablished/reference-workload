# 01 - Confirm M2 Host-Side Floor

**Upstream package:** RW-0.

**Purpose:** do not build guest-sdk unblock work on an assumed emulator floor.
Before M3/M4/M5 evidence is accepted, confirm that this repo has a host-side
first-room route and deterministic emulator evidence, or record an explicit
operator waiver with scope.

## Deliverables

1. Add or update a repo-local evidence note at
   `.agents/plans/guest-sdk-unblock-reference-workload/m2-floor-evidence.md`
   during implementation. It should contain:
   - git rev and branch used for the floor check;
   - date, owner, and machine for every evidence run;
   - host-side first-room script evidence path on the lab runner;
   - feature-map offset evidence pointer, usually the phase-2 bring-up log or
     lab-side `ramdiff watch` records;
   - x86_64 and aarch64 hash evidence;
   - the `determinism-proto` source used by this checkout: sibling
     `../control-plane` git rev, tag, or explicitly approved replacement;
   - any waiver, with date, owner, reason, and exact scope.
2. Re-run repo-side synthetic gates to confirm the working tree has not
   regressed since the phase-2 plan was written.
3. Verify the control-plane proto floor before running workspace commands. The
   normal preestablished layout has `../control-plane`; clean double-build roots
   in package 04 need the same sibling checkout or a pinned substitute.
4. Check the operator-game evidence without copying game content into the repo.
   Record only hashes, lab paths, and pass/fail status.

## Commands

Run these locally before editing packages 02-04:

```sh
test -d ../control-plane
cargo build --locked --manifest-path ../control-plane/Cargo.toml -p determinism-proto --all-features
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run --locked -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml
cargo run --locked -p xtask -- deny
cargo test --release --locked -p xtask --test determinism -- --include-ignored
cargo test --release --locked -p xtask --test zero_alloc
cargo run --locked --release -p xtask -- hash-chain --frames 10000
```

If the public test-ROM corpora are available in `target/test-roms`, also run:

```sh
cargo run --locked --release -p xtask -- cpu-tests
cargo run --locked --release -p xtask -- spc-tests
```

On the lab runner, the evidence note should point to equivalent operator-game
commands, not embed their outputs if they include game-derived data:

```sh
refwork-verify play --rom <operator>.rom --script <first-room>.padlog --map feature-maps/demo-game.yaml --report <lab>/m2-run.json
refwork-verify map-check --rom <operator>.rom --map feature-maps/demo-game.yaml --script <first-room>.padlog --expect <lab>/first-room-expect.yaml
refwork-verify double-run --rom <operator>.rom --script <first-room>.padlog --frames 100000 --report <lab>/double-run.json
```

Run the 100k-frame double-run on both x86_64 and real aarch64 lab hardware.
QEMU is fine for CI signal, but it is not M2 evidence.

## Acceptance

- `m2-floor-evidence.md` exists and maps every RW-0 acceptance clause to a
  command or lab artifact pointer.
- M2 engine packages and `refwork-verify` are present and build.
- Host-side first-room script and feature-map offset evidence are recorded, or
  an explicit waiver is recorded with date, owner, reason, and scope.
- x86_64 and aarch64 deterministic hash evidence is recorded for the host-side
  floor.
- The `determinism-proto` source is recorded and buildable from this checkout.
- No game content, framebuffer goldens, WRAM dumps, or script semantics are
  committed to the repo.

## Stop Conditions

- If `feature-maps/demo-game.yaml` still contains placeholder offsets and no
  waiver exists, stop package 05/06 integration work. Package 02/03 can still
  proceed using the synthetic ROM.
- If cross-arch hash evidence diverges, treat it as a P0 determinism bug in the
  emulator floor before building in-VM evidence on top.
- If `../control-plane` is missing and no pinned replacement is recorded, stop
  and restore the shared proto build input before running workspace gates.
- If a waiver is used, keep it narrow. A waiver for host-side first-room
  evidence does not waive protocol, image reproducibility, or suite evidence.
