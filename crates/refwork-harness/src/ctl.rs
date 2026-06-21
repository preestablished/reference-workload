use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use refwork_protocol::{CtlMsg, DecodeError, EncodeError, MAX_DATAGRAM};

pub const CONTROL_FD: RawFd = 3;

#[cfg(target_os = "linux")]
const SEND_FLAGS: libc::c_int = libc::MSG_NOSIGNAL;
#[cfg(not(target_os = "linux"))]
const SEND_FLAGS: libc::c_int = 0;

#[derive(Debug)]
pub enum ControlError {
    Io(io::Error),
    Oversize { len: usize },
    Decode(DecodeError),
    Encode(EncodeError),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::Io(err) => write!(f, "control I/O error: {err}"),
            ControlError::Oversize { len } => {
                write!(f, "control datagram length {len} exceeds {MAX_DATAGRAM}")
            }
            ControlError::Decode(err) => write!(f, "control decode error: {err}"),
            ControlError::Encode(err) => write!(f, "control encode error: {err}"),
        }
    }
}

impl std::error::Error for ControlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ControlError::Io(err) => Some(err),
            ControlError::Oversize { .. } => None,
            ControlError::Decode(err) => Some(err),
            ControlError::Encode(err) => Some(err),
        }
    }
}

impl From<io::Error> for ControlError {
    fn from(err: io::Error) -> Self {
        ControlError::Io(err)
    }
}

impl From<DecodeError> for ControlError {
    fn from(err: DecodeError) -> Self {
        ControlError::Decode(err)
    }
}

impl From<EncodeError> for ControlError {
    fn from(err: EncodeError) -> Self {
        ControlError::Encode(err)
    }
}

pub trait DatagramTransport {
    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn try_recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>>;
    fn send_datagram(&mut self, bytes: &[u8]) -> io::Result<()>;
}

pub struct ControlChannel<T> {
    transport: T,
}

impl<T> ControlChannel<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
}

impl<T: DatagramTransport> ControlChannel<T> {
    pub fn recv_msg(&mut self) -> Result<CtlMsg, ControlError> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        let len = self.transport.recv_datagram(&mut buf)?;
        decode_datagram(&buf[..len.min(buf.len())], len)
    }

    pub fn try_recv_msg(&mut self) -> Result<Option<CtlMsg>, ControlError> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        match self.transport.try_recv_datagram(&mut buf)? {
            Some(len) => decode_datagram(&buf[..len.min(buf.len())], len).map(Some),
            None => Ok(None),
        }
    }

    pub fn send_msg(&mut self, msg: &CtlMsg) -> Result<(), ControlError> {
        let bytes = refwork_protocol::encode(msg)?;
        self.transport.send_datagram(&bytes)?;
        Ok(())
    }
}

fn decode_datagram(bytes: &[u8], len: usize) -> Result<CtlMsg, ControlError> {
    if len > MAX_DATAGRAM {
        return Err(ControlError::Oversize { len });
    }
    Ok(refwork_protocol::decode(bytes)?)
}

pub struct SeqpacketFd {
    fd: OwnedFd,
}

impl SeqpacketFd {
    pub fn from_inherited_control_fd() -> io::Result<Self> {
        // Safety: fcntl(F_GETFD) only inspects fd metadata and does not take
        // ownership.
        let rc = unsafe { libc::fcntl(CONTROL_FD, libc::F_GETFD) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        validate_seqpacket(CONTROL_FD)?;

        // Safety: the harness process owns inherited control fd 3 in production
        // mode. Wrapping it in OwnedFd closes it exactly once on process exit or
        // setup failure.
        let fd = unsafe { OwnedFd::from_raw_fd(CONTROL_FD) };
        Ok(Self { fd })
    }

    #[cfg(test)]
    fn from_owned_fd(fd: OwnedFd) -> Self {
        Self { fd }
    }
}

fn validate_seqpacket(fd: RawFd) -> io::Result<()> {
    let mut socket_type: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&socket_type) as libc::socklen_t;
    // Safety: pointers refer to valid storage for the socket type and length.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut socket_type as *mut libc::c_int).cast(),
            &mut len,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    if socket_type != libc::SOCK_SEQPACKET {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("fd 3 is socket type {socket_type}, expected SOCK_SEQPACKET"),
        ));
    }
    validate_domain(fd)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_domain(fd: RawFd) -> io::Result<()> {
    let mut domain: libc::c_int = 0;
    let mut len = std::mem::size_of_val(&domain) as libc::socklen_t;
    // Safety: pointers refer to valid storage for the socket domain and length.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_DOMAIN,
            (&mut domain as *mut libc::c_int).cast(),
            &mut len,
        )
    };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    if domain != libc::AF_UNIX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("fd 3 socket domain {domain}, expected AF_UNIX"),
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn validate_domain(_fd: RawFd) -> io::Result<()> {
    Ok(())
}

