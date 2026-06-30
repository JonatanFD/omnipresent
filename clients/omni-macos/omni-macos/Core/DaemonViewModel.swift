//
//  DaemonViewModel.swift
//  omni-macos
//
//  A live, bindable view of the daemon. It performs the version handshake,
//  follows the daemon's push stream to keep state current, reconnects on its
//  own when the daemon goes away, and exposes commands that map straight to IPC
//  requests. It holds no business logic — the daemon owns state. Mirrors the
//  Windows client's `DaemonViewModel`.
//

import Foundation
import Observation

@Observable
@MainActor
public final class DaemonViewModel {
    /// The client's view of the IPC connection.
    public enum ConnectionStatus {
        case connecting
        case connected
        case disconnected
        case incompatible
    }

    private let client: DaemonClient
    private let reconnectDelay: Duration
    private var runTask: Task<Void, Never>?

    public init(client: DaemonClient, reconnectDelay: Duration = .seconds(2)) {
        self.client = client
        self.reconnectDelay = reconnectDelay
    }

    /// Starts the connect/subscribe/reconnect loop once, for the whole app
    /// lifetime. Idempotent: safe to call from both the window and the menu-bar
    /// views, so live updates keep flowing even while the window is closed.
    public func start() {
        guard runTask == nil else { return }
        runTask = Task { await self.run() }
    }

    // MARK: - Observed state

    public private(set) var connection: ConnectionStatus = .connecting
    public private(set) var statusText = "Connecting…"
    public private(set) var fingerprint = ""
    public private(set) var port: UInt16 = 0
    public private(set) var capturing = false
    public private(set) var clipboardSharing = false
    public private(set) var daemonVersion = ""
    public private(set) var lastError: String?

    public private(set) var sessions: [SessionInfo] = []
    public private(set) var pending: [PendingInfo] = []
    public private(set) var peers: [PeerInfo] = []
    public private(set) var placements: [LayoutInfo] = []

    /// True only while live, for enabling/disabling controls.
    public var isConnected: Bool { connection == .connected }
    /// True when the daemon is too new; the UI shows an "update" notice.
    public var isIncompatible: Bool { connection == .incompatible }
    /// True when there is an error message to show.
    public var hasError: Bool { !(lastError ?? "").isEmpty }

    // MARK: - Lifecycle

    /// Runs until the surrounding task is cancelled: handshake, then follow the
    /// push stream, reconnecting after a delay whenever it drops. Stops
    /// permanently only if the daemon is too new (`incompatible`).
    public func run() async {
        while !Task.isCancelled {
            do {
                let hello = try await client.hello()
                if hello.protocolVersion > OmniProtocol.version {
                    connection = .incompatible
                    statusText = """
                        The daemon speaks protocol v\(hello.protocolVersion); this app \
                        understands v\(OmniProtocol.version). Please update Omnipresent.
                        """
                    lastError = statusText
                    return
                }
                daemonVersion = hello.daemonVersion

                for try await snapshot in client.subscribe() {
                    apply(snapshot)
                    await refreshLists()
                    connection = .connected
                    statusText = "Connected"
                }
            } catch is CancellationError {
                break
            } catch let error as OmniDaemonError {
                setDisconnected(error.message)
            } catch {
                setDisconnected(error.localizedDescription)
            }

            if Task.isCancelled { break }
            // Use .connecting (not .disconnected) while sleeping before the next
            // attempt — the daemon may just be starting up, so orange is correct.
            connection = .connecting
            statusText = "Waiting for the daemon…"
            do {
                try await Task.sleep(for: reconnectDelay)
            } catch {
                break  // cancelled while waiting
            }
        }
    }

    // MARK: - Commands

    public func connect(host: String) async { await guarded { try await self.client.connect(host: host) } }
    public func disconnect(host: String) async { await guarded { try await self.client.disconnect(host: host) } }
    public func accept(selector: String) async { await guarded { try await self.client.accept(selector: selector) } }
    public func reject(selector: String) async { await guarded { try await self.client.reject(selector: selector) } }
    public func removePeer(selector: String) async { await guarded { try await self.client.removePeer(selector: selector) } }
    public func setLayout(host: String, edge: String) async { await guarded { try await self.client.setLayout(host: host, edge: edge) } }
    public func setClipboard(enabled: Bool) async { await guarded { try await self.client.setClipboard(enabled: enabled) } }

    /// Launches `omni start` then immediately retries the connection so the UI
    /// turns green without waiting for the next scheduled reconnect interval.
    public func startDaemon() async {
        lastError = nil
        let candidates = [
            "/usr/local/bin/omni",
            "/opt/homebrew/bin/omni",
            "\(NSHomeDirectory())/.local/bin/omni",
            "\(NSHomeDirectory())/.cargo/bin/omni",
        ]
        guard let binaryPath = candidates.first(where: {
            FileManager.default.isExecutableFile(atPath: $0)
        }) else {
            lastError = "Could not find the omni binary. Make sure Omnipresent is installed."
            return
        }
        do {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: binaryPath)
            process.arguments = ["start"]
            try process.run()
        } catch {
            lastError = "Failed to start the daemon: \(error.localizedDescription)"
            return
        }
        // Restart the run loop immediately so we don't wait for the retry sleep.
        reconnectNow()
    }

    /// Sends a stop command to the running daemon.
    public func stopDaemon() async {
        await guarded { try await self.client.stop() }
    }

    /// Cancels the current run loop and starts a fresh one immediately,
    /// bypassing any in-progress reconnect-delay sleep.
    public func reconnectNow() {
        runTask?.cancel()
        runTask = nil
        runTask = Task { await self.run() }
    }

    // MARK: - Internals

    private func guarded(_ action: () async throws -> Void) async {
        lastError = nil
        do {
            try await action()
        } catch let error as OmniDaemonError {
            lastError = error.message
        } catch {
            lastError = error.localizedDescription
        }
    }

    private func apply(_ snapshot: StatusInfo) {
        fingerprint = snapshot.fingerprint
        port = snapshot.port
        capturing = snapshot.capturing
        clipboardSharing = snapshot.clipboardSharing
        sessions = snapshot.sessions
        pending = snapshot.pending
    }

    private func refreshLists() async {
        // Peers and placements are separate requests, not part of the snapshot;
        // refresh them whenever state changes so the lists stay live. Keep the
        // last values if a refresh fails — the snapshot itself still applied.
        do {
            let freshPeers = try await client.peers()
            let freshPlacements = try await client.layout()
            peers = freshPeers
            placements = freshPlacements
        } catch {
            // keep whatever we last had
        }
    }

    private func setDisconnected(_ text: String) {
        connection = .disconnected
        statusText = text
    }
}
