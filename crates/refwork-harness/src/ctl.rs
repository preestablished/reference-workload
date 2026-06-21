use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

use refwork_protocol::{CtlMsg, DecodeError, EncodeError, MAX_DATAGRAM};

pub const CONTROL_FD: RawFd = 3;

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
}

impl<T: DatagramTransport> ControlChannel<T> {
    pub fn recv_msg(&mut self) -> Result<CtlMsg, ControlError> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        let len = self.transport.recv_datagram(&mut buf)?;
        if len > MAX_DATAGRAM {
            return Err(ControlError::Oversize { len });
        }
        Ok(refwork_protocol::decode(&buf[..len])?)
    }

    pub fn send_msg(&mut self, msg: &CtlMsg) -> Result<(), ControlError> {
        let bytes = refwork_protocol::encode(msg)?;
        self.transport.send_datagram(&bytes)?;
        Ok(())
    }
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

    fn send_datagram(&mut self, bytes: &[u8]) -> io::Result<()> {
        loop {
            // Safety: `bytes` is a valid readable byte slice for its full length,
            // and `fd` is owned by this transport.
            let n =
                unsafe { libc::send(self.fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len(), 0) };
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
