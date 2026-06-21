#![forbid(unsafe_code)]

use std::fmt;

use refwork_emu::EMU_VERSION;
use refwork_protocol::{CtlMsg, FaultCode, PROTO_VERSION};

use crate::ctl::{ControlChannel, ControlError, DatagramTransport};
use crate::game::{GameLoadError, GameLoader, LoadedGame};
use crate::meta::{MetaPage, META_SIZE};
use crate::regions::{HarnessRegions, RegionError};

pub struct SetupConfig {
    pub emu_name: &'static str,
    pub emu_version: &'static str,
    pub vram: bool,
    pub sram_len: Option<usize>,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            emu_name: "refwork-emu",
            emu_version: EMU_VERSION,
            vram: false,
            sram_len: None,
        }
    }
}

pub struct SetupResult {
    pub game: LoadedGame,
    pub regions: HarnessRegions,
}

#[derive(Debug)]
pub enum SetupError {
    Control(ControlError),
    BadProto {
        detail: String,
    },
    BadGame(GameLoadError),
    Region(RegionError),
    ProtocolOrder {
        expected: &'static str,
        actual: CtlMsg,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::Control(err) => write!(f, "{err}"),
            SetupError::BadProto { detail } => write!(f, "bad protocol: {detail}"),
            SetupError::BadGame(err) => write!(f, "bad game: {err}"),
            SetupError::Region(err) => write!(f, "region preparation failed: {err}"),
            SetupError::ProtocolOrder { expected, actual } => {
                write!(f, "expected {expected}, got {actual:?}")
            }
        }
    }
}

impl std::error::Error for SetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SetupError::Control(err) => Some(err),
            SetupError::BadProto { .. } => None,
            SetupError::BadGame(err) => Some(err),
            SetupError::Region(err) => Some(err),
            SetupError::ProtocolOrder { .. } => None,
        }
    }
}

impl From<ControlError> for SetupError {
    fn from(err: ControlError) -> Self {
        SetupError::Control(err)
    }
}

pub fn run_setup<T, L>(
    channel: &mut ControlChannel<T>,
    loader: &mut L,
    config: SetupConfig,
) -> Result<SetupResult, SetupError>
where
    T: DatagramTransport,
    L: GameLoader,
{
    expect_hello(channel, &config)?;
    let dev_path = expect_load_game(channel)?;
    let game = load_game_or_fault(channel, loader, &dev_path)?;
    let mut regions = prepare_regions_or_fault(channel, &game, &config)?;

    send_game_loaded(channel, &game)?;
    send_regions(channel, &regions)?;
    channel.send_msg(&CtlMsg::Ready { frame: 0 })?;

    expect_start(channel, &mut regions)?;
    Ok(SetupResult { game, regions })
}

fn expect_hello<T>(channel: &mut ControlChannel<T>, config: &SetupConfig) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    match recv_agent_msg(channel)? {
        CtlMsg::Hello { proto_version } if proto_version == PROTO_VERSION => {
            channel.send_msg(&crate::hello_ack(config.emu_name, config.emu_version))?;
            Ok(())
        }
        CtlMsg::Hello { proto_version } => {
            let detail = format!("protocol version {proto_version} != {PROTO_VERSION}");
            send_fault(channel, FaultCode::BadProto, &detail)?;
            Err(SetupError::BadProto { detail })
        }
        actual => protocol_order(channel, "Hello", actual),
    }
}

fn expect_load_game<T>(channel: &mut ControlChannel<T>) -> Result<String, SetupError>
where
    T: DatagramTransport,
{
    match recv_agent_msg(channel)? {
        CtlMsg::LoadGame { dev_path } => Ok(dev_path),
        actual => protocol_order(channel, "LoadGame", actual),
    }
}

fn expect_start<T>(
    channel: &mut ControlChannel<T>,
    regions: &mut HarnessRegions,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    match recv_agent_msg(channel)? {
        CtlMsg::Start {} => Ok(()),
        actual => {
            mark_meta_fault(regions, FaultCode::ProtocolOrder);
            protocol_order(channel, "Start", actual)
        }
    }
}

