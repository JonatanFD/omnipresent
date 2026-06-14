//! Network addressing value object.

use std::net::SocketAddr;

/// A peer's network address — the far end of a QUIC connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Endpoint(SocketAddr);

impl Endpoint {
    /// Wraps a socket address.
    pub const fn new(addr: SocketAddr) -> Self {
        Self(addr)
    }

    /// The underlying socket address.
    pub const fn addr(&self) -> SocketAddr {
        self.0
    }
}

impl From<SocketAddr> for Endpoint {
    fn from(addr: SocketAddr) -> Self {
        Self(addr)
    }
}
