#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

use refwork_emu::{Core, CoreError, FrameFlags};
use refwork_protocol::{CtlMsg, FaultCode};

use crate::ctl::{ControlChannel, ControlError, DatagramTransport};
use crate::meta::{MetaPage, META_SIZE};
use crate::regions::{ActiveEmuRegions, RegionError};
use crate::runner::SetupResult;

const MAX_FAULT_DETAIL_BYTES: usize = 512;

pub trait Platform {
    fn poll_input(&mut self, port: u8) -> u16;
    fn frame_mark(&mut self, frame: u64);
    fn quiesce_check(&mut self);
}

pub struct NoopPlatform;

impl Platform for NoopPlatform {
    fn poll_input(&mut self, _port: u8) -> u16 {
        0
    }

    fn frame_mark(&mut self, _frame: u64) {}

    fn quiesce_check(&mut self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameLoopExit {
    Shutdown { frame: u64 },
}

#[derive(Debug)]
pub enum FrameLoopError {
    Control(ControlError),
    Region(RegionError),
    Core(CoreError),
    BadProto { frame: u64, detail: String },
    EmuHalt { frame: u64, detail: String },
    ProtocolOrder { frame: u64, detail: String },
}

impl fmt::Display for FrameLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameLoopError::Control(err) => write!(f, "{err}"),
            FrameLoopError::Region(err) => write!(f, "region access failed: {err}"),
            FrameLoopError::Core(err) => write!(f, "core construction failed: {err:?}"),
            FrameLoopError::BadProto { frame, detail } => {
                write!(f, "bad protocol at frame {frame}: {detail}")
            }
            FrameLoopError::EmuHalt { frame, detail } => {
                write!(f, "emulator halted at frame {frame}: {detail}")
            }
            FrameLoopError::ProtocolOrder { frame, detail } => {
                write!(f, "protocol order fault at frame {frame}: {detail}")
            }
        }
    }
}

impl std::error::Error for FrameLoopError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FrameLoopError::Control(err) => Some(err),
            FrameLoopError::Region(err) => Some(err),
            FrameLoopError::Core(_) => None,
            FrameLoopError::BadProto { .. }
            | FrameLoopError::EmuHalt { .. }
            | FrameLoopError::ProtocolOrder { .. } => None,
        }
    }
}

impl From<ControlError> for FrameLoopError {
    fn from(err: ControlError) -> Self {
        FrameLoopError::Control(err)
    }
}

impl From<RegionError> for FrameLoopError {
    fn from(err: RegionError) -> Self {
        FrameLoopError::Region(err)
    }
}

impl From<CoreError> for FrameLoopError {
    fn from(err: CoreError) -> Self {
        FrameLoopError::Core(err)
    }
}

pub struct FrameLoop {
    core: Core,
    active: ActiveEmuRegions,
}

impl FrameLoop {
    pub fn new(setup: SetupResult) -> Result<Self, FrameLoopError> {
        // Safety: the setup phase consumed the region owner before any emulator
        // core exists. `FrameLoop` stores the active owner for the full lifetime
        // of the core and does not manufacture additional WRAM/VRAM/SRAM aliases.
        let mut active = unsafe { setup.regions.activate_for_emu() }?;
        let buffers = active.take_buffers();
        let core = Core::new(setup.game.cart, buffers)?;
        Ok(Self { core, active })
    }

    pub fn meta_bytes(&mut self) -> Result<&mut [u8; META_SIZE], FrameLoopError> {
        Ok(self.active.meta_mut()?)
    }

    pub fn run<T, P>(
        &mut self,
        channel: &mut ControlChannel<T>,
        platform: &mut P,
    ) -> Result<FrameLoopExit, FrameLoopError>
    where
        T: DatagramTransport,
        P: Platform,
    {
        loop {
            let pad = platform.poll_input(0) & 0x0fff;
            let flags = self.core.run_one_frame(pad);
            if flags.contains(FrameFlags::FAULTED) {
                let frame = self.core.frame_counter();
                return self.fault_emu(channel, frame, "core returned FAULTED");
            }

            self.core
                .blit_completed_frame(self.active.framebuffer_mut()?);
            let frame = self.core.frame_counter();
            MetaPage::from_existing(self.active.meta_mut()?).set_running_frame(frame, pad);
            platform.frame_mark(frame);
            platform.quiesce_check();

            match self.recv_boundary_msg(channel, frame)? {
                Some(BoundaryAction::Continue) | None => {}
                Some(BoundaryAction::Shutdown) => return Ok(FrameLoopExit::Shutdown { frame }),
            }
        }
    }