fn recv_agent_msg<T>(channel: &mut ControlChannel<T>) -> Result<CtlMsg, SetupError>
where
    T: DatagramTransport,
{
    match channel.recv_msg() {
        Ok(msg) => Ok(msg),
        Err(ControlError::Oversize { len }) => {
            let detail = format!("oversize control datagram: {len} bytes");
            send_fault(channel, FaultCode::BadProto, &detail)?;
            Err(SetupError::BadProto { detail })
        }
        Err(ControlError::Decode(err)) => {
            let detail = err.to_string();
            send_fault(channel, FaultCode::BadProto, &detail)?;
            Err(SetupError::BadProto { detail })
        }
        Err(err) => Err(SetupError::Control(err)),
    }
}

fn load_game_or_fault<T, L>(
    channel: &mut ControlChannel<T>,
    loader: &mut L,
    dev_path: &str,
) -> Result<LoadedGame, SetupError>
where
    T: DatagramTransport,
    L: GameLoader,
{
    match loader.load_game(dev_path) {
        Ok(game) => Ok(game),
        Err(err) => {
            let detail = err.to_string();
            send_fault(channel, FaultCode::BadGame, &detail)?;
            Err(SetupError::BadGame(err))
        }
    }
}

fn prepare_regions_or_fault<T>(
    channel: &mut ControlChannel<T>,
    game: &LoadedGame,
    config: &SetupConfig,
) -> Result<HarnessRegions, SetupError>
where
    T: DatagramTransport,
{
    let result =
        HarnessRegions::with_optional(config.vram, config.sram_len).and_then(|mut regions| {
            init_meta(&mut regions, game.cart_hash, config.emu_version)?;
            Ok(regions)
        });

    match result {
        Ok(regions) => Ok(regions),
        Err(err) => {
            let detail = err.to_string();
            send_fault(channel, FaultCode::RegionRegFailed, &detail)?;
            Err(SetupError::Region(err))
        }
    }
}

fn init_meta(
    regions: &mut HarnessRegions,
    cart_hash: [u8; 32],
    emu_version: &str,
) -> Result<(), RegionError> {
    let bytes = regions.meta_mut().as_mut_slice()?;
    let len = bytes.len();
    let meta_bytes: &mut [u8; META_SIZE] =
        bytes.try_into().map_err(|_| RegionError::WrongSize {
            name: "meta",
            expected: META_SIZE,
            actual: len,
        })?;
    let mut meta = MetaPage::new(meta_bytes);
    meta.set_cart_hash(cart_hash);
    meta.set_emu_version(emu_version);
    meta.set_ready();
    Ok(())
}

fn mark_meta_fault(regions: &mut HarnessRegions, code: FaultCode) {
    if let Ok(bytes) = regions.meta_mut().as_mut_slice() {
        if let Ok(meta_bytes) = <&mut [u8; META_SIZE]>::try_from(bytes) {
            MetaPage::from_existing(meta_bytes).set_fault(0, code);
        }
    }
}

fn send_game_loaded<T>(channel: &mut ControlChannel<T>, game: &LoadedGame) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    channel.send_msg(&CtlMsg::GameLoaded {
        cart_hash: game.cart_hash,
        mapper: game.mapper.clone(),
        sram_size: game.sram_size,
    })?;
    Ok(())
}

fn send_regions<T>(
    channel: &mut ControlChannel<T>,
    regions: &HarnessRegions,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    for descriptor in regions.descriptors() {
        channel.send_msg(&CtlMsg::RegisterRegion {
            name: descriptor.name.into(),
            gva: descriptor.gva,
            len: descriptor.len,
            writable: descriptor.writable,
        })?;
    }
    Ok(())
}

fn protocol_order<T, R>(
    channel: &mut ControlChannel<T>,
    expected: &'static str,
    actual: CtlMsg,
) -> Result<R, SetupError>
where
    T: DatagramTransport,
{
    let detail = format!("expected {expected}, got {actual:?}");
    send_fault(channel, FaultCode::ProtocolOrder, &detail)?;
    Err(SetupError::ProtocolOrder { expected, actual })
}

