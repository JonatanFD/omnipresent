//
//  MenuBarContent.swift
//  omni-macos
//
//  The menu-bar entry. It shows live status and, crucially, the accept/reject
//  prompt even when the window is closed — a TOFU decision point must not
//  depend on an open window. Window style (`.menuBarExtraStyle(.window)`) so it
//  can host buttons and a peer's fingerprint.
//

import SwiftUI

struct MenuBarContent: View {
    @Bindable var viewModel: DaemonViewModel
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 6) {
                Image(systemName: viewModel.isConnected ? "checkmark.circle.fill" : "ellipsis.circle")
                    .foregroundStyle(viewModel.isConnected ? .green : .secondary)
                Text(viewModel.statusText).font(.headline)
            }

            if !viewModel.pending.isEmpty {
                Divider()
                Text("Incoming requests").font(.subheadline).foregroundStyle(.secondary)
                ForEach(viewModel.pending) { request in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(request.host).font(.body.weight(.medium))
                        Text(request.fingerprint)
                            .font(.system(.caption2, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                        HStack {
                            Button("Accept") { Task { await viewModel.accept(selector: request.fingerprint) } }
                            Button("Reject", role: .destructive) {
                                Task { await viewModel.reject(selector: request.fingerprint) }
                            }
                        }
                    }
                }
            } else if viewModel.isConnected {
                Text("\(viewModel.sessions.count) active session(s)")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Divider()
            Button("Open Omnipresent") { openWindow(id: "main") }
            Button("Quit") { NSApplication.shared.terminate(nil) }
        }
        .padding(12)
        .frame(width: 280)
    }
}