    fn recv_boundary_msg<T>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
    ) -> Result<Option<BoundaryAction>, FrameLoopError>
    where
        T: DatagramTransport,
    {
        let msg = match channel.try_recv_msg() {
            Ok(Some(msg)) => msg,
            Ok(None) => return Ok(None),
            Err(ControlError::Oversize { len }) => {
                let detail = format!("oversize control datagram: {len} bytes");
                return self.fault_bad_proto(channel, frame, &detail);
            }
            Err(ControlError::Decode(err)) => {
                let detail = err.to_string();
                return self.fault_bad_proto(channel, frame, &detail);
            }
            Err(err) => return Err(FrameLoopError::Control(err)),
        };

        match msg {
            CtlMsg::HashRequest { frame: requested } if requested == frame => {
                self.send_hash_report(channel, frame)?;
                Ok(Some(BoundaryAction::Continue))
            }
            CtlMsg::HashRequest { frame: requested } => {
                let detail =
                    format!("HashRequest frame {requested} != last completed frame {frame}");
                self.fault_protocol_order(channel, frame, &detail)
            }
            CtlMsg::Shutdown {} => Ok(Some(BoundaryAction::Shutdown)),
            other => {
                let detail = format!("unexpected steady-state message: {other:?}");
                self.fault_protocol_order(channel, frame, &detail)
            }
        }
    }

    fn send_hash_report<T>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
    ) -> Result<(), FrameLoopError>
    where
        T: DatagramTransport,
    {
        channel.send_msg(&CtlMsg::HashReport {
            frame,
            wram: blake3::hash(self.core.wram()).into(),
            fb: blake3::hash(self.active.framebuffer()?).into(),
        })?;
        Ok(())
    }

    fn fault_bad_proto<T, R>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
        detail: &str,
    ) -> Result<R, FrameLoopError>
    where
        T: DatagramTransport,
    {
        self.mark_meta_fault(frame, FaultCode::BadProto)?;
        self.send_fault(channel, frame, FaultCode::BadProto, detail)?;
        Err(FrameLoopError::BadProto {
            frame,
            detail: detail.into(),
        })
    }

    fn fault_emu<T, R>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
        detail: &str,
    ) -> Result<R, FrameLoopError>
    where
        T: DatagramTransport,
    {
        self.mark_meta_fault(frame, FaultCode::EmuHalt)?;
        self.send_fault(channel, frame, FaultCode::EmuHalt, detail)?;
        Err(FrameLoopError::EmuHalt {
            frame,
            detail: detail.into(),
        })
    }

    fn fault_protocol_order<T, R>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
        detail: &str,
    ) -> Result<R, FrameLoopError>
    where
        T: DatagramTransport,
    {
        self.mark_meta_fault(frame, FaultCode::ProtocolOrder)?;
        self.send_fault(channel, frame, FaultCode::ProtocolOrder, detail)?;
        Err(FrameLoopError::ProtocolOrder {
            frame,
            detail: detail.into(),
        })
    }

    fn mark_meta_fault(&mut self, frame: u64, code: FaultCode) -> Result<(), FrameLoopError> {
        MetaPage::from_existing(self.active.meta_mut()?).set_fault(frame, code);
        Ok(())
    }

    fn send_fault<T>(
        &mut self,
        channel: &mut ControlChannel<T>,
        frame: u64,
        code: FaultCode,
        detail: &str,
    ) -> Result<(), FrameLoopError>
    where
        T: DatagramTransport,
    {
        channel.send_msg(&CtlMsg::Fault {
            frame,
            code,
            detail: bounded_fault_detail(detail),
        })?;
        Ok(())
    }
}

pub fn run_frame_loop<T, P>(
    channel: &mut ControlChannel<T>,
    setup: SetupResult,
    platform: &mut P,
) -> Result<FrameLoopExit, FrameLoopError>
where
    T: DatagramTransport,
    P: Platform,
{
    let mut frame_loop = FrameLoop::new(setup)?;
    frame_loop.run(channel, platform)
}

enum BoundaryAction {
    Continue,
    Shutdown,
}

