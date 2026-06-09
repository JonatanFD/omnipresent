//! The `SecureChannel` port: an established, authenticated connection to one
//! peer over which datagrams are exchanged.
//!
//! With QUIC, confidentiality, integrity, and replay protection all live inside
//! the connection (TLS 1.3 + QUIC packet protection), so a channel deals only in
//! already-protected datagram payloads — there is no separate "encrypt" step at
//! this layer. The real adapter wraps [`quinn`](https://docs.rs/quinn) and sends
//! unreliable QUIC datagrams (RFC 9221); this module ships a loopback adapter for
//! tests.

use crate::endpoint::Endpoint;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::rc::Rc;

/// An established secure connection to a single peer. Datagrams are unreliable:
/// a lost one is not retransmitted, which is what keeps input latency low.
pub trait SecureChannel {
    /// What can go wrong sending or receiving on the connection.
    type Error;

    /// The peer at the far end.
    fn peer(&self) -> Endpoint;

    /// Sends one datagram. The payload is protected by the QUIC/TLS layer before
    /// it leaves the machine.
    fn send_datagram(&mut self, payload: &[u8]) -> Result<(), Self::Error>;

    /// Returns the next received datagram payload, or `None` if none is ready.
    /// Non-blocking, so the event loop can poll it.
    fn recv_datagram(&mut self) -> Result<Option<Vec<u8>>, Self::Error>;
}

/// A [`SecureChannel`] backed by in-memory queues, standing in for a real QUIC
/// connection in tests. It moves bytes only — it trusts that the real connection
/// provides the cryptography, exactly as the production code will.
#[derive(Debug)]
pub struct LoopbackChannel {
    peer: Endpoint,
    inbox: Rc<RefCell<VecDeque<Vec<u8>>>>,
    outbox: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl LoopbackChannel {
    /// Creates two cross-wired channels: whatever one sends, the other receives.
    /// Each channel's `peer` is the address of the opposite end.
    pub fn pair(a: Endpoint, b: Endpoint) -> (LoopbackChannel, LoopbackChannel) {
        let a_inbox = Rc::new(RefCell::new(VecDeque::new()));
        let b_inbox = Rc::new(RefCell::new(VecDeque::new()));
        let side_a = LoopbackChannel {
            peer: b,
            inbox: a_inbox.clone(),
            outbox: b_inbox.clone(),
        };
        let side_b = LoopbackChannel {
            peer: a,
            inbox: b_inbox,
            outbox: a_inbox,
        };
        (side_a, side_b)
    }
}

impl SecureChannel for LoopbackChannel {
    type Error = Infallible;

    fn peer(&self) -> Endpoint {
        self.peer
    }

    fn send_datagram(&mut self, payload: &[u8]) -> Result<(), Self::Error> {
        self.outbox.borrow_mut().push_back(payload.to_vec());
        Ok(())
    }

    fn recv_datagram(&mut self) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.inbox.borrow_mut().pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoints() -> (Endpoint, Endpoint) {
        (
            Endpoint::new("127.0.0.1:7001".parse().unwrap()),
            Endpoint::new("127.0.0.1:7002".parse().unwrap()),
        )
    }

    #[test]
    fn each_side_knows_the_other_as_its_peer() {
        let (a_addr, b_addr) = endpoints();
        let (side_a, side_b) = LoopbackChannel::pair(a_addr, b_addr);

        assert_eq!(side_a.peer(), b_addr);
        assert_eq!(side_b.peer(), a_addr);
    }

    #[test]
    fn datagrams_sent_by_one_side_arrive_at_the_other_in_order() {
        let (a_addr, b_addr) = endpoints();
        let (mut side_a, mut side_b) = LoopbackChannel::pair(a_addr, b_addr);

        side_a.send_datagram(b"first").unwrap();
        side_a.send_datagram(b"second").unwrap();

        assert_eq!(side_b.recv_datagram().unwrap(), Some(b"first".to_vec()));
        assert_eq!(side_b.recv_datagram().unwrap(), Some(b"second".to_vec()));
        assert_eq!(side_b.recv_datagram().unwrap(), None);
    }

    #[test]
    fn directions_are_independent() {
        let (a_addr, b_addr) = endpoints();
        let (mut side_a, mut side_b) = LoopbackChannel::pair(a_addr, b_addr);

        side_b.send_datagram(b"reply").unwrap();

        assert_eq!(side_a.recv_datagram().unwrap(), Some(b"reply".to_vec()));
        assert_eq!(side_b.recv_datagram().unwrap(), None);
    }
}
