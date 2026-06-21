#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use refwork_emu::{Cartridge, Core, FrameFlags, RegionBuffers, FB_BYTES, WRAM_INIT_BYTE};
use refwork_harness::ctl::CONTROL_FD;
use refwork_harness::regions::{META_SIZE, WRAM_SIZE};
use refwork_protocol::{CtlMsg, FaultCode, MAX_DATAGRAM, PROTO_VERSION};

const FRAMES: u64 = 1_000;
const FB_BYTES_U64: u64 = FB_BYTES as u64;
const SEND_FLAGS: libc::c_int = libc::MSG_NOSIGNAL;

#[test]
fn mock_agent_happy_path_1000_frames() {
    let mut harness = spawn_mock_harness();

    perform_hello(harness.fd());
    send_load_game(harness.fd(), &harness.rom_file);
    expect_game_loaded(harness.fd(), &harness.rom);
    let regions = expect_regions_until_ready(harness.fd());
    assert_required_regions(&regions);

    // Prime every frame request before validation. The harness frame loop
    // free-runs and polls one queued datagram per frame boundary.
    send_msg(harness.fd(), &CtlMsg::Start {});
    for frame in 1..=FRAMES {
        send_msg(harness.fd(), &CtlMsg::HashRequest { frame });
    }

    let mut direct = DirectRun::new(&harness.rom);
    for frame in 1..=FRAMES {
        let report = expect_hash_report(harness.fd(), frame);
        let expected = direct.run_frame(frame);
        assert_eq!(
            report.wram, expected.wram,
            "wram hash mismatch at frame {frame}"
        );
        assert_eq!(
            report.fb, expected.fb,
            "framebuffer hash mismatch at frame {frame}"
        );
    }

    send_msg(harness.fd(), &CtlMsg::Shutdown {});
    wait_for_success(&mut harness.child);
}

#[test]
fn start_before_load_game_faults_protocol_order() {
    let mut harness = spawn_mock_harness();

    perform_hello(harness.fd());
    send_msg(harness.fd(), &CtlMsg::Start {});

    expect_fault(harness.fd(), FaultCode::ProtocolOrder, 0);
    let stderr = wait_for_failure(&mut harness.child);
    assert!(
        stderr.contains("setup failed"),
        "unexpected harness stderr: {stderr}"
    );
}

#[test]
fn hash_request_before_start_faults_protocol_order() {
    let mut harness = spawn_mock_harness();

    perform_hello(harness.fd());
    send_load_game(harness.fd(), &harness.rom_file);
    expect_game_loaded(harness.fd(), &harness.rom);
    let regions = expect_regions_until_ready(harness.fd());
    assert_required_regions(&regions);
    send_msg(harness.fd(), &CtlMsg::HashRequest { frame: 1 });

    expect_fault(harness.fd(), FaultCode::ProtocolOrder, 0);
    let stderr = wait_for_failure(&mut harness.child);
    assert!(
        stderr.contains("setup failed"),
        "unexpected harness stderr: {stderr}"
    );
}

#[test]
fn double_start_faults_protocol_order_at_first_frame_boundary() {
    let mut harness = spawn_mock_harness();

    complete_setup_until_ready(&harness);
    send_msg(harness.fd(), &CtlMsg::Start {});
    send_msg(harness.fd(), &CtlMsg::Start {});

    expect_fault(harness.fd(), FaultCode::ProtocolOrder, 1);
    let stderr = wait_for_failure(&mut harness.child);
    assert!(
        stderr.contains("frame loop failed"),
        "unexpected harness stderr: {stderr}"
    );
}

#[test]
fn malformed_postcard_datagram_faults_bad_proto() {
    let mut harness = spawn_mock_harness();

    send_raw(harness.fd(), &[0xff]);

    expect_fault(harness.fd(), FaultCode::BadProto, 0);
    let stderr = wait_for_failure(&mut harness.child);
    assert!(
        stderr.contains("setup failed"),
        "unexpected harness stderr: {stderr}"
    );
}

