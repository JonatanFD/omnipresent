//! The local IPC surface between the `omni` CLI and the daemon: JSON lines
//! over a Unix domain socket in the config directory.

use serde::{Deserialize, Serialize};

/// A command from the CLI to the daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
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
}

/// The daemon's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Error { message: String },
    Status(StatusInfo),
    Peers { peers: Vec<PeerInfo> },
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
}
