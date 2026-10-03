//! `LoopbackAddr`: a socket address that can only point at this machine (ADR 0001 rule 2).

use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

/// A socket address in `127.0.0.0/8` or `::1`. No other address can be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopbackAddr(SocketAddr);

/// Why an address was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddrError {
    /// Not `host:port` with an IP literal.
    Syntax(String),
    /// A valid address that is not loopback.
    NotLoopback(String),
}

impl fmt::Display for AddrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "not an IP address and port: {s}"),
            Self::NotLoopback(s) => write!(f, "not a loopback address: {s}"),
        }
    }
}

impl LoopbackAddr {
    /// Parses `127.0.0.1:4444`, `[::1]:4444` or `http://127.0.0.1:4444[/]`.
    ///
    /// # Errors
    ///
    /// Fails for anything that is not an IP literal and port, or not loopback.
    pub fn parse(text: &str) -> Result<Self, AddrError> {
        let trimmed = text.trim();
        let bare = trimmed.strip_prefix("http://").unwrap_or(trimmed);
        let bare = bare.strip_suffix('/').unwrap_or(bare);
        let addr: SocketAddr = bare
            .parse()
            .map_err(|_| AddrError::Syntax(text.to_owned()))?;
        Self::new(addr)
    }

    /// Wraps `addr` when it is loopback.
    ///
    /// # Errors
    ///
    /// Fails when `addr` is not loopback.
    pub fn new(addr: SocketAddr) -> Result<Self, AddrError> {
        if addr.ip().is_loopback() {
            Ok(Self(addr))
        } else {
            Err(AddrError::NotLoopback(addr.to_string()))
        }
    }

    /// A free port on `127.0.0.1`, bound and listening.
    ///
    /// # Errors
    ///
    /// Fails when the OS refuses the bind.
    pub fn listen_any() -> io::Result<(TcpListener, Self)> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        Ok((listener, Self(addr)))
    }

    /// Opens a TCP connection with read and write timeouts.
    ///
    /// # Errors
    ///
    /// Fails when the connection or the timeouts fail.
    pub fn connect(&self, timeout: Duration) -> io::Result<TcpStream> {
        let stream = TcpStream::connect_timeout(&self.0, timeout)?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        Ok(stream)
    }

    /// `http://host:port`, for engine proxy settings.
    #[must_use]
    pub fn http_url(&self) -> String {
        format!("http://{}", self.0)
    }
}

impl fmt::Display for LoopbackAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_only() {
        assert!(LoopbackAddr::parse("127.0.0.1:4444").is_ok());
        assert!(LoopbackAddr::parse("127.8.9.10:1").is_ok());
        assert!(LoopbackAddr::parse("[::1]:4444").is_ok());
        assert!(LoopbackAddr::parse(" http://127.0.0.1:4444/ ").is_ok());
    }

    #[test]
    fn refuses_everything_else() {
        let lan = LoopbackAddr::parse("192.168.1.2:4444").unwrap_err();
        assert_eq!(lan, AddrError::NotLoopback("192.168.1.2:4444".into()));
        assert!(lan.to_string().contains("loopback"));
        let name = LoopbackAddr::parse("localhost:4444").unwrap_err();
        assert!(matches!(name, AddrError::Syntax(_)));
        assert!(name.to_string().contains("IP address"));
        assert!(LoopbackAddr::parse("0.0.0.0:4444").is_err());
        assert!(LoopbackAddr::parse("127.0.0.1").is_err());
    }

    #[test]
    fn display_and_url() {
        let addr = LoopbackAddr::parse("127.0.0.1:4444").unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:4444");
        assert_eq!(addr.http_url(), "http://127.0.0.1:4444");
    }

    #[test]
    fn listen_and_connect() {
        let (listener, addr) = LoopbackAddr::listen_any().unwrap();
        let stream = addr.connect(Duration::from_secs(2)).unwrap();
        assert!(listener.accept().is_ok());
        drop(stream);
    }
}
