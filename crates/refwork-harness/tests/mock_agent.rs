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
use refwork_protocol::{CtlMsg, PROTO_VERSION};

const FRAMES: u64 = 1_000;
const REQUEST_WINDOW: u64 = 32;
const FB_BYTES_U64: u64 = FB_BYTES as u64;

#[test]
fn mock_agent_happy_path_1000_frames() {
    let rom = xtask::build_synth_rom();
    let rom_file = TempRom::write(&rom);
    let (agent_fd, harness_fd) = seqpacket_pair();
    set_recv_timeout(agent_fd.as_raw_fd());
    let mut child = spawn_harness(harness_fd);

    send_msg(
        agent_fd.as_raw_fd(),
        &CtlMsg::Hello {
            proto_version: PROTO_VERSION,
        },
    );
    expect_hello_ack(agent_fd.as_raw_fd());

    send_msg(
        agent_fd.as_raw_fd(),
        &CtlMsg::LoadGame {
            dev_path: rom_file.path.display().to_string(),
        },
    );
    send_msg(agent_fd.as_raw_fd(), &CtlMsg::Start {});
    for frame in 1..=REQUEST_WINDOW {
        send_msg(agent_fd.as_raw_fd(), &CtlMsg::HashRequest { frame });
    }

    expect_game_loaded(agent_fd.as_raw_fd(), &rom);
    let regions = expect_regions_until_ready(agent_fd.as_raw_fd());
    assert_required_regions(&regions);

    let mut direct = DirectRun::new(&rom);
    for frame in 1..=FRAMES {
        let report = expect_hash_report(agent_fd.as_raw_fd(), frame);
        let next_request = frame + REQUEST_WINDOW;
        if next_request <= FRAMES {
            send_msg(
                agent_fd.as_raw_fd(),
                &CtlMsg::HashRequest {
                    frame: next_request,
                },
            );
        }
        if frame == FRAMES {
            send_msg(agent_fd.as_raw_fd(), &CtlMsg::Shutdown {});
        }

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

    wait_for_success(&mut child);
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

fn send_msg(fd: RawFd, msg: &CtlMsg) {
    let bytes = refwork_protocol::encode(msg).expect("encode control message");
    let sent = unsafe { libc::send(fd, bytes.as_ptr().cast(), bytes.len(), 0) };
    assert_eq!(
        sent,
        bytes.len() as isize,
        "send failed for {msg:?}: {}",
        io::Error::last_os_error()
    );
}

fn recv_msg(fd: RawFd) -> CtlMsg {
    let mut buf = [0u8; 8192];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr().cast(), buf.len(), 0) };
    assert!(n > 0, "recv failed: {}", io::Error::last_os_error());
    refwork_protocol::decode(&buf[..n as usize]).expect("decode control message")
}

fn seqpacket_pair() -> (OwnedFd, OwnedFd) {
    let mut fds = [-1; 2];
    let rc = unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) };
    assert_eq!(rc, 0, "socketpair failed: {}", io::Error::last_os_error());
    let left = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let right = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    (left, right)
}

fn set_recv_timeout(fd: RawFd) {
    let timeout = libc::timeval {
        tv_sec: 10,
        tv_usec: 0,
    };
    let rc = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&timeout as *const libc::timeval).cast(),
            std::mem::size_of_val(&timeout) as libc::socklen_t,
        )
    };
    assert_eq!(
        rc,
        0,
        "setsockopt SO_RCVTIMEO failed: {}",
        io::Error::last_os_error()
    );
}

fn spawn_harness(harness_fd: OwnedFd) -> Child {
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
            Ok(())
        });
    }

    let child = command.spawn().expect("spawn refwork-harness");
    drop(harness_fd);
    child
}

fn wait_for_success(child: &mut Child) {
    for _ in 0..1_000 {
        match child.try_wait().expect("poll child") {
            Some(status) if status.success() => return,
            Some(status) => {
                let stderr = read_child_stderr(child);
                panic!("harness exited with {status}: {stderr}");
            }
            None => unsafe {
                libc::usleep(10_000);
            },
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let stderr = read_child_stderr(child);
    panic!("harness did not exit after Shutdown: {stderr}");
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
