//
//  OmniPaths.swift
//  omni-macos
//
//  Locates the daemon's Unix-domain socket, reproducing the daemon's own
//  derivation (see `crates/omni-runtime/src/config.rs`) so this client finds it
//  without the daemon publishing an address.
//

import Foundation

public nonisolated enum OmniPaths {
    /// The daemon's state directory: `$OMNI_CONFIG_DIR` if set, otherwise the
    /// platform config directory with `omni` appended. On macOS that is
    /// `~/Library/Application Support/omni`, matching Rust's
    /// `dirs::config_dir()` (which maps to Application Support).
    public static func configDirectory(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> URL {
        if let override = environment["OMNI_CONFIG_DIR"], !override.isEmpty {
            return URL(fileURLWithPath: override, isDirectory: true)
        }
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory())
                .appendingPathComponent("Library/Application Support", isDirectory: true)
        return base.appendingPathComponent("omni", isDirectory: true)
    }

    /// The Unix-domain socket the daemon listens on (mode 0600): `daemon.sock`
    /// inside the state directory.
    public static func socketPath(
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> String {
        configDirectory(environment: environment)
            .appendingPathComponent("daemon.sock", isDirectory: false)
            .path
    }
}
