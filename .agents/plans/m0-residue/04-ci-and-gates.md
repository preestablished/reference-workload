# 04 — CI wiring and gate hardening

Depends on: 01–03. All edits are to `.github/workflows/ci.yaml` and
`xtask/src/deny.rs` unless noted.

## 1. Feature-map validation step (M0 acceptance, clause 1)

Add to CI after the build step:

```yaml
- run: cargo run -p refwork-featuremap -- validate feature-maps/demo-game.yaml --scoring scoring/demo-game.yaml
  working-directory: repo
```

(The negative fixtures run inside `cargo test --workspace` via the fixtures
integration test — no separate CI step needed.)

## 2. JSON-Schema drift gate

The committed `schema/feature-map.schema.json` must always match the types:

```yaml
- run: |
    cargo run -p refwork-featuremap -- schema > /tmp/feature-map.schema.json
    diff -u schema/feature-map.schema.json /tmp/feature-map.schema.json
  working-directory: repo
```

Requires package 01's byte-deterministic schema output (incl. the pinned
trailing-newline convention).

Determinism hardening for this gate:
- Pass `--locked` on the new CI cargo steps (Cargo.lock is committed); the
  schema's byte-stability depends on `schemars`/`serde_json` not drifting.
  (The sibling `control-plane` checkout pins nothing — its default-branch
  HEAD is an accepted pre-existing CI looseness, but our lock at least pins
  the schema-relevant crates.)
- Document the one-command regeneration next to the gate so a legitimate
  type change is mechanical:
  `cargo run -p refwork-featuremap -- schema > schema/feature-map.schema.json`.

## 3. Deny-gate scope (+ the "demonstrably fails" proof)

`xtask/src/deny.rs` currently scans (verified at lines ~224-225):

```rust
workspace_root.join("crates/refwork-emu/src"),
workspace_root.join("crates/refwork-harness/src"),
```

Changes:
a. Add `crates/refwork-protocol/src` to the scan set — the protocol crate
   links into the guest harness binary and inherits D1–D4.
b. **Self-test proving the M0 acceptance clause** ("CI deny-gates
   demonstrably fail a PR that adds `std::thread` to `refwork-emu`"):
   add an integration test `xtask/tests/deny_selftest.rs` that
   1. copies `crates/refwork-emu/src/lib.rs` into a temp dir along with a
      planted file containing `std::thread::spawn(|| {});` (and one case per
      banned token class: `tokio`, `rand`, `f32`, `Instant`, `HashMap`),
   2. runs the public scan function over that temp tree,
   3. asserts each planted token is found at the right file:line, and
   4. asserts a comment-only occurrence (`// std::thread is banned`) is NOT
      flagged.
   This runs in `cargo test --workspace` on every PR — the failure mode is
   demonstrated continuously without committing a poisoned crate.
   Precondition: `deny.rs` must expose a directory-scanning function taking a
   root path (today `scan_file` is `pub`; add/`pub`-ify a `scan_tree(root)` if
   the current entry point hard-codes the workspace roots).

## 4. Protocol crate in the workspace test sweep

No CI change needed — `cargo test --workspace` picks up the new
round-trip/golden-bytes tests automatically. Verify the golden-bytes test is
NOT `#[ignore]`d.

## 5. Ordering note

Keep the deny gate before the build step (cheap fail-fast), as it is today.
Place validate + schema-drift after build (they need a compiled binary; with
`cargo run` they compile on demand anyway, so position is a cache choice, not
correctness).
