//
//  DaemonViewModelTests.swift
//  omni-macosTests
//
//  The view model against a fake client: the handshake, the incompatible-daemon
//  notice, applying a pushed snapshot, refreshing peers/placements, surfacing a
//  command error, reconnect-loop state, and Start/Stop button conditions.
//

import Foundation
import Testing

@testable import omni_macos

// MARK: - Fake clients

/// A scripted `DaemonClient` for view-model tests.
final class FakeDaemonClient: DaemonClient, @unchecked Sendable {
    var helloInfo = HelloInfo(protocolVersion: 1, daemonVersion: "0.4.0")
    var snapshot = StatusInfo(
        fingerprint: "abc", port: 4733, capturing: true, clipboardSharing: false,
        sessions: [], pending: [])
    var peersList: [PeerInfo] = []
    var layoutList: [LayoutInfo] = []
    var commandError: OmniDaemonError?
    /// When true, subscribe() finishes after the first snapshot instead of
    /// staying open — lets tests exercise the reconnect path.
    var subscribeFinishesImmediately = false

    private let lock = NSLock()
    private var _connectedHosts: [String] = []
    private var _stopCalled = false

    var connectedHosts: [String] {
        lock.lock(); defer { lock.unlock() }; return _connectedHosts
    }
    var stopCalled: Bool {
        lock.lock(); defer { lock.unlock() }; return _stopCalled
    }

    func hello() async throws -> HelloInfo { helloInfo }
    func status() async throws -> StatusInfo { snapshot }
    func peers() async throws -> [PeerInfo] { peersList }
    func layout() async throws -> [LayoutInfo] { layoutList }

    func connect(host: String) async throws {
        if let commandError { throw commandError }
        lock.lock(); _connectedHosts.append(host); lock.unlock()
    }
    func disconnect(host: String) async throws { if let commandError { throw commandError } }
    func accept(selector: String) async throws { if let commandError { throw commandError } }
    func reject(selector: String) async throws { if let commandError { throw commandError } }
    func removePeer(selector: String) async throws { if let commandError { throw commandError } }
    func setLayout(host: String, edge: String) async throws { if let commandError { throw commandError } }
    func setClipboard(enabled: Bool) async throws { if let commandError { throw commandError } }
    func stop() async throws {
        if let commandError { throw commandError }
        lock.lock(); _stopCalled = true; lock.unlock()
    }

    func subscribe() -> AsyncThrowingStream<StatusInfo, Error> {
        let snap = snapshot
        let finishImmediately = subscribeFinishesImmediately
        return AsyncThrowingStream { continuation in
            continuation.yield(snap)
            if finishImmediately {
                continuation.finish()
            }
            // Otherwise leave the stream open so the view model stays "connected"
            // until the test cancels the run task (mirrors a live daemon).
        }
    }
}

/// A `DaemonClient` that starts unavailable and can be flipped to available at
/// runtime — lets tests verify that `reconnectNow()` bypasses the retry delay.
final class ToggleableDaemonClient: DaemonClient, @unchecked Sendable {
    private let lock = NSLock()
    private var _available = false

    var available: Bool {
        get { lock.withLock { _available } }
        set { lock.withLock { _available = newValue } }
    }

    private func checkAvailable() throws {
        guard lock.withLock({ _available }) else {
            throw OmniDaemonError("the omni daemon is not running")
        }
    }

    func hello() async throws -> HelloInfo {
        try checkAvailable()
        return HelloInfo(protocolVersion: 1, daemonVersion: "0.4.0")
    }
    func status() async throws -> StatusInfo {
        try checkAvailable()
        return StatusInfo(fingerprint: "ok", port: 4733, capturing: false,
                         clipboardSharing: false, sessions: [], pending: [])
    }
    func peers() async throws -> [PeerInfo] { [] }
    func layout() async throws -> [LayoutInfo] { [] }
    func connect(host: String) async throws {}
    func disconnect(host: String) async throws {}
    func accept(selector: String) async throws {}
    func reject(selector: String) async throws {}
    func removePeer(selector: String) async throws {}
    func setLayout(host: String, edge: String) async throws {}
    func setClipboard(enabled: Bool) async throws {}
    func stop() async throws {}

    func subscribe() -> AsyncThrowingStream<StatusInfo, Error> {
        if !available {
            return AsyncThrowingStream { $0.finish(throwing: OmniDaemonError("not running")) }
        }
        let snap = StatusInfo(fingerprint: "ok", port: 4733, capturing: false,
                             clipboardSharing: false, sessions: [], pending: [])
        return AsyncThrowingStream { continuation in
            continuation.yield(snap)
            // Leave open — mirrors a live daemon holding the connection.
        }
    }
}

/// A `DaemonClient` whose `hello()` always throws — simulates a daemon that is
/// not yet running so the run loop enters the reconnect-wait path.
final class AlwaysFailingDaemonClient: DaemonClient, @unchecked Sendable {
    let error: OmniDaemonError
    init(_ error: OmniDaemonError = OmniDaemonError("the omni daemon is not running")) {
        self.error = error
    }

    func hello() async throws -> HelloInfo { throw error }
    func status() async throws -> StatusInfo { throw error }
    func peers() async throws -> [PeerInfo] { throw error }
    func layout() async throws -> [LayoutInfo] { throw error }
    func connect(host: String) async throws { throw error }
    func disconnect(host: String) async throws { throw error }
    func accept(selector: String) async throws { throw error }
    func reject(selector: String) async throws { throw error }
    func removePeer(selector: String) async throws { throw error }
    func setLayout(host: String, edge: String) async throws { throw error }
    func setClipboard(enabled: Bool) async throws { throw error }
    func stop() async throws { throw error }
    func subscribe() -> AsyncThrowingStream<StatusInfo, Error> {
        let err = error
        return AsyncThrowingStream { $0.finish(throwing: err) }
    }
}