#[test]
fn oversize_datagram_faults_bad_proto() {
    let mut harness = spawn_mock_harness();
    let bytes = vec![0u8; MAX_DATAGRAM + 2];

    send_raw(harness.fd(), &bytes);

    expect_fault(harness.fd(), FaultCode::BadProto, 0);
    let stderr = wait_for_failure(&mut harness.child);
    assert!(
        stderr.contains("setup failed"),
        "unexpected harness stderr: {stderr}"
    );
}

struct MockHarness {
    fd: OwnedFd,
    child: HarnessChild,
    rom: Vec<u8>,
    rom_file: TempRom,
}

impl MockHarness {
    fn fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

fn spawn_mock_harness() -> MockHarness {
    let rom = xtask::build_synth_rom();
    let rom_file = TempRom::write(&rom);
    let (fd, harness_fd) = seqpacket_pair();
    configure_socket(fd.as_raw_fd());
    configure_socket(harness_fd.as_raw_fd());
    let child = spawn_harness(harness_fd);
    MockHarness {
        fd,
        child,
        rom,
        rom_file,
    }
}

fn perform_hello(fd: RawFd) {
    send_msg(
        fd,
        &CtlMsg::Hello {
            proto_version: PROTO_VERSION,
        },
    );
    expect_hello_ack(fd);
}

fn send_load_game(fd: RawFd, rom_file: &TempRom) {
    send_msg(
        fd,
        &CtlMsg::LoadGame {
            dev_path: rom_file.path.display().to_string(),
        },
    );
}

fn complete_setup_until_ready(harness: &MockHarness) {
    perform_hello(harness.fd());
    send_load_game(harness.fd(), &harness.rom_file);
    expect_game_loaded(harness.fd(), &harness.rom);
    let regions = expect_regions_until_ready(harness.fd());
    assert_required_regions(&regions);
}

struct HashPair {
    wram: [u8; 32],
    fb: [u8; 32],
}

struct DirectRun {
    core: Core,
    fb: Box<[u8; FB_BYTES]>,
}

impl DirectRun {
    fn new(rom: &[u8]) -> Self {
        let cart = Cartridge::from_rom(rom.to_vec(), None).expect("synthetic ROM is valid");
        let wram = Box::leak(Box::new([WRAM_INIT_BYTE; WRAM_SIZE]));
        let regions = RegionBuffers {
            wram,
            vram: None,
            sram: None,
        };
        let core = Core::new(cart, regions).expect("direct core construction");
        Self {
            core,
            fb: Box::new([0u8; FB_BYTES]),
        }
    }

