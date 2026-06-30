//
//  UnixSocketDaemonClient.swift
//  omni-macos
//
//  The real `DaemonClient`: speaks JSON lines over the daemon's Unix-domain
//  socket. Each command opens a short-lived connection (like the `omni` CLI);
//  `subscribe` keeps one open for the push stream. Blocking POSIX socket I/O
//  runs on a private queue and is bridged to async.
//

import Darwin
import Foundation

/// One connection to the daemon's socket: a connected `AF_UNIX` stream with
/// line-oriented read/write. Not safe to read from two threads at once, but
/// `close()` may be called from another thread to unblock a pending read (used
/// to tear down a subscription).
nonisolated final class DaemonConnection: @unchecked Sendable {
    private let fd: Int32
    private let lock = NSLock()
    private var closed = false
    private var buffer: [UInt8] = []

    /// Opens and connects to the socket at `path`. `readTimeout` (seconds) caps
    /// how long a read blocks; pass 0 for no timeout (used by the push stream,
    /// which blocks waiting for the next event).
    init(path: String, readTimeout: TimeInterval) throws {
        let maxPathLength = MemoryLayout.size(ofValue: sockaddr_un().sun_path)
        let pathBytes = Array(path.utf8)
        guard pathBytes.count < maxPathLength else {
            throw OmniDaemonError("daemon socket path is too long for a Unix socket: \(path)")
        }

        let descriptor = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else {
            throw OmniDaemonError("could not create a socket: \(DaemonConnection.errnoText())")
        }

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutablePointer(to: &address.sun_path) { tuplePtr in
            tuplePtr.withMemoryRebound(to: CChar.self, capacity: maxPathLength) { dst in
                for (i, byte) in pathBytes.enumerated() { dst[i] = CChar(bitPattern: byte) }
                dst[pathBytes.count] = 0
            }
        }

        let size = socklen_t(MemoryLayout<sockaddr_un>.size)
        let result = withUnsafePointer(to: &address) { addrPtr in
            addrPtr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                Darwin.connect(descriptor, sa, size)
            }
        }
        guard result == 0 else {
            Darwin.close(descriptor)
            throw OmniDaemonError("the omni daemon is not running (no socket to connect to)")
        }

        if readTimeout > 0 {
            var tv = timeval(
                tv_sec: Int(readTimeout),
                tv_usec: Int32((readTimeout - Double(Int(readTimeout))) * 1_000_000))
            setsockopt(
                descriptor, SOL_SOCKET, SO_RCVTIMEO, &tv,
                socklen_t(MemoryLayout<timeval>.size))
        }
        self.fd = descriptor
    }

    /// Writes one line followed by a newline.
    func write(line: String) throws {
        var bytes = Array(line.utf8)
        bytes.append(0x0A)
        var offset = 0
        while offset < bytes.count {
            let written = bytes[offset...].withUnsafeBytes { raw in
                Darwin.write(fd, raw.baseAddress, raw.count)
            }
            if written < 0 {
                if errno == EINTR { continue }
                throw OmniDaemonError("could not send to the daemon: \(DaemonConnection.errnoText())")
            }
            if written == 0 {
                throw OmniDaemonError("the daemon closed the connection")
            }
            offset += written
        }
    }

    /// Reads the next newline-delimited line, or nil at end of stream. A line is
    /// returned without its trailing newline.
    func readLine() throws -> String? {
        while true {
            if let newline = buffer.firstIndex(of: 0x0A) {
                let line = Array(buffer[..<newline])
                buffer.removeSubrange(...newline)
                return String(decoding: line, as: UTF8.self)
            }

            var chunk = [UInt8](repeating: 0, count: 4096)
            let count = chunk.withUnsafeMutableBytes { raw in
                Darwin.read(fd, raw.baseAddress, raw.count)
            }
            if count < 0 {
                if errno == EINTR { continue }
                throw OmniDaemonError("could not read from the daemon: \(DaemonConnection.errnoText())")
            }
            if count == 0 {
                // End of stream: flush any trailing unterminated bytes once.
                if buffer.isEmpty { return nil }
                let rest = String(decoding: buffer, as: UTF8.self)
                buffer.removeAll()
                return rest
            }
            buffer.append(contentsOf: chunk[0..<count])
        }
    }

    func close() {
        lock.lock()
        defer { lock.unlock() }
        guard !closed else { return }
        closed = true
        Darwin.close(fd)
    }

    private static func errnoText() -> String {
        String(cString: strerror(errno))
    }
}

