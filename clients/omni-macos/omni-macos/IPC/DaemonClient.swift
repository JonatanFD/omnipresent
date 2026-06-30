//
//  DaemonClient.swift
//  omni-macos
//
//  The abstraction the view model talks to. The real implementation is
//  `UnixSocketDaemonClient`; tests use a fake. Mirrors the Windows client's
//  `IOmniDaemonClient`.
//

import Foundation

/// The daemon's answer to a version handshake.
public nonisolated struct HelloInfo: Equatable, Sendable {
    public let protocolVersion: UInt32
    public let daemonVersion: String

    public init(protocolVersion: UInt32, daemonVersion: String) {
        self.protocolVersion = protocolVersion
        self.daemonVersion = daemonVersion
    }
}

/// A client of the daemon's local IPC. Each command maps to exactly one
/// `Request`; `subscribe` opens a long-lived push stream of status snapshots.
/// The client holds no business logic — the daemon owns all state.
public nonisolated protocol DaemonClient: Sendable {
    func hello() async throws -> HelloInfo
    func status() async throws -> StatusInfo
    func peers() async throws -> [PeerInfo]
    func layout() async throws -> [LayoutInfo]

    func connect(host: String) async throws
    func disconnect(host: String) async throws
    func accept(selector: String) async throws
    func reject(selector: String) async throws
    func removePeer(selector: String) async throws
    func setLayout(host: String, edge: String) async throws
    func setClipboard(enabled: Bool) async throws
    func stop() async throws

    /// A live stream of status snapshots, pushed by the daemon on change. The
    /// stream finishes when the daemon closes the connection (or on error).
    func subscribe() -> AsyncThrowingStream<StatusInfo, Error>
}
