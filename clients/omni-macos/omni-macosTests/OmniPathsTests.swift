//
//  OmniPathsTests.swift
//  omni-macosTests
//
//  The daemon socket path must match the daemon's own derivation in
//  `crates/omni-runtime/src/config.rs`, or this client cannot find the daemon.
//

import Foundation
import Testing

@testable import omni_macos

struct OmniPathsTests {
    @Test func honorsConfigDirOverride() {
        let env = ["OMNI_CONFIG_DIR": "/tmp/omni-x"]
        #expect(OmniPaths.configDirectory(environment: env).path == "/tmp/omni-x")
        #expect(OmniPaths.socketPath(environment: env) == "/tmp/omni-x/daemon.sock")
    }

    @Test func ignoresEmptyOverride() {
        // An empty override is treated as unset, matching the daemon.
        let path = OmniPaths.socketPath(environment: ["OMNI_CONFIG_DIR": ""])
        #expect(path.hasSuffix("/Library/Application Support/omni/daemon.sock"))
    }

    @Test func defaultsToApplicationSupportOmni() {
        // On macOS the daemon's `dirs::config_dir()` is ~/Library/Application Support.
        let path = OmniPaths.socketPath(environment: [:])
        #expect(path.hasSuffix("/Library/Application Support/omni/daemon.sock"))
    }
}
