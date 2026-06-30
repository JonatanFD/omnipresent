//
//  OmniProtocol.swift
//  omni-macos
//
//  The IPC contract between this app and the omni daemon. Mirrors the Rust
//  `Request` / `Response` / `Event` types in `crates/omni-runtime/src/ipc.rs`
//  (the single source of truth): JSON lines over the daemon's Unix-domain
//  socket. This file is the only place that knows the wire shape; everything
//  else in the app speaks these Swift types.
//
//  The types are `nonisolated` because (de)serialization runs off the main
//  actor on the socket I/O queue, while the app's default actor isolation is
//  the main actor.
//

import Foundation

/// The IPC protocol version this client understands. The client sends
/// `Request.hello` first and compares the daemon's answer; if the daemon speaks
/// a newer version, the app tells the user to update rather than misbehave.
/// Matches `PROTOCOL_VERSION` in `ipc.rs`.
public nonisolated enum OmniProtocol {
    public static let version: UInt32 = 1
}

// MARK: - Errors

/// A failure reported by the daemon, or by the transport while talking to it.
/// Mirrors the Windows client's `OmniDaemonException`.
public nonisolated struct OmniDaemonError: Error, Equatable, CustomStringConvertible {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var description: String { message }

    static func unexpected(_ response: Response) -> OmniDaemonError {
        OmniDaemonError("unexpected response from the daemon")
    }
}

// MARK: - Requests

/// A command sent from this client to the daemon. Encodes to one JSON line
/// tagged by a `"cmd"` field, e.g. `{"cmd":"connect","host":"10.0.0.2:4733"}`.
public nonisolated enum Request: Equatable, Sendable {
    case hello
    case subscribe
    case status
    case stop
    case peers
    case connect(host: String)
    case disconnect(host: String)
    case accept(selector: String)
    case reject(selector: String)
    case removePeer(selector: String)
    case layout(host: String?, edge: String?)
    case clipboard(enabled: Bool)
}

nonisolated extension Request: Encodable {
    private enum Key: String, CodingKey { case cmd, host, edge, selector, enabled }

    public nonisolated func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: Key.self)
        switch self {
        case .hello: try c.encode("hello", forKey: .cmd)
        case .subscribe: try c.encode("subscribe", forKey: .cmd)
        case .status: try c.encode("status", forKey: .cmd)
        case .stop: try c.encode("stop", forKey: .cmd)
        case .peers: try c.encode("peers", forKey: .cmd)
        case let .connect(host):
            try c.encode("connect", forKey: .cmd)
            try c.encode(host, forKey: .host)
        case let .disconnect(host):
            try c.encode("disconnect", forKey: .cmd)
            try c.encode(host, forKey: .host)
        case let .accept(selector):
            try c.encode("accept", forKey: .cmd)
            try c.encode(selector, forKey: .selector)
        case let .reject(selector):
            try c.encode("reject", forKey: .cmd)
            try c.encode(selector, forKey: .selector)
        case let .removePeer(selector):
            try c.encode("remove_peer", forKey: .cmd)
            try c.encode(selector, forKey: .selector)
        case let .layout(host, edge):
            try c.encode("layout", forKey: .cmd)
            // The daemon's `Option`s serialize as JSON null when absent (no
            // skip), so we encode them the same way to round-trip exactly.
            try c.encode(host, forKey: .host)
            try c.encode(edge, forKey: .edge)
        case let .clipboard(enabled):
            try c.encode("clipboard", forKey: .cmd)
            try c.encode(enabled, forKey: .enabled)
        }
    }
}

// MARK: - Responses

/// The daemon's answer to a `Request`. Tagged by a `"result"` field. The
/// `status` / `peers` / `layout` variants carry their payload alongside the
/// tag (serde flattens newtype variants), so decoding reads the discriminator
/// and then the rest of the same object.
public nonisolated enum Response: Equatable, Sendable {
    case ok
    case error(message: String)
    case hello(protocolVersion: UInt32, daemonVersion: String)
    case status(StatusInfo)
    case peers([PeerInfo])
    case layout([LayoutInfo])
}

nonisolated extension Response: Decodable {
    private enum Key: String, CodingKey {
        case result, message, protocol_version, daemon_version, peers, placements
    }

    public nonisolated init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Key.self)
        let result = try c.decode(String.self, forKey: .result)
        switch result {
        case "ok":
            self = .ok
        case "error":
            self = .error(message: try c.decode(String.self, forKey: .message))
        case "hello":
            self = .hello(
                protocolVersion: try c.decode(UInt32.self, forKey: .protocol_version),
                daemonVersion: try c.decode(String.self, forKey: .daemon_version))
        case "status":
            // Flattened: StatusInfo fields sit next to "result".
            self = .status(try StatusInfo(from: decoder))
        case "peers":
            self = .peers(try c.decode([PeerInfo].self, forKey: .peers))
        case "layout":
            self = .layout(try c.decode([LayoutInfo].self, forKey: .placements))
        default:
            throw OmniDaemonError("unknown response from the daemon: \(result)")
        }
    }
}

// MARK: - Events (pushed on a Subscribe connection)

