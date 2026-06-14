//! The top-level wire message and its binary encoding.
//!
//! Everything that crosses the network is a [`Message`]. Encoding uses postcard,
//! a compact varint binary format, chosen for small datagrams and low-latency
//! (de)serialization. Framing and the secure channel are Transport's job; this
//! module only turns a `Message` into bytes and back.

use crate::control::ControlMessage;
use crate::ids::SessionId;
use crate::input::InputEvent;
use serde::{Deserialize, Serialize};

/// Anything that travels between two machines: either a single input event bound
/// to a session, or an out-of-band control message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    /// An input event belonging to an established session.
    Input {
        session: SessionId,
        event: InputEvent,
    },
    /// Session signalling (connect, accept, heartbeat, ...).
    Control(ControlMessage),
}

/// Why encoding or decoding a [`Message`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The bytes could not be turned into a `Message` (truncated or malformed).
    Malformed,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Malformed => f.write_str("malformed protocol message"),
        }
    }
}

impl std::error::Error for CodecError {}

/// Serializes a message into a compact byte buffer ready to hand to Transport.
pub fn encode(message: &Message) -> Result<Vec<u8>, CodecError> {
    postcard::to_allocvec(message).map_err(|_| CodecError::Malformed)
}

/// Parses a message from received bytes. Any leftover or malformed input is
/// rejected rather than silently accepted.
pub fn decode(bytes: &[u8]) -> Result<Message, CodecError> {
    postcard::from_bytes(bytes).map_err(|_| CodecError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Action, KeyCode, Modifiers, MouseButton, MouseDelta};

    fn round_trip(message: Message) {
        let bytes = encode(&message).expect("encoding should succeed");
        let decoded = decode(&bytes).expect("decoding should succeed");
        assert_eq!(decoded, message);
    }

    #[test]
    fn input_key_event_round_trips() {
        round_trip(Message::Input {
            session: SessionId::new(123),
            event: InputEvent::Key {
                code: KeyCode::new(0x04),
                action: Action::Press,
                modifiers: Modifiers::CONTROL.with(Modifiers::SHIFT),
            },
        });
    }

    #[test]
    fn input_motion_pointer_and_button_round_trip() {
        round_trip(Message::Input {
            session: SessionId::new(1),
            event: InputEvent::Motion(MouseDelta::new(-10, 4)),
        });
        round_trip(Message::Input {
            session: SessionId::new(1),
            event: InputEvent::Pointer { x: -1920, y: 1080 },
        });
        round_trip(Message::Input {
            session: SessionId::new(1),
            event: InputEvent::Button {
                button: MouseButton::Other(7),
                action: Action::Release,
            },
        });
    }

    #[test]
    fn control_message_round_trips() {
        round_trip(Message::Control(ControlMessage::Heartbeat {
            session: SessionId::new(u128::MAX),
        }));
    }

    #[test]
    fn truncated_input_is_rejected() {
        let bytes = encode(&Message::Control(ControlMessage::Disconnect {
            session: SessionId::new(5),
        }))
        .unwrap();

        assert_eq!(
            decode(&bytes[..bytes.len() - 1]),
            Err(CodecError::Malformed)
        );
    }

    #[test]
    fn empty_input_is_rejected() {
        assert_eq!(decode(&[]), Err(CodecError::Malformed));
    }
}
