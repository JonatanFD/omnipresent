//! Turning Protocol messages into datagrams and back over a [`SecureChannel`].
//!
//! This is the thin glue that the Runtime drives: encode a [`Message`] and hand
//! it to the channel; poll the channel and decode what comes back. Everything
//! security-related (the handshake, mutual auth, replay protection) is inside the
//! channel — Transport only frames messages.

use crate::channel::SecureChannel;
use crate::endpoint::Endpoint;
use omni_protocol::{CodecError, Message, decode, encode};

/// Why sending or receiving a message failed.
#[derive(Debug)]
pub enum TransportError<C> {
    /// The underlying secure channel failed.
    Channel(C),
    /// A message could not be encoded or a received datagram could not be decoded.
    Codec(CodecError),
}

impl<C: std::fmt::Display> std::fmt::Display for TransportError<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Channel(e) => write!(f, "secure channel error: {e}"),
            TransportError::Codec(e) => write!(f, "codec error: {e}"),
        }
    }
}

impl<C: std::fmt::Debug + std::fmt::Display> std::error::Error for TransportError<C> {}

/// Sends and receives Protocol [`Message`]s over a single peer's secure channel.
#[derive(Debug)]
pub struct Transport<C> {
    channel: C,
}

impl<C: SecureChannel> Transport<C> {
    /// Wraps an established secure channel.
    pub fn new(channel: C) -> Self {
        Self { channel }
    }

    /// The peer this transport talks to.
    pub fn peer(&self) -> Endpoint {
        self.channel.peer()
    }

    /// Read-only access to the channel (mainly for tests).
    pub fn channel(&self) -> &C {
        &self.channel
    }

    /// Mutable access to the channel, for adapter-specific operations (e.g.
    /// the QUIC adapter's async receive).
    pub fn channel_mut(&mut self) -> &mut C {
        &mut self.channel
    }

    /// Encodes a message and sends it as one datagram.
    pub fn send(&mut self, message: &Message) -> Result<(), TransportError<C::Error>> {
        let payload = encode(message).map_err(TransportError::Codec)?;
        self.channel
            .send_datagram(&payload)
            .map_err(TransportError::Channel)
    }

    /// Polls for the next datagram and decodes it. Returns `None` when nothing is
    /// waiting.
    pub fn recv(&mut self) -> Result<Option<Message>, TransportError<C::Error>> {
        let Some(payload) = self
            .channel
            .recv_datagram()
            .map_err(TransportError::Channel)?
        else {
            return Ok(None);
        };
        let message = decode(&payload).map_err(TransportError::Codec)?;
        Ok(Some(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::LoopbackChannel;
    use omni_protocol::input::{Action, KeyCode, Modifiers, MouseDelta};
    use omni_protocol::{ControlMessage, InputEvent, SessionId};

    fn endpoints() -> (Endpoint, Endpoint) {
        (
            Endpoint::new("127.0.0.1:8001".parse().unwrap()),
            Endpoint::new("127.0.0.1:8002".parse().unwrap()),
        )
    }

    fn key_message() -> Message {
        Message::Input {
            session: SessionId::new(1),
            event: InputEvent::Key {
                code: KeyCode::new(0x04),
                action: Action::Press,
                modifiers: Modifiers::CONTROL,
            },
        }
    }

    fn motion_message() -> Message {
        Message::Input {
            session: SessionId::new(1),
            event: InputEvent::Motion(MouseDelta::new(5, -7)),
        }
    }

    #[test]
    fn messages_round_trip_in_order() {
        let (a, b) = endpoints();
        let (chan_a, chan_b) = LoopbackChannel::pair(a, b);
        let mut sender = Transport::new(chan_a);
        let mut receiver = Transport::new(chan_b);

        sender.send(&key_message()).unwrap();
        sender.send(&motion_message()).unwrap();

        assert_eq!(receiver.recv().unwrap(), Some(key_message()));
        assert_eq!(receiver.recv().unwrap(), Some(motion_message()));
        assert_eq!(receiver.recv().unwrap(), None);
    }

    #[test]
    fn control_messages_round_trip() {
        let (a, b) = endpoints();
        let (chan_a, chan_b) = LoopbackChannel::pair(a, b);
        let mut sender = Transport::new(chan_a);
        let mut receiver = Transport::new(chan_b);

        let message = Message::Control(ControlMessage::Disconnect {
            session: SessionId::new(9),
        });
        sender.send(&message).unwrap();

        assert_eq!(receiver.recv().unwrap(), Some(message));
    }

    #[test]
    fn transport_reports_its_peer() {
        let (a, b) = endpoints();
        let (chan_a, _chan_b) = LoopbackChannel::pair(a, b);
        let sender = Transport::new(chan_a);
        assert_eq!(sender.peer(), b);
    }

    #[test]
    fn a_malformed_datagram_is_a_codec_error() {
        // Inject raw bytes that are not a valid encoded Message via a bare
        // channel, then try to receive them through a Transport.
        let (a, b) = endpoints();
        let (mut raw_sender, chan_b) = LoopbackChannel::pair(a, b);
        let mut receiver = Transport::new(chan_b);

        raw_sender.send_datagram(&[]).unwrap();

        assert!(matches!(
            receiver.recv(),
            Err(TransportError::Codec(CodecError::Malformed)),
        ));
    }

    #[test]
    fn recv_on_an_idle_channel_is_none() {
        let (a, b) = endpoints();
        let (_chan_a, chan_b) = LoopbackChannel::pair(a, b);
        let mut receiver = Transport::new(chan_b);
        assert_eq!(receiver.recv().unwrap(), None);
    }
}