impl DatagramTransport for SeqpacketFd {
    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            // Safety: `buf` is a valid writable byte slice for its full length,
            // and `fd` is owned by this transport.
            let n =
                unsafe { libc::recv(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
            if n >= 0 {
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "control socket closed",
                    ));
                }
                return Ok(n as usize);
            }

            let err = io::Error::last_os_error();
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
    }

    fn try_recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        loop {
            // Safety: `buf` is a valid writable byte slice for its full length,
            // and `fd` is owned by this transport.
            let n = unsafe {
                libc::recv(
                    self.fd.as_raw_fd(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if n >= 0 {
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "control socket closed",
                    ));
                }
                return Ok(Some(n as usize));
            }

            let err = io::Error::last_os_error();
            match err.kind() {
                io::ErrorKind::Interrupted => {}
                io::ErrorKind::WouldBlock => return Ok(None),
                _ => return Err(err),
            }
        }
    }

    fn send_datagram(&mut self, bytes: &[u8]) -> io::Result<()> {
        loop {
            // Safety: `bytes` is a valid readable byte slice for its full length,
            // and `fd` is owned by this transport.
            let n = unsafe {
                libc::send(
                    self.fd.as_raw_fd(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    SEND_FLAGS,
                )
            };
            if n >= 0 {
                let sent = n as usize;
                if sent == bytes.len() {
                    return Ok(());
                }
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!("short control datagram send: {sent}/{}", bytes.len()),
                ));
            }

            let err = io::Error::last_os_error();
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::FromRawFd;

    use refwork_protocol::{CtlMsg, FaultCode, PROTO_VERSION};

    use super::*;

    fn seqpacket_pair() -> (SeqpacketFd, OwnedFd) {
        let mut fds = [-1; 2];
        // Safety: `fds` points to two valid c_int slots filled by socketpair on
        // success.
        let rc =
            unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_SEQPACKET, 0, fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "socketpair failed: {}", io::Error::last_os_error());

        // Safety: each raw fd is newly returned by socketpair and becomes owned
        // exactly once here.
        let left = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        let right = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        (SeqpacketFd::from_owned_fd(left), right)
    }

    #[test]
    fn seqpacket_transport_preserves_message_boundaries() {
        let (transport, mut peer) = seqpacket_pair();
        let mut channel = ControlChannel::new(transport);
        let first = refwork_protocol::encode(&CtlMsg::Hello {
            proto_version: PROTO_VERSION,
        })
        .unwrap();
        let second = refwork_protocol::encode(&CtlMsg::Start {}).unwrap();

        send_raw(&mut peer, &first);
        send_raw(&mut peer, &second);

        assert_eq!(
            channel.recv_msg().unwrap(),
            CtlMsg::Hello {
                proto_version: PROTO_VERSION
            }
        );
        assert_eq!(channel.recv_msg().unwrap(), CtlMsg::Start {});
    }

    #[test]
    fn seqpacket_transport_reports_oversize_datagram() {
        let (transport, mut peer) = seqpacket_pair();
        let mut channel = ControlChannel::new(transport);

        send_raw(&mut peer, &[0u8; MAX_DATAGRAM + 8]);

        assert!(matches!(
            channel.recv_msg(),
            Err(ControlError::Oversize { len }) if len == MAX_DATAGRAM + 1
        ));
    }

    #[test]
    fn seqpacket_try_recv_reports_empty_then_message() {
        let (transport, mut peer) = seqpacket_pair();
        let mut channel = ControlChannel::new(transport);

        assert_eq!(channel.try_recv_msg().unwrap(), None);

        let msg = refwork_protocol::encode(&CtlMsg::Shutdown {}).unwrap();
        send_raw(&mut peer, &msg);

        assert_eq!(channel.try_recv_msg().unwrap(), Some(CtlMsg::Shutdown {}));
    }

    #[test]
    fn closed_peer_send_returns_error_instead_of_sigpipe() {
        let (transport, peer) = seqpacket_pair();
        drop(peer);
        let mut channel = ControlChannel::new(transport);

        let err = channel
            .send_msg(&CtlMsg::Fault {
                frame: 0,
                code: FaultCode::BadProto,
                detail: "closed peer".into(),
            })
            .expect_err("closed peer must be reported");

        assert!(matches!(err, ControlError::Io(io) if io.raw_os_error() == Some(libc::EPIPE)));
    }

    fn send_raw(fd: &mut OwnedFd, bytes: &[u8]) {
        // Safety: `bytes` is readable for its full length and `fd` is live.
        let n = unsafe {
            libc::send(
                fd.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                SEND_FLAGS,
            )
        };
        assert_eq!(
            n,
            bytes.len() as isize,
            "send failed: {}",
            io::Error::last_os_error()
        );
    }
}
