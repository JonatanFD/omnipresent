//! Control messages: the out-of-band signalling that sets up, maintains, and
//! tears down sessions. Distinct from the input stream, which carries the actual
//! keyboard and mouse events.

use crate::ids::{Fingerprint, MachineId, SessionId};
use serde::{Deserialize, Serialize};

/// A signalling message between two machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlMessage {
    /// "I would like to control you." Sent by the initiator; the target's user
    /// answers with `omni accept` / `omni reject`.
    ConnectRequest {
        machine: MachineId,
        fingerprint: Fingerprint,
    },
    /// The request was approved; the session now exists under this id.
    Accept { session: SessionId },
    /// The request was refused, with the reason why.
    Reject { reason: RejectReason },
    /// Either side ends an established session.
    Disconnect { session: SessionId },
    /// Keep-alive so each side can detect a silently dropped peer.
    Heartbeat { session: SessionId },
}

/// Why a [`ControlMessage::ConnectRequest`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The peer is not on this machine's allowlist.
    NotAllowed,
    /// The peer's certificate fingerprint differs from the pinned one (TOFU).
    FingerprintChanged,
    /// A person explicitly declined the request.
    Declined,
    /// The machine is already in a session and cannot take another.
    Busy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_request_carries_who_is_asking() {
        let msg = ControlMessage::ConnectRequest {
            machine: MachineId::new(1),
            fingerprint: Fingerprint::from_bytes([9; 32]),
        };

        match msg {
            ControlMessage::ConnectRequest {
                machine,
                fingerprint,
            } => {
                assert_eq!(machine, MachineId::new(1));
                assert_eq!(fingerprint, Fingerprint::from_bytes([9; 32]));
            }
            _ => panic!("expected a connect request"),
        }
    }
}