/// The production `DaemonClient`, over the daemon's Unix-domain socket.
public nonisolated final class UnixSocketDaemonClient: DaemonClient {
    private let socketPath: String
    private let requestTimeout: TimeInterval
    private let ioQueue = DispatchQueue(
        label: "com.omnipresent.ipc", qos: .userInitiated, attributes: .concurrent)

    /// - Parameters:
    ///   - socketPath: the daemon socket; defaults to the running daemon's.
    ///   - requestTimeout: how long a one-shot request waits for an answer.
    public init(
        socketPath: String = OmniPaths.socketPath(),
        requestTimeout: TimeInterval = 5
    ) {
        self.socketPath = socketPath
        self.requestTimeout = requestTimeout
    }

    public func hello() async throws -> HelloInfo {
        switch try await send(.hello) {
        case let .hello(version, daemon): return HelloInfo(protocolVersion: version, daemonVersion: daemon)
        case let .error(message): throw OmniDaemonError(message)
        case let other: throw OmniDaemonError.unexpected(other)
        }
    }

    public func status() async throws -> StatusInfo {
        switch try await send(.status) {
        case let .status(info): return info
        case let .error(message): throw OmniDaemonError(message)
        case let other: throw OmniDaemonError.unexpected(other)
        }
    }

    public func peers() async throws -> [PeerInfo] {
        switch try await send(.peers) {
        case let .peers(list): return list
        case let .error(message): throw OmniDaemonError(message)
        case let other: throw OmniDaemonError.unexpected(other)
        }
    }

    public func layout() async throws -> [LayoutInfo] {
        switch try await send(.layout(host: nil, edge: nil)) {
        case let .layout(list): return list
        case let .error(message): throw OmniDaemonError(message)
        case let other: throw OmniDaemonError.unexpected(other)
        }
    }

    public func connect(host: String) async throws { try await expectOk(.connect(host: host)) }
    public func disconnect(host: String) async throws { try await expectOk(.disconnect(host: host)) }
    public func accept(selector: String) async throws { try await expectOk(.accept(selector: selector)) }
    public func reject(selector: String) async throws { try await expectOk(.reject(selector: selector)) }
    public func removePeer(selector: String) async throws { try await expectOk(.removePeer(selector: selector)) }
    public func setLayout(host: String, edge: String) async throws { try await expectOk(.layout(host: host, edge: edge)) }
    public func setClipboard(enabled: Bool) async throws { try await expectOk(.clipboard(enabled: enabled)) }
    public func stop() async throws { try await expectOk(.stop) }

    public func subscribe() -> AsyncThrowingStream<StatusInfo, Error> {
        let path = socketPath
        let queue = ioQueue
        return AsyncThrowingStream { continuation in
            let connection: DaemonConnection
            do {
                connection = try DaemonConnection(path: path, readTimeout: 0)
                try connection.write(line: try ProtocolCodec.encode(.subscribe))
            } catch {
                continuation.finish(throwing: error)
                return
            }
            // Closing the connection unblocks the read loop below.
            continuation.onTermination = { _ in connection.close() }
            queue.async {
                do {
                    while let line = try connection.readLine() {
                        if line.isEmpty { continue }
                        if case let .status(info) = try ProtocolCodec.decodeEvent(line) {
                            continuation.yield(info)
                        }
                    }
                    continuation.finish()  // the daemon closed the connection
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    // MARK: - Transport

    private func expectOk(_ request: Request) async throws {
        switch try await send(request) {
        case .ok: return
        case let .error(message): throw OmniDaemonError(message)
        case let other: throw OmniDaemonError.unexpected(other)
        }
    }

    private func send(_ request: Request) async throws -> Response {
        let path = socketPath
        let timeout = requestTimeout
        let line = try ProtocolCodec.encode(request)
        return try await withCheckedThrowingContinuation { continuation in
            ioQueue.async {
                do {
                    let connection = try DaemonConnection(path: path, readTimeout: timeout)
                    defer { connection.close() }
                    try connection.write(line: line)
                    guard let reply = try connection.readLine() else {
                        throw OmniDaemonError("the daemon closed the connection without responding")
                    }
                    continuation.resume(returning: try ProtocolCodec.decodeResponse(reply))
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }
}
