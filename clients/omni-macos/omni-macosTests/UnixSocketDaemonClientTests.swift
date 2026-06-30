//
//  UnixSocketDaemonClientTests.swift
//  omni-macosTests
//
//  The real transport against a fake daemon bound to an actual Unix-domain
//  socket: one-shot request/response, the exact request bytes the client sends,
//  a daemon-reported error, and the subscribe push stream. This is the closest
//  thing to a live daemon without running one.
//

import Darwin
import Foundation
import Testing

@testable import omni_macos

/// A minimal stand-in for the daemon: binds a Unix socket, and for each
/// connection reads one request line and replies with whatever `handler`
/// returns (one or more lines), then closes. `init` finishes binding and
/// listening before returning, so a client may connect immediately.
final class FakeDaemon: @unchecked Sendable {
    let path: String
    private let listenFd: Int32
    private let queue = DispatchQueue(label: "fake.daemon")
    private let handler: @Sendable (String) -> [String]

    private let lock = NSLock()
    private var _requests: [String] = []
    var requests: [String] {
        lock.lock(); defer { lock.unlock() }
        return _requests
    }

    init(handler: @escaping @Sendable (String) -> [String]) throws {
        self.handler = handler
        self.path = "/tmp/omni-ipc-test-\(UUID().uuidString.prefix(8)).sock"
        unlink(path)

        listenFd = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        precondition(listenFd >= 0, "socket() failed")

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = Array(path.utf8)
        withUnsafeMutablePointer(to: &address.sun_path) { tuplePtr in
            tuplePtr.withMemoryRebound(to: CChar.self, capacity: pathBytes.count + 1) { dst in
                for (i, b) in pathBytes.enumerated() { dst[i] = CChar(bitPattern: b) }
                dst[pathBytes.count] = 0
            }
        }
        let size = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bound = withUnsafePointer(to: &address) { p in
            p.withMemoryRebound(to: sockaddr.self, capacity: 1) { Darwin.bind(listenFd, $0, size) }
        }
        precondition(bound == 0, "bind() failed: \(String(cString: strerror(errno)))")
        precondition(Darwin.listen(listenFd, 16) == 0, "listen() failed")

        queue.async { [self] in acceptLoop() }
    }

    private func acceptLoop() {
        while true {
            let clientFd = Darwin.accept(listenFd, nil, nil)
            if clientFd < 0 { return }  // listener closed
            if let line = readLine(clientFd) {
                lock.lock(); _requests.append(line); lock.unlock()
                for reply in handler(line) {
                    writeLine(clientFd, reply)
                }
            }
            Darwin.close(clientFd)
        }
    }

    private func readLine(_ fd: Int32) -> String? {
        var bytes: [UInt8] = []
        var chunk = [UInt8](repeating: 0, count: 1024)
        while true {
            let n = chunk.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress, $0.count) }
            if n <= 0 { return bytes.isEmpty ? nil : String(decoding: bytes, as: UTF8.self) }
            for i in 0..<n {
                if chunk[i] == 0x0A { return String(decoding: bytes, as: UTF8.self) }
                bytes.append(chunk[i])
            }
        }
    }

    private func writeLine(_ fd: Int32, _ line: String) {
        var bytes = Array(line.utf8)
        bytes.append(0x0A)
        _ = bytes.withUnsafeBytes { Darwin.write(fd, $0.baseAddress, $0.count) }
    }

    func stop() {
        Darwin.close(listenFd)
        unlink(path)
    }
}

struct UnixSocketDaemonClientTests {
    @Test func helloRoundTrips() async throws {
        let daemon = try FakeDaemon { _ in
            [#"{"result":"hello","protocol_version":1,"daemon_version":"0.4.0"}"#]
        }
        defer { daemon.stop() }
        let client = UnixSocketDaemonClient(socketPath: daemon.path)

        let hello = try await client.hello()
        #expect(hello.protocolVersion == 1)
        #expect(hello.daemonVersion == "0.4.0")
    }

    @Test func statusRoundTrips() async throws {
        let daemon = try FakeDaemon { _ in
            [#"{"result":"status","fingerprint":"abc","port":4733,"capturing":true,"clipboard_sharing":true,"sessions":[],"pending":[]}"#]
        }
        defer { daemon.stop() }
        let client = UnixSocketDaemonClient(socketPath: daemon.path)

        let status = try await client.status()
        #expect(status.fingerprint == "abc")
        #expect(status.port == 4733)
        #expect(status.clipboardSharing == true)
    }

    @Test func connectSendsTheExpectedRequestAndAcceptsOk() async throws {
        let daemon = try FakeDaemon { _ in [#"{"result":"ok"}"#] }
        defer { daemon.stop() }
        let client = UnixSocketDaemonClient(socketPath: daemon.path)

        try await client.connect(host: "10.0.0.2:4733")

        let request = try #require(daemon.requests.first)
        let obj = try #require(
            JSONSerialization.jsonObject(with: Data(request.utf8)) as? [String: Any])
        #expect(obj["cmd"] as? String == "connect")
        #expect(obj["host"] as? String == "10.0.0.2:4733")
    }

    @Test func daemonErrorBecomesAThrownError() async throws {
        let daemon = try FakeDaemon { _ in [#"{"result":"error","message":"peer not found"}"#] }
        defer { daemon.stop() }
        let client = UnixSocketDaemonClient(socketPath: daemon.path)

        await #expect(throws: OmniDaemonError("peer not found")) {
            try await client.disconnect(host: "ghost")
        }
    }

    @Test func missingDaemonReportsNotRunning() async {
        let client = UnixSocketDaemonClient(socketPath: "/tmp/omni-does-not-exist-\(UUID().uuidString).sock")
        await #expect(throws: OmniDaemonError.self) {
            _ = try await client.status()
        }
    }

    @Test func subscribeYieldsPushedSnapshotsUntilTheDaemonCloses() async throws {
        let daemon = try FakeDaemon { request in
            // Only the subscribe connection gets the event stream.
            guard request.contains("subscribe") else { return [] }
            return [
                #"{"event":"status","fingerprint":"one","port":4733,"sessions":[],"pending":[]}"#,
                #"{"event":"status","fingerprint":"two","port":4733,"sessions":[],"pending":[]}"#,
            ]
        }
        defer { daemon.stop() }
        let client = UnixSocketDaemonClient(socketPath: daemon.path)

        var fingerprints: [String] = []
        for try await snapshot in client.subscribe() {
            fingerprints.append(snapshot.fingerprint)
        }
        #expect(fingerprints == ["one", "two"])
    }
}