    fn run_frame(&mut self, frame: u64) -> HashPair {
        let flags = self.core.run_one_frame(0);
        assert!(
            !flags.contains(FrameFlags::FAULTED),
            "direct core faulted at frame {frame}: {:?}",
            self.core.fault()
        );
        self.core.blit_completed_frame(&mut self.fb);
        assert_eq!(self.core.frame_counter(), frame);
        HashPair {
            wram: blake3::hash(self.core.wram()).into(),
            fb: blake3::hash(&self.fb[..]).into(),
        }
    }
}

#[derive(Debug)]
struct RegionInfo {
    len: u64,
    writable: bool,
}

fn expect_hello_ack(fd: RawFd) {
    match recv_msg(fd) {
        CtlMsg::HelloAck {
            proto_version,
            emu,
            emu_version,
        } => {
            assert_eq!(proto_version, PROTO_VERSION);
            assert_eq!(emu, "refwork-emu");
            assert!(
                emu_version.starts_with("refwork-emu "),
                "unexpected emulator version {emu_version}"
            );
        }
        other => panic!("expected HelloAck, got {other:?}"),
    }
}

fn expect_game_loaded(fd: RawFd, rom: &[u8]) {
    match recv_msg(fd) {
        CtlMsg::GameLoaded {
            cart_hash,
            mapper,
            sram_size,
        } => {
            let expected_hash: [u8; 32] = blake3::hash(rom).into();
            assert_eq!(cart_hash, expected_hash);
            assert_eq!(mapper, "lorom");
            assert_eq!(sram_size, 0);
        }
        CtlMsg::Fault {
            frame,
            code,
            detail,
        } => panic!("unexpected Fault({code:?}) at frame {frame}: {detail}"),
        other => panic!("expected GameLoaded, got {other:?}"),
    }
}

fn expect_regions_until_ready(fd: RawFd) -> BTreeMap<String, RegionInfo> {
    let mut regions = BTreeMap::new();
    loop {
        match recv_msg(fd) {
            CtlMsg::RegisterRegion {
                name,
                gva,
                len,
                writable,
            } => {
                assert_ne!(gva, 0, "region {name} gva must be nonzero");
                assert_ne!(len, 0, "region {name} len must be nonzero");
                regions.insert(name, RegionInfo { len, writable });
            }
            CtlMsg::Ready { frame } => {
                assert_eq!(frame, 0);
                return regions;
            }
            CtlMsg::Fault {
                frame,
                code,
                detail,
            } => panic!("unexpected Fault({code:?}) at frame {frame}: {detail}"),
            other => panic!("expected RegisterRegion or Ready, got {other:?}"),
        }
    }
}

fn assert_required_regions(regions: &BTreeMap<String, RegionInfo>) {
    assert_eq!(regions.get("wram").map(|r| r.len), Some(WRAM_SIZE as u64));
    assert_eq!(
        regions.get("framebuffer").map(|r| r.len),
        Some(FB_BYTES_U64)
    );
    assert_eq!(regions.get("meta").map(|r| r.len), Some(META_SIZE as u64));
    assert!(
        regions.values().all(|region| !region.writable),
        "published regions should be read-only descriptors: {regions:?}"
    );
}

fn expect_hash_report(fd: RawFd, expected_frame: u64) -> HashPair {
    match recv_msg(fd) {
        CtlMsg::HashReport { frame, wram, fb } => {
            assert_eq!(frame, expected_frame);
            HashPair { wram, fb }
        }
        CtlMsg::Fault {
            frame,
            code,
            detail,
        } => panic!("unexpected Fault({code:?}) at frame {frame}: {detail}"),
        other => panic!("expected HashReport({expected_frame}), got {other:?}"),
    }
}

fn expect_fault(fd: RawFd, expected_code: FaultCode, expected_frame: u64) {
    match recv_msg(fd) {
        CtlMsg::Fault {
            frame,
            code,
            detail,
        } => {
            assert_eq!(frame, expected_frame);
            assert_eq!(code, expected_code);
            assert!(
                !detail.is_empty(),
                "Fault({expected_code:?}) should include bounded detail"
            );
        }
        other => panic!("expected Fault({expected_code:?}), got {other:?}"),
    }
}

fn send_msg(fd: RawFd, msg: &CtlMsg) {
    let bytes = refwork_protocol::encode(msg).expect("encode control message");
    send_raw(fd, &bytes);
}

fn send_raw(fd: RawFd, bytes: &[u8]) {
    loop {
        let sent = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), SEND_FLAGS) };
        if sent == bytes.len() as isize {
            return;
        }
        let err = io::Error::last_os_error();
        if sent < 0 && err.kind() == io::ErrorKind::Interrupted {
            continue;
        }
        panic!("send failed for {} byte datagram: {err}", bytes.len());
    }
}

fn recv_msg(fd: RawFd) -> CtlMsg {
    let mut buf = [0u8; 8192];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
    assert!(n > 0, "recv failed: {}", io::Error::last_os_error());
    refwork_protocol::decode(&buf[..n as usize]).expect("decode control message")
}

fn seqpacket_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    let rc = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    assert_eq!(rc, 0, "socketpair failed: {}", io::Error::last_os_error());
    let left = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let right = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    (left, right)
}

fn configure_socket(fd: RawFd) {
    set_socket_buffer(fd, libc::SO_RCVBUF, 4 * 1024 * 1024);
    set_socket_buffer(fd, libc::SO_SNDBUF, 4 * 1024 * 1024);
    set_socket_timeout(fd, libc::SO_RCVTIMEO);
    set_socket_timeout(fd, libc::SO_SNDTIMEO);
}