// MARK: - Tests

@MainActor
struct DaemonViewModelTests {
    /// Polls `condition` on the main actor up to a timeout. Avoids fixed sleeps.
    private func wait(
        for condition: @MainActor () -> Bool, timeout: Duration = .seconds(2)
    ) async -> Bool {
        let deadline = ContinuousClock.now.advanced(by: timeout)
        while ContinuousClock.now < deadline {
            if condition() { return true }
            try? await Task.sleep(for: .milliseconds(10))
        }
        return condition()
    }

    // MARK: Happy path

    @Test func handshakeThenAppliesSnapshotAndRefreshesLists() async {
        let client = FakeDaemonClient()
        client.peersList = [PeerInfo(host: "mac", fingerprint: "ab", connected: true)]
        client.layoutList = [LayoutInfo(host: "mac", edge: "left", connected: true)]
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(50))

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        #expect(await wait { viewModel.isConnected })
        #expect(viewModel.fingerprint == "abc")
        #expect(viewModel.port == 4733)
        #expect(viewModel.capturing == true)
        #expect(viewModel.daemonVersion == "0.4.0")
        #expect(viewModel.peers.count == 1)
        #expect(viewModel.placements.count == 1)
    }

    // MARK: Protocol incompatibility

    @Test func tooNewDaemonIsFlaggedIncompatible() async {
        let client = FakeDaemonClient()
        client.helloInfo = HelloInfo(protocolVersion: OmniProtocol.version + 1, daemonVersion: "9.9.9")
        let viewModel = DaemonViewModel(client: client)

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        #expect(await wait { viewModel.isIncompatible })
        #expect(viewModel.hasError)
    }

    // MARK: Command errors

    @Test func commandErrorSurfacesAsLastError() async {
        let client = FakeDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(50))
        let task = Task { await viewModel.run() }
        defer { task.cancel() }
        #expect(await wait { viewModel.isConnected })

        client.commandError = OmniDaemonError("peer not found")
        await viewModel.connect(host: "ghost")
        #expect(viewModel.lastError == "peer not found")
    }

    @Test func successfulCommandReachesTheClientAndClearsError() async {
        let client = FakeDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(50))
        let task = Task { await viewModel.run() }
        defer { task.cancel() }
        #expect(await wait { viewModel.isConnected })

        await viewModel.connect(host: "10.0.0.2:4733")
        #expect(client.connectedHosts == ["10.0.0.2:4733"])
        #expect(viewModel.lastError == nil)
    }

    // MARK: Reconnect-loop state machine

    @Test func reconnectWaitUsesConnectingNotDisconnected() async {
        // When hello() fails the run loop sleeps before retrying. During that
        // sleep the state must be .connecting (not .disconnected) so the sidebar
        // dot stays orange rather than flashing red.
        let client = AlwaysFailingDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(100))

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        #expect(await wait { viewModel.connection == .connecting })
        #expect(viewModel.connection != .disconnected)
    }

    @Test func startButtonRemainsAvailableWhileConnecting() async {
        // The Start button is disabled only when .connected or .incompatible.
        // While the run loop retries (.connecting), it must be tappable so the
        // user can launch the daemon.
        let client = AlwaysFailingDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(100))

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        #expect(await wait { viewModel.connection == .connecting })
        // Start disabled condition: isConnected || isIncompatible
        #expect(!viewModel.isConnected)
        #expect(!viewModel.isIncompatible)
    }

    @Test func stopDaemonCallsClientStop() async {
        let client = FakeDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(50))
        let task = Task { await viewModel.run() }
        defer { task.cancel() }
        #expect(await wait { viewModel.isConnected })

        await viewModel.stopDaemon()
        #expect(client.stopCalled)
        #expect(viewModel.lastError == nil)
    }

    @Test func subscribeStreamEndingTriggersReconnect() async {
        // When the daemon closes its end of the subscribe connection the run loop
        // must transition back to .connecting and retry, not get stuck.
        let client = FakeDaemonClient()
        client.subscribeFinishesImmediately = true
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .milliseconds(50))

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        // Wait for evidence that hello() succeeded at least once (daemonVersion set)
        // AND the loop is back in .connecting — the brief .connected→.connecting
        // transition has no await between them so we can't reliably catch .connected
        // with a 10ms poll. daemonVersion being set proves the loop did connect first.
        #expect(await wait {
            viewModel.connection == .connecting && !viewModel.daemonVersion.isEmpty
        })
    }

    @Test func reconnectNowConnectsImmediatelyAfterDaemonBecomesAvailable() async {
        // This covers the click-Start flow: the daemon starts, reconnectNow() is
        // called, and the UI turns green without waiting for the 5-second delay.
        let client = ToggleableDaemonClient()
        let viewModel = DaemonViewModel(client: client, reconnectDelay: .seconds(5))

        let task = Task { await viewModel.run() }
        defer { task.cancel() }

        // Daemon unavailable — run loop is sleeping in its reconnect interval.
        #expect(await wait { viewModel.connection == .connecting })

        // Daemon becomes available (simulates omni start succeeding).
        client.available = true

        // Without reconnectNow() we would wait the full 5-second delay.
        viewModel.reconnectNow()

        #expect(await wait { viewModel.isConnected })
    }
}