/// A pushed update on a `Request.subscribe` connection. Tagged by an `"event"`
/// field, which is what tells a subscriber a push apart from a `Response`. For
/// now the only event is a full status snapshot; the client just re-renders.
public nonisolated enum Event: Equatable, Sendable {
    case status(StatusInfo)
}

nonisolated extension Event: Decodable {
    private enum Key: String, CodingKey { case event }

    public nonisolated init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Key.self)
        let event = try c.decode(String.self, forKey: .event)
        switch event {
        case "status":
            self = .status(try StatusInfo(from: decoder))
        default:
            throw OmniDaemonError("unknown event from the daemon: \(event)")
        }
    }
}

// MARK: - Payloads

/// What `omni status` shows. Mirrors the Rust `StatusInfo`. `capturing` and
/// `clipboard_sharing` carry `#[serde(default)]` on the daemon, so they may be
/// absent from an older daemon — decode them defensively as `false`.
public nonisolated struct StatusInfo: Equatable, Sendable, Codable {
    public let fingerprint: String
    public let port: UInt16
    public let capturing: Bool
    public let clipboardSharing: Bool
    public let sessions: [SessionInfo]
    public let pending: [PendingInfo]

    enum CodingKeys: String, CodingKey {
        case fingerprint, port, capturing
        case clipboardSharing = "clipboard_sharing"
        case sessions, pending
    }

    public init(
        fingerprint: String, port: UInt16, capturing: Bool, clipboardSharing: Bool,
        sessions: [SessionInfo], pending: [PendingInfo]
    ) {
        self.fingerprint = fingerprint
        self.port = port
        self.capturing = capturing
        self.clipboardSharing = clipboardSharing
        self.sessions = sessions
        self.pending = pending
    }

    public nonisolated init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        fingerprint = try c.decode(String.self, forKey: .fingerprint)
        port = try c.decode(UInt16.self, forKey: .port)
        capturing = try c.decodeIfPresent(Bool.self, forKey: .capturing) ?? false
        clipboardSharing = try c.decodeIfPresent(Bool.self, forKey: .clipboardSharing) ?? false
        sessions = try c.decodeIfPresent([SessionInfo].self, forKey: .sessions) ?? []
        pending = try c.decodeIfPresent([PendingInfo].self, forKey: .pending) ?? []
    }
}

/// One active session. `role` is this machine's role: "controller" or "target".
public nonisolated struct SessionInfo: Equatable, Sendable, Codable, Identifiable {
    public let host: String
    public let fingerprint: String
    public let role: String
    public let active: Bool

    public var id: String { "\(host)#\(fingerprint)" }

    public init(host: String, fingerprint: String, role: String, active: Bool) {
        self.host = host
        self.fingerprint = fingerprint
        self.role = role
        self.active = active
    }
}

/// An incoming request awaiting accept/reject. The accept prompt shows the
/// peer's host and fingerprint — the human verification point for TOFU.
public nonisolated struct PendingInfo: Equatable, Sendable, Codable, Identifiable {
    public let host: String
    public let fingerprint: String

    public var id: String { fingerprint }

    public init(host: String, fingerprint: String) {
        self.host = host
        self.fingerprint = fingerprint
    }
}

/// One known peer. `host` may be nil if the peer was never named.
public nonisolated struct PeerInfo: Equatable, Sendable, Codable, Identifiable {
    public let host: String?
    public let fingerprint: String
    public let connected: Bool

    public var id: String { fingerprint }

    public init(host: String?, fingerprint: String, connected: Bool) {
        self.host = host
        self.fingerprint = fingerprint
        self.connected = connected
    }
}

/// One peer's placement in the virtual desktop. `edge` is "left", "right",
/// "top", or "bottom"; `connected` is true if from a live session, false if
/// only saved in the config for next time.
public nonisolated struct LayoutInfo: Equatable, Sendable, Codable, Identifiable {
    public let host: String
    public let edge: String
    public let connected: Bool

    public var id: String { host }

    public init(host: String, edge: String, connected: Bool) {
        self.host = host
        self.edge = edge
        self.connected = connected
    }
}

// MARK: - Codec

/// JSON-line (de)serialization for the IPC protocol. Encoders/decoders are
/// created per call so the codec is safe to use from any thread.
public nonisolated enum ProtocolCodec {
    /// Serialize a request to a single JSON line (no trailing newline).
    public static func encode(_ request: Request) throws -> String {
        let data = try JSONEncoder().encode(request)
        return String(decoding: data, as: UTF8.self)
    }

    /// Decode one response line. A daemon-reported error surfaces as-is; any
    /// other decode failure becomes a clear "cannot read" error.
    public static func decodeResponse(_ line: String) throws -> Response {
        do {
            return try JSONDecoder().decode(Response.self, from: Data(line.utf8))
        } catch let error as OmniDaemonError {
            throw error
        } catch {
            throw OmniDaemonError("the daemon sent a response this client cannot read")
        }
    }

    /// Decode one pushed event line.
    public static func decodeEvent(_ line: String) throws -> Event {
        do {
            return try JSONDecoder().decode(Event.self, from: Data(line.utf8))
        } catch let error as OmniDaemonError {
            throw error
        } catch {
            throw OmniDaemonError("the daemon sent an event this client cannot read")
        }
    }
}
