//
//  ProtocolTests.swift
//  omni-macosTests
//
//  Locks this client's wire format to the daemon's. The JSON shapes asserted
//  here are exactly what `crates/omni-runtime/src/ipc.rs` serializes; if the
//  daemon's protocol changes, these tests must change with it.
//

import Foundation
import Testing

@testable import omni_macos

struct ProtocolTests {
    /// Parse an encoded request line back into a dictionary so assertions are
    /// independent of JSON key order.
    private func object(_ request: Request) throws -> [String: Any] {
        let line = try ProtocolCodec.encode(request)
        #expect(!line.contains("\n"))
        let data = Data(line.utf8)
        return try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    // MARK: - Requests

    @Test func helloEncodesItsTag() throws {
        let obj = try object(.hello)
        #expect(obj["cmd"] as? String == "hello")
    }

    @Test func connectCarriesHost() throws {
        let obj = try object(.connect(host: "10.0.0.2:4733"))
        #expect(obj["cmd"] as? String == "connect")
        #expect(obj["host"] as? String == "10.0.0.2:4733")
    }

    @Test func removePeerUsesSnakeCaseTagAndSelector() throws {
        let obj = try object(.removePeer(selector: "laptop"))
        #expect(obj["cmd"] as? String == "remove_peer")
        #expect(obj["selector"] as? String == "laptop")
    }

    @Test func clipboardCarriesEnabledFlag() throws {
        let obj = try object(.clipboard(enabled: true))
        #expect(obj["cmd"] as? String == "clipboard")
        #expect(obj["enabled"] as? Bool == true)
    }

    @Test func layoutListSendsNullHostAndEdge() throws {
        // The daemon's Options serialize as JSON null when absent; matching that
        // is how "list placements" is told apart from "set placement".
        let obj = try object(.layout(host: nil, edge: nil))
        #expect(obj["cmd"] as? String == "layout")
        #expect(obj["host"] is NSNull)
        #expect(obj["edge"] is NSNull)
    }

    @Test func layoutSetCarriesHostAndEdge() throws {
        let obj = try object(.layout(host: "desktop", edge: "left"))
        #expect(obj["host"] as? String == "desktop")
        #expect(obj["edge"] as? String == "left")
    }

    // MARK: - Responses

    @Test func decodesOk() throws {
        #expect(try ProtocolCodec.decodeResponse(#"{"result":"ok"}"#) == .ok)
    }

    @Test func decodesError() throws {
        let response = try ProtocolCodec.decodeResponse(#"{"result":"error","message":"nope"}"#)
        #expect(response == .error(message: "nope"))
    }

    @Test func decodesHelloHandshake() throws {
        let line = #"{"result":"hello","protocol_version":1,"daemon_version":"0.4.0"}"#
        let response = try ProtocolCodec.decodeResponse(line)
        #expect(response == .hello(protocolVersion: 1, daemonVersion: "0.4.0"))
    }

    @Test func decodesFlattenedStatus() throws {
        // The `status` variant flattens StatusInfo alongside the "result" tag.
        let line = """
            {"result":"status","fingerprint":"abc","port":4733,"capturing":true,\
            "clipboard_sharing":false,\
            "sessions":[{"host":"10.0.0.2","fingerprint":"cd","role":"controller","active":true}],\
            "pending":[{"host":"10.0.0.3","fingerprint":"ef"}]}
            """
        guard case let .status(info) = try ProtocolCodec.decodeResponse(line) else {
            Issue.record("expected a status response")
            return
        }
        #expect(info.fingerprint == "abc")
        #expect(info.port == 4733)
        #expect(info.capturing == true)
        #expect(info.clipboardSharing == false)
        #expect(info.sessions == [SessionInfo(host: "10.0.0.2", fingerprint: "cd", role: "controller", active: true)])
        #expect(info.pending == [PendingInfo(host: "10.0.0.3", fingerprint: "ef")])
    }

    @Test func statusDefaultsMissingOptionalFlagsToFalse() throws {
        // `capturing` / `clipboard_sharing` carry #[serde(default)] on the daemon,
        // so an older daemon may omit them; they must decode as false, not throw.
        let line = #"{"result":"status","fingerprint":"abc","port":4733,"sessions":[],"pending":[]}"#
        guard case let .status(info) = try ProtocolCodec.decodeResponse(line) else {
            Issue.record("expected a status response")
            return
        }
        #expect(info.capturing == false)
        #expect(info.clipboardSharing == false)
    }

    @Test func decodesPeers() throws {
        let line = #"{"result":"peers","peers":[{"host":"mac","fingerprint":"ab","connected":true}]}"#
        let response = try ProtocolCodec.decodeResponse(line)
        #expect(response == .peers([PeerInfo(host: "mac", fingerprint: "ab", connected: true)]))
    }

    @Test func decodesPeerWithNullHost() throws {
        let line = #"{"result":"peers","peers":[{"host":null,"fingerprint":"ab","connected":false}]}"#
        let response = try ProtocolCodec.decodeResponse(line)
        #expect(response == .peers([PeerInfo(host: nil, fingerprint: "ab", connected: false)]))
    }

    @Test func decodesLayout() throws {
        let line = #"{"result":"layout","placements":[{"host":"mac","edge":"left","connected":true}]}"#
        let response = try ProtocolCodec.decodeResponse(line)
        #expect(response == .layout([LayoutInfo(host: "mac", edge: "left", connected: true)]))
    }

    @Test func unreadableResponseBecomesAClearError() {
        #expect(throws: OmniDaemonError.self) {
            _ = try ProtocolCodec.decodeResponse("not json")
        }
    }

    // MARK: - Events

    @Test func decodesStatusEventTaggedAsEvent() throws {
        let line = #"{"event":"status","fingerprint":"abc","port":4733,"sessions":[],"pending":[]}"#
        let event = try ProtocolCodec.decodeEvent(line)
        guard case let .status(info) = event else {
            Issue.record("expected a status event")
            return
        }
        #expect(info.fingerprint == "abc")
    }
}