fn send_fault<T>(
    channel: &mut ControlChannel<T>,
    code: FaultCode,
    detail: &str,
) -> Result<(), SetupError>
where
    T: DatagramTransport,
{
    channel.send_msg(&CtlMsg::Fault {
        frame: 0,
        code,
        detail: detail.into(),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io;

    use refwork_emu::CoreError;
    use refwork_protocol::MAX_DATAGRAM;

    use super::*;
    use crate::ctl::DatagramTransport;
    use crate::game::loaded_game_from_rom;

    struct ScriptTransport {
        inbound: VecDeque<Vec<u8>>,
        sent: Vec<CtlMsg>,
    }

    impl ScriptTransport {
        fn new(messages: Vec<Vec<u8>>) -> Self {
            Self {
                inbound: messages.into(),
                sent: Vec::new(),
            }
        }
    }

    impl DatagramTransport for ScriptTransport {
        fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let bytes = self.inbound.pop_front().ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "no scripted datagram")
            })?;
            let copied = bytes.len().min(buf.len());
            buf[..copied].copy_from_slice(&bytes[..copied]);
            Ok(copied)
        }

        fn send_datagram(&mut self, bytes: &[u8]) -> io::Result<()> {
            let msg = refwork_protocol::decode(bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            self.sent.push(msg);
            Ok(())
        }
    }

    struct FakeLoader {
        rom: Option<Vec<u8>>,
        fail: bool,
        paths: Vec<String>,
    }

    impl FakeLoader {
        fn ok() -> Self {
            Self {
                rom: Some(valid_rom()),
                fail: false,
                paths: Vec::new(),
            }
        }

        fn fail() -> Self {
            Self {
                rom: None,
                fail: true,
                paths: Vec::new(),
            }
        }
    }

    impl GameLoader for FakeLoader {
        fn load_game(&mut self, dev_path: &str) -> Result<LoadedGame, GameLoadError> {
            self.paths.push(dev_path.into());
            if self.fail {
                return Err(GameLoadError::Cart(CoreError::BadRomSize { len: 0 }));
            }
            loaded_game_from_rom(self.rom.take().expect("single fake ROM"))
        }
    }

    fn valid_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x7FFC] = 0x00;
        rom[0x7FFD] = 0x80;
        rom
    }

    fn wire(msg: CtlMsg) -> Vec<u8> {
        refwork_protocol::encode(&msg).expect("encode scripted message")
    }

    fn run_with(
        inbound: Vec<Vec<u8>>,
        loader: &mut FakeLoader,
        config: SetupConfig,
    ) -> (Result<SetupResult, SetupError>, Vec<CtlMsg>) {
        let transport = ScriptTransport::new(inbound);
        let mut channel = ControlChannel::new(transport);
        let result = run_setup(&mut channel, loader, config);
        let sent = channel.transport().sent.clone();
        (result, sent)
    }

    #[test]
    fn happy_setup_ordering_emits_game_regions_ready_after_load() {
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(
            vec![
                wire(CtlMsg::Hello {
                    proto_version: PROTO_VERSION,
                }),
                wire(CtlMsg::LoadGame {
                    dev_path: "/dev/vdb".into(),
                }),
                wire(CtlMsg::Start {}),
            ],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(result.is_ok());
        assert_eq!(loader.paths, vec!["/dev/vdb"]);
        assert!(matches!(sent[0], CtlMsg::HelloAck { .. }));
        assert!(matches!(sent[1], CtlMsg::GameLoaded { .. }));
        assert_region(&sent[2], "wram");
        assert_region(&sent[3], "framebuffer");
        assert_region(&sent[4], "meta");
        assert_eq!(sent[5], CtlMsg::Ready { frame: 0 });
        assert_eq!(
            sent.len(),
            6,
            "setup must not emit per-frame control traffic"
        );
    }

    #[test]
    fn version_mismatch_faults_bad_proto() {
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(
            vec![wire(CtlMsg::Hello {
                proto_version: PROTO_VERSION + 1,
            })],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(matches!(result, Err(SetupError::BadProto { .. })));
        assert_fault(&sent, FaultCode::BadProto);
        assert!(loader.paths.is_empty());
    }

    #[test]
    fn malformed_datagram_faults_bad_proto() {
        let mut hello = wire(CtlMsg::Hello {
            proto_version: PROTO_VERSION,
        });
        hello.push(0);
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(vec![hello], &mut loader, SetupConfig::default());

        assert!(matches!(result, Err(SetupError::BadProto { .. })));
        assert_fault(&sent, FaultCode::BadProto);
    }

    #[test]
    fn oversize_datagram_faults_bad_proto() {
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(
            vec![vec![0u8; MAX_DATAGRAM + 2]],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(matches!(result, Err(SetupError::BadProto { .. })));
        assert_fault(&sent, FaultCode::BadProto);
    }

    #[test]
    fn out_of_order_first_message_faults_protocol_order() {
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(
            vec![wire(CtlMsg::LoadGame {
                dev_path: "/dev/vdb".into(),
            })],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(matches!(result, Err(SetupError::ProtocolOrder { .. })));
        assert_fault(&sent, FaultCode::ProtocolOrder);
        assert!(loader.paths.is_empty());
    }

    #[test]
    fn bad_game_faults_bad_game_after_hello_ack() {
        let mut loader = FakeLoader::fail();
        let (result, sent) = run_with(
            vec![
                wire(CtlMsg::Hello {
                    proto_version: PROTO_VERSION,
                }),
                wire(CtlMsg::LoadGame {
                    dev_path: "/dev/vdb".into(),
                }),
            ],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(matches!(result, Err(SetupError::BadGame(_))));
        assert!(matches!(sent[0], CtlMsg::HelloAck { .. }));
        assert_fault(&sent[1..], FaultCode::BadGame);
    }

    #[test]
    fn region_preparation_failure_faults_region_reg_failed() {
        let mut loader = FakeLoader::ok();
        let config = SetupConfig {
            sram_len: Some(1024),
            ..SetupConfig::default()
        };
        let (result, sent) = run_with(
            vec![
                wire(CtlMsg::Hello {
                    proto_version: PROTO_VERSION,
                }),
                wire(CtlMsg::LoadGame {
                    dev_path: "/dev/vdb".into(),
                }),
            ],
            &mut loader,
            config,
        );

        assert!(matches!(result, Err(SetupError::Region(_))));
        assert!(matches!(sent[0], CtlMsg::HelloAck { .. }));
        assert_fault(&sent[1..], FaultCode::RegionRegFailed);
    }

    #[test]
    fn out_of_order_start_faults_after_ready() {
        let mut loader = FakeLoader::ok();
        let (result, sent) = run_with(
            vec![
                wire(CtlMsg::Hello {
                    proto_version: PROTO_VERSION,
                }),
                wire(CtlMsg::LoadGame {
                    dev_path: "/dev/vdb".into(),
                }),
                wire(CtlMsg::Shutdown {}),
            ],
            &mut loader,
            SetupConfig::default(),
        );

        assert!(matches!(result, Err(SetupError::ProtocolOrder { .. })));
        assert_eq!(sent[5], CtlMsg::Ready { frame: 0 });
        assert_fault(&sent[6..], FaultCode::ProtocolOrder);
    }

    fn assert_region(msg: &CtlMsg, name: &str) {
        match msg {
            CtlMsg::RegisterRegion {
                name: actual,
                gva,
                len,
                ..
            } => {
                assert_eq!(actual, name);
                assert_ne!(*gva, 0);
                assert_ne!(*len, 0);
            }
            other => panic!("expected RegisterRegion({name}), got {other:?}"),
        }
    }

    fn assert_fault(msgs: &[CtlMsg], code: FaultCode) {
        assert!(
            msgs.iter()
                .any(|msg| matches!(msg, CtlMsg::Fault { code: actual, .. } if *actual == code)),
            "missing Fault({code:?}) in {msgs:?}"
        );
    }
}
