# Suggestions

### Add real `AF_UNIX SOCK_SEQPACKET` tests for the fd transport

- File/lines: `crates/refwork-harness/src/ctl.rs:100`, `crates/refwork-harness/src/ctl.rs:117`, `crates/refwork-harness/src/runner.rs:310`

The runner tests use `ScriptTransport`, which is useful for state-machine coverage, but no test currently exercises `SeqpacketFd` with a real `socketpair(AF_UNIX, SOCK_SEQPACKET)`. Add tests for datagram boundaries, type/domain validation, peer close, and the `MAX_DATAGRAM + 1` truncation path.

```rust
let mut fds = [-1; 2];
let rc = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) };
assert_eq!(rc, 0);
```

### Validate the fd domain and avoid `SIGPIPE` on control sends

- File/lines: `crates/refwork-harness/src/ctl.rs:117`, `crates/refwork-harness/src/ctl.rs:133`, `crates/refwork-harness/src/ctl.rs:166`

The code validates `SO_TYPE == SOCK_SEQPACKET`, but the bead asks for `AF_UNIX SOCK_SEQPACKET`. On Linux, `SO_DOMAIN` can verify `AF_UNIX`. Also consider using `MSG_NOSIGNAL` on `send` so a closed peer returns an error instead of terminating the harness before Rust error handling runs.

```rust
let flags = libc::MSG_NOSIGNAL;
let n = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), flags) };
```

### Assert setup metadata, not just message variants

- File/lines: `crates/refwork-harness/src/game.rs:67`, `crates/refwork-harness/src/runner.rs:399`, `crates/refwork-harness/src/runner.rs:418`, `crates/refwork-harness/src/runner.rs:419`

The happy-path test checks that `GameLoaded` exists, but not that `cart_hash`, `mapper`, or `sram_size` are correct. It also does not inspect the meta bytes after setup. Add assertions that `GameLoaded.cart_hash == blake3::hash(valid_rom())`, `mapper == "lorom"`, and the meta page contains the same cart hash and emulator version before `Ready`.

```rust
match &sent[1] {
    CtlMsg::GameLoaded { cart_hash, mapper, sram_size } => {
        assert_eq!(*cart_hash, blake3::hash(&valid_rom()).into());
        assert_eq!(mapper, "lorom");
        assert_eq!(*sram_size, 0);
    }
    other => panic!("expected GameLoaded, got {other:?}"),
}
```

### Reject trailing command-line arguments

- File/lines: `crates/refwork-harness/src/main.rs:6`, `crates/refwork-harness/src/main.rs:12`

`refwork-harness --fd3 extra` currently runs fd-3 mode and ignores `extra`. Rejecting trailing args makes startup failures easier to diagnose.

```rust
let mode = args.next();
if let Some(extra) = args.next() {
    eprintln!("refwork-harness: unexpected argument `{extra}`");
    std::process::exit(2);
}
```
