# Critical And Important Issues

## Critical

None.

## Important

### 1. Post-READY malformed or oversized datagrams leave `meta.status` as ready

- Severity: Important
- File/lines: `crates/refwork-harness/src/runner.rs:132`, `crates/refwork-harness/src/runner.rs:148`, `crates/refwork-harness/src/runner.rs:154`, `crates/refwork-harness/src/runner.rs:159`, `crates/refwork-harness/src/runner.rs:231`

Problem: `expect_start` calls `recv_agent_msg(channel)?` before it can inspect or mark the meta page. If the agent sends a malformed or oversized datagram after the harness has already sent `RegisterRegion` and `Ready`, `recv_agent_msg` sends `Fault { code: BadProto }` but has no access to `regions`, so `meta.status` remains `ready`. Only decoded-but-out-of-order messages take the `mark_meta_fault(regions, FaultCode::ProtocolOrder)` path.

Suggested fix:

```rust
fn expect_start<T>(
    channel: &mut ControlChannel<T>,
    regions: &mut HarnessRegions,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    match recv_agent_msg_with_meta_fault(channel, Some(regions))? {
        CtlMsg::Start {} => Ok(()),
        actual => {
            mark_meta_fault(regions, FaultCode::ProtocolOrder);
            protocol_order(channel, "Start", actual)
        }
    }
}

fn recv_agent_msg_with_meta_fault<T>(
    channel: &mut ControlChannel<T>,
    mut regions: Option<&mut HarnessRegions>,
) -> Result<CtlMsg, SetupError>
where
    T: DatagramTransport,
{
    match channel.recv_msg() {
        Ok(msg) => Ok(msg),
        Err(ControlError::Oversize { len }) => {
            if let Some(regions) = regions.as_deref_mut() {
                mark_meta_fault(regions, FaultCode::BadProto);
            }
            let detail = format!("oversize control datagram: {len} bytes");
            send_fault(channel, FaultCode::BadProto, &detail)?;
            Err(SetupError::BadProto { detail })
        }
        Err(ControlError::Decode(err)) => {
            if let Some(regions) = regions.as_deref_mut() {
                mark_meta_fault(regions, FaultCode::BadProto);
            }
            let detail = err.to_string();
            send_fault(channel, FaultCode::BadProto, &detail)?;
            Err(SetupError::BadProto { detail })
        }
        Err(err) => Err(SetupError::Control(err)),
    }
}
```

Add a test where setup reaches `Ready`, the next inbound datagram is malformed or `MAX_DATAGRAM + 2`, and the returned `SetupResult` or inspected meta bytes show `Faulted` with `BadProto`.

### 2. The production binary drops all published regions and exits after `Start`

- Severity: Important
- File/lines: `crates/refwork-harness/src/main.rs:28`, `crates/refwork-harness/src/main.rs:30`, `crates/refwork-harness/src/runner.rs:96`, `crates/refwork-harness/src/runner.rs:100`, `crates/refwork-harness/src/runner.rs:101`, `crates/refwork-harness/src/regions.rs:177`

Problem: `run_fd3` treats `run_setup(...)` as a complete production run. On success, the temporary `SetupResult` is immediately dropped, which drops `HarnessRegions` and unmaps the addresses that were just sent via `RegisterRegion`. The process then exits 0 after accepting `Start`. That makes the advertised fd-3 production mode unusable for a real agent and invalidates the region publication handoff.

Suggested fix:

```rust
fn run_fd3() {
    let transport = open_control_or_exit();
    let mut channel = ControlChannel::new(transport);
    let mut loader = FilesystemGameLoader;

    let setup = match run_setup(&mut channel, &mut loader, SetupConfig::default()) {
        Ok(setup) => setup,
        Err(err) => {
            eprintln!("refwork-harness: setup failed: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = run_frame_loop(&mut channel, setup) {
        eprintln!("refwork-harness: frame loop failed: {err}");
        std::process::exit(1);
    }
}
```

If the frame loop is intentionally deferred to the next bead, the binary should not silently claim success after `Start` while tearing down the published mappings. Keep `SetupResult` owned by the next runner phase, or make the temporary unsupported state explicit and non-successful.

### 3. Optional SRAM publication does not match `GameLoaded` metadata or cartridge wiring

- Severity: Important
- File/lines: `crates/refwork-harness/src/game.rs:67`, `crates/refwork-harness/src/game.rs:69`, `crates/refwork-harness/src/game.rs:74`, `crates/refwork-harness/src/runner.rs:195`, `crates/refwork-harness/src/runner.rs:243`, `crates/refwork-harness/src/runner.rs:246`, `crates/refwork-harness/src/regions.rs:290`

Problem: `SetupConfig` can request an SRAM region, and `HarnessRegions::with_optional` will publish it. Separately, `loaded_game_from_rom` always constructs the cartridge with `Cartridge::from_rom(rom, None)` and always reports `sram_size: 0`. With `sram_len: Some(8192)`, the agent can receive a registered `sram` region while `GameLoaded` says `sram_size = 0`, and the `LoadedGame.cart` still has no SRAM slice for the emulator core.

Suggested fix:

```rust
pub struct LoadedRom {
    pub rom: Vec<u8>,
    pub cart_hash: [u8; 32],
    pub mapper: String,
}

let loaded = loader.load_rom(&dev_path)?;
let mut regions = HarnessRegions::with_optional(config.vram, config.sram_len)?;
let sram_size = config.sram_len.unwrap_or(0);

// When activating for the emulator, pass the same SRAM slice used for the
// published region into Cartridge::from_rom.
let active = unsafe { regions.activate_for_emu()? };
let cart = Cartridge::from_rom(loaded.rom, active.buffers.sram)?;

channel.send_msg(&CtlMsg::GameLoaded {
    cart_hash: loaded.cart_hash,
    mapper: loaded.mapper,
    sram_size: sram_size as u32,
})?;
```

If SRAM is not part of this bead, reject `sram_len: Some(_)` for now or do not publish the region. The metadata, registered regions, and eventual `Cartridge` must all come from the same SRAM decision.