fn set_socket_buffer(fd: RawFd, opt: libc::c_int, bytes: libc::c_int) {
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            opt,
            (&bytes as *const libc::c_int).cast(),
            std::mem::size_of_val(&bytes) as libc::socklen_t,
        )
    };
    assert_eq!(
        rc,
        0,
        "setsockopt buffer option {opt} failed: {}",
        io::Error::last_os_error()
    );
}

fn set_socket_timeout(fd: RawFd, opt: libc::c_int) {
    let timeout = libc::timeval {
        tv_sec: 10,
        tv_usec: 0,
    };
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            opt,
            (&timeout as *const libc::timeval).cast(),
            std::mem::size_of_val(&timeout) as libc::socklen_t,
        )
    };
    assert_eq!(
        rc,
        0,
        "setsockopt timeout option {opt} failed: {}",
        io::Error::last_os_error()
    );
}

fn spawn_harness(harness_fd: OwnedFd) -> HarnessChild {
    let raw_fd = harness_fd.as_raw_fd();
    let mut command = Command::new(env!("CARGO_BIN_EXE_refwork-harness"));
    command
        .arg("--fd3")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    unsafe {
        command.pre_exec(move || {
            if raw_fd != CONTROL_FD {
                let rc = libc::dup2(raw_fd, CONTROL_FD);
                if rc < 0 {
                    return Err(io::Error::last_os_error());
                }
                libc::close(raw_fd);
            }
            let flags = libc::fcntl(CONTROL_FD, libc::F_GETFD);
            if flags < 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::fcntl(CONTROL_FD, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = command.spawn().expect("spawn refwork-harness");
    drop(harness_fd);
    HarnessChild {
        child,
        reaped: false,
    }
}

struct HarnessChild {
    child: Child,
    reaped: bool,
}

impl Drop for HarnessChild {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        match self.child.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}

fn wait_for_success(harness: &mut HarnessChild) {
    for _ in 0..1_000 {
        match harness.child.try_wait().expect("poll child") {
            Some(status) if status.success() => {
                harness.reaped = true;
                return;
            }
            Some(status) => {
                harness.reaped = true;
                let stderr = read_child_stderr(&mut harness.child);
                panic!("harness exited with {status}: {stderr}");
            }
            None => unsafe {
                libc::usleep(10_000);
            },
        }
    }

    let _ = harness.child.kill();
    let _ = harness.child.wait();
    harness.reaped = true;
    let stderr = read_child_stderr(&mut harness.child);
    panic!("harness did not exit after Shutdown: {stderr}");
}

fn wait_for_failure(harness: &mut HarnessChild) -> String {
    for _ in 0..1_000 {
        match harness.child.try_wait().expect("poll child") {
            Some(status) if !status.success() => {
                harness.reaped = true;
                return read_child_stderr(&mut harness.child);
            }
            Some(status) => {
                harness.reaped = true;
                panic!("harness unexpectedly exited successfully with {status}");
            }
            None => unsafe {
                libc::usleep(10_000);
            },
        }
    }

    let _ = harness.child.kill();
    let _ = harness.child.wait();
    harness.reaped = true;
    let stderr = read_child_stderr(&mut harness.child);
    panic!("harness did not exit after protocol fault: {stderr}");
}

fn read_child_stderr(child: &mut Child) -> String {
    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }
    stderr
}

struct TempRom {
    dir: PathBuf,
    path: PathBuf,
}

impl TempRom {
    fn write(rom: &[u8]) -> Self {
        for attempt in 0..1000u32 {
            let dir = std::env::temp_dir().join(format!(
                "refwork-harness-mock-agent-{}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => {
                    let path = dir.join("synth.rom");
                    std::fs::write(&path, rom).expect("write synthetic ROM");
                    return Self { dir, path };
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                Err(err) => panic!("create temp dir failed: {err}"),
            }
        }
        panic!("could not allocate temp dir");
    }
}

impl Drop for TempRom {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