fn bounded_fault_detail(detail: &str) -> String {
    let mut end = detail.len().min(MAX_FAULT_DETAIL_BYTES);
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    if end < detail.len() {
        format!("{}...", &detail[..end])
    } else {
        detail.into()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io;

    use crate::game::loaded_game_from_rom;
    use crate::meta::{fault_code_value, MetaStatus};
    use crate::regions::HarnessRegions;

    use super::*;

    enum Inbound {
        Bytes(Vec<u8>),
        EmptyPoll,
    }

    struct ScriptTransport {
        inbound: VecDeque<Inbound>,
        sent: Vec<CtlMsg>,
    }

    impl ScriptTransport {
        fn new(inbound: Vec<Inbound>) -> Self {
            Self {
                inbound: inbound.into(),
                sent: Vec::new(),
            }
        }
    }

    impl DatagramTransport for ScriptTransport {
        fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.inbound.pop_front() {
                Some(Inbound::Bytes(bytes)) => {
                    let copied = bytes.len().min(buf.len());
                    buf[..copied].copy_from_slice(&bytes[..copied]);
                    Ok(copied)
                }
                Some(Inbound::EmptyPoll) => Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "scripted nonblocking empty poll on blocking recv",
                )),
                None => Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "no scripted datagram",
                )),
            }
        }

        fn try_recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
            match self.inbound.front() {
                Some(Inbound::EmptyPoll) => {
                    self.inbound.pop_front();
                    Ok(None)
                }
                Some(Inbound::Bytes(_)) => self.recv_datagram(buf).map(Some),
                None => Ok(None),
            }
        }

        fn send_datagram(&mut self, bytes: &[u8]) -> io::Result<()> {
            let msg = refwork_protocol::decode(bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            self.sent.push(msg);
            Ok(())
        }
    }

    #[derive(Default)]
    struct TestPlatform {
        pads: VecDeque<u16>,
        polls: Vec<u8>,
        marks: Vec<u64>,
        quiesce_checks: u64,
    }

    impl TestPlatform {
        fn with_pads(pads: &[u16]) -> Self {
            Self {
                pads: pads.iter().copied().collect(),
                polls: Vec::new(),
                marks: Vec::new(),
                quiesce_checks: 0,
            }
        }
    }

    impl Platform for TestPlatform {
        fn poll_input(&mut self, port: u8) -> u16 {
            self.polls.push(port);
            self.pads.pop_front().unwrap_or(0)
        }

        fn frame_mark(&mut self, frame: u64) {
            self.marks.push(frame);
        }

        fn quiesce_check(&mut self) {
            self.quiesce_checks += 1;
        }
    }

    fn wire(msg: CtlMsg) -> Vec<u8> {
        refwork_protocol::encode(&msg).expect("encode")
    }

    fn nop_rom() -> Vec<u8> {
        let mut rom = vec![0xeau8; 0x8000];
        rom[0x7ffc] = 0x00;
        rom[0x7ffd] = 0x80;
        rom
    }

    fn setup_result() -> SetupResult {
        let game = loaded_game_from_rom(nop_rom()).unwrap();
        let mut regions = HarnessRegions::required().unwrap();
        let meta_bytes: &mut [u8; META_SIZE] = regions
            .meta_mut()
            .as_mut_slice()
            .unwrap()
            .try_into()
            .unwrap();
        let mut meta = MetaPage::new(meta_bytes);
        meta.set_cart_hash(game.cart_hash);
        meta.set_emu_version("test-emu");
        meta.set_ready();
        SetupResult { game, regions }
    }

    #[test]
    fn shutdown_exits_at_frame_boundary_after_one_pad_and_mark() {
        let setup = setup_result();
        let mut frame_loop = FrameLoop::new(setup).unwrap();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![Inbound::Bytes(wire(
            CtlMsg::Shutdown {},
        ))]));
        let mut platform = TestPlatform::with_pads(&[0x0abc]);

        let exit = frame_loop.run(&mut channel, &mut platform).unwrap();

        assert_eq!(exit, FrameLoopExit::Shutdown { frame: 1 });
        assert_eq!(platform.polls, vec![0]);
        assert_eq!(platform.marks, vec![1]);
        assert_eq!(platform.quiesce_checks, 1);
        assert_eq!(
            u32_at(frame_loop.meta_bytes().unwrap(), 0x04),
            MetaStatus::Running as u32
        );
        assert_eq!(u64_at(frame_loop.meta_bytes().unwrap(), 0x08), 1);
        assert_eq!(u16_at(frame_loop.meta_bytes().unwrap(), 0x10), 0x0abc);
    }

    #[test]
    fn hash_request_reports_only_last_completed_frame() {
        let setup = setup_result();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![
            Inbound::Bytes(wire(CtlMsg::HashRequest { frame: 1 })),
            Inbound::Bytes(wire(CtlMsg::Shutdown {})),
        ]));
        let mut platform = TestPlatform::with_pads(&[1, 2]);

        let exit = run_frame_loop(&mut channel, setup, &mut platform).unwrap();

        assert_eq!(exit, FrameLoopExit::Shutdown { frame: 2 });
        assert!(matches!(
            channel.transport().sent.first(),
            Some(CtlMsg::HashReport { frame: 1, .. })
        ));
        assert_eq!(platform.marks, vec![1, 2]);
    }

    #[test]
    fn hash_request_for_future_frame_faults() {
        let setup = setup_result();
        let mut frame_loop = FrameLoop::new(setup).unwrap();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![Inbound::Bytes(wire(
            CtlMsg::HashRequest { frame: 2 },
        ))]));
        let mut platform = TestPlatform::with_pads(&[0]);

        let err = frame_loop.run(&mut channel, &mut platform).unwrap_err();

        assert!(matches!(
            err,
            FrameLoopError::ProtocolOrder { frame: 1, .. }
        ));
        assert_fault(&channel.transport().sent, FaultCode::ProtocolOrder, 1);
        assert_eq!(
            u32_at(frame_loop.meta_bytes().unwrap(), 0x04),
            MetaStatus::Faulted as u32
        );
        assert_eq!(
            u32_at(frame_loop.meta_bytes().unwrap(), 0x14),
            fault_code_value(FaultCode::ProtocolOrder)
        );
    }

    #[test]
    fn hash_request_for_stale_frame_faults() {
        let setup = setup_result();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![
            Inbound::EmptyPoll,
            Inbound::Bytes(wire(CtlMsg::HashRequest { frame: 1 })),
        ]));
        let mut platform = TestPlatform::with_pads(&[0, 0]);

        let err = run_frame_loop(&mut channel, setup, &mut platform).unwrap_err();

        assert!(matches!(
            err,
            FrameLoopError::ProtocolOrder { frame: 2, .. }
        ));
        assert_fault(&channel.transport().sent, FaultCode::ProtocolOrder, 2);
    }

    #[test]
    fn unexpected_steady_state_message_faults() {
        let setup = setup_result();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![Inbound::Bytes(wire(
            CtlMsg::Start {},
        ))]));
        let mut platform = TestPlatform::with_pads(&[0]);

        let err = run_frame_loop(&mut channel, setup, &mut platform).unwrap_err();

        assert!(matches!(
            err,
            FrameLoopError::ProtocolOrder { frame: 1, .. }
        ));
        assert_fault(&channel.transport().sent, FaultCode::ProtocolOrder, 1);
    }

    #[test]
    fn no_control_message_continues_to_next_frame() {
        let setup = setup_result();
        let mut channel = ControlChannel::new(ScriptTransport::new(vec![
            Inbound::EmptyPoll,
            Inbound::Bytes(wire(CtlMsg::Shutdown {})),
        ]));
        let mut platform = TestPlatform::with_pads(&[0x0001, 0x0002]);

        let exit = run_frame_loop(&mut channel, setup, &mut platform).unwrap();

        assert_eq!(exit, FrameLoopExit::Shutdown { frame: 2 });
        assert_eq!(platform.polls, vec![0, 0]);
        assert_eq!(platform.marks, vec![1, 2]);
        assert_eq!(platform.quiesce_checks, 2);
        assert!(channel.transport().sent.is_empty());
    }

    fn assert_fault(msgs: &[CtlMsg], code: FaultCode, frame: u64) {
        assert!(
            msgs.iter().any(|msg| {
                matches!(
                    msg,
                    CtlMsg::Fault {
                        frame: actual_frame,
                        code: actual_code,
                        ..
                    } if *actual_frame == frame && *actual_code == code
                )
            }),
            "missing Fault({code:?}) at frame {frame} in {msgs:?}"
        );
    }

    fn u16_at(bytes: &[u8], off: usize) -> u16 {
        u16::from_le_bytes(bytes[off..off + 2].try_into().unwrap())
    }

    fn u32_at(bytes: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap())
    }

    fn u64_at(bytes: &[u8], off: usize) -> u64 {
        u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap())
    }
}
