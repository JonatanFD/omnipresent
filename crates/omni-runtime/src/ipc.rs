//! The local IPC surface between the `omni` CLI and the daemon: JSON lines
//! over a Unix domain socket in the config directory.

use serde::{Deserialize, Serialize};

/// The IPC protocol version. A client sends [`Request::Hello`] first and compares
/// the [`Response::Hello`] it gets back; if the daemon's version is newer than the
/// client understands, the client tells the user to update rather than misbehave.
/// Bump this only on a breaking change — additive `Request`/`Response`/`Event`
/// variants and new optional fields stay backward-compatible and do not need it.
pub const PROTOCOL_VERSION: u32 = 1;

/// A command from the CLI to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Version handshake: ask the daemon which protocol version it speaks.
    Hello,
    /// Subscribe to live updates. The connection stays open and the daemon
    /// pushes [`Event`] lines as state changes, until the client disconnects.
    /// Commands are sent on a separate connection.
    Subscribe,
    /// Daemon and session overview.
    Status,
    /// Shut the daemon down.
    Stop,
    /// Dial a peer and request control of it.
    Connect { host: String },
    /// End the session with a peer.
    Disconnect { host: String },
    /// Approve a pending incoming request (by host or fingerprint prefix).
    Accept { selector: String },
    /// Deny a pending incoming request (by host or fingerprint prefix).
    Reject { selector: String },
    /// List known peers.
    Peers,
    /// Forget a peer (by host or fingerprint prefix).
    RemovePeer { selector: String },
    /// Inspect or change where peers sit in the virtual desktop. With `host`
    /// and `edge` set, place that peer past the given edge; with both `None`,
    /// list the current placements.
    Layout {
        host: Option<String>,
        edge: Option<String>,
    },
    /// Turn opt-in clipboard sharing on or off at runtime. The choice is
    /// persisted to the config so it survives a daemon restart.
    Clipboard { enabled: bool },
}

/// The daemon's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error {
        message: String,
    },
    /// Answer to [`Request::Hello`]: the daemon's protocol and build versions.
    Hello {
        protocol_version: u32,
        daemon_version: String,
    },
    Status(StatusInfo),
    Peers {
        peers: Vec<PeerInfo>,
    },
    Layout {
        placements: Vec<LayoutInfo>,
    },
}

/// A pushed update sent on a [`Request::Subscribe`] connection. Each event is one
/// JSON line, distinguished from a [`Response`] by its `event` tag. For now a
/// change pushes a fresh full [`StatusInfo`] snapshot — coarse but simple, and the
/// client just re-renders. Finer-grained events can be added later, additively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Status(StatusInfo),
}

/// What `omni status` shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    /// This machine's certificate fingerprint (what peers pin).
    pub fingerprint: String,
    /// The UDP port the daemon listens on.
    pub port: u16,
    /// Whether local input capture is running (false = target-only: the OS
    /// permission for capture is missing or the capture thread died).
    #[serde(default)]
    pub capturing: bool,
    /// Whether opt-in clipboard sharing is currently on.
    #[serde(default)]
    pub clipboard_sharing: bool,
    /// Active sessions.
    pub sessions: Vec<SessionInfo>,
    /// Incoming requests awaiting `omni accept` / `omni reject`.
    pub pending: Vec<PendingInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub host: String,
    pub fingerprint: String,
    /// This machine's role: "controller" or "target".
    pub role: String,
    /// Whether input is currently routed to this peer.
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingInfo {
    pub host: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub host: Option<String>,
    pub fingerprint: String,
    pub connected: bool,
}

/// One peer's placement in the virtual desktop, as `omni layout` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutInfo {
    pub host: String,
    /// The edge this peer sits past: "left", "right", "top", or "bottom".
    pub edge: String,
    /// Whether this placement is from a live session (`true`) or only saved in
    /// the config for the next time the peer connects (`false`).
    pub connected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_as_json_lines() {
        let requests = [
            Request::Status,
            Request::Connect {
                host: "10.0.0.2:4733".into(),
            },
            Request::Accept {
                selector: "ab12".into(),
            },
            Request::RemovePeer {
                selector: "laptop".into(),
            },
            Request::Clipboard { enabled: true },
        ];
        for request in requests {
            let line = serde_json::to_string(&request).unwrap();
            assert!(!line.contains('\n'));
            let back: Request = serde_json::from_str(&line).unwrap();
            assert_eq!(back, request);
        }
    }

    #[test]
    fn responses_round_trip() {
        let response = Response::Status(StatusInfo {
            fingerprint: "ab".repeat(32),
            port: 4733,
            capturing: true,
            clipboard_sharing: true,
            sessions: vec![SessionInfo {
                host: "10.0.0.2".into(),
                fingerprint: "cd".repeat(32),
                role: "controller".into(),
                active: true,
            }],
            pending: vec![],
        });
        let line = serde_json::to_string(&response).unwrap();
        let back: Response = serde_json::from_str(&line).unwrap();
        assert_eq!(back, response);
    }

    #[test]
    fn hello_handshake_round_trips() {
        let request = Request::Hello;
        let line = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), request);

        let response = Response::Hello {
            protocol_version: PROTOCOL_VERSION,
            daemon_version: "0.3.7".into(),
        };
        let line = serde_json::to_string(&response).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&line).unwrap(), response);
    }

    #[test]
    fn subscribe_request_round_trips() {
        let line = serde_json::to_string(&Request::Subscribe).unwrap();
        assert_eq!(
            serde_json::from_str::<Request>(&line).unwrap(),
            Request::Subscribe
        );
    }

    #[test]
    fn an_event_is_one_line_tagged_as_an_event() {
        let event = Event::Status(StatusInfo {
            fingerprint: "ab".repeat(32),
            port: 4733,
            capturing: false,
            clipboard_sharing: false,
            sessions: vec![],
            pending: vec![],
        });
        let line = serde_json::to_string(&event).unwrap();
        assert!(!line.contains('\n'));
        // The `event` tag is what lets a subscriber tell a push from a response.
        assert!(line.contains("\"event\":\"status\""));
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), event);
    }
}
