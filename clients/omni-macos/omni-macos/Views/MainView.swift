//
//  MainView.swift
//  omni-macos
//
//  The main window: a sidebar for navigation and a detail pane for each
//  section. Connect, Peers, and Layout are unified under "Connections"
//  because they all relate to managing peers and active sessions.
//

import SwiftUI

// MARK: - Sidebar navigation

private enum SidebarSection: String, CaseIterable, Identifiable, Hashable {
    case general     = "General"
    case connections = "Connections"
    case system      = "System"
    case update      = "Update"

    var id: Self { self }

    var systemImage: String {
        switch self {
        case .general:     "antenna.radiowaves.left.and.right"
        case .connections: "network"
        case .system:      "gearshape"
        case .update:      "arrow.down.circle"
        }
    }
}

// MARK: - Root

struct MainView: View {
    @Bindable var viewModel: DaemonViewModel
    @State private var selectedSection: SidebarSection? = .general

    var body: some View {
        NavigationSplitView {
            List(SidebarSection.allCases, selection: $selectedSection) { section in
                sidebarRow(section)
                    .tag(section)
            }
            .navigationTitle("Omnipresent")
            .listStyle(.sidebar)
        } detail: {
            switch selectedSection ?? .general {
            case .general:     GeneralDetailView(viewModel: viewModel)
            case .connections: ConnectionsDetailView(viewModel: viewModel)
            case .system:      SystemDetailView(viewModel: viewModel)
            case .update:      UpdateDetailView(viewModel: viewModel)
            }
        }
    }

    @ViewBuilder
    private func sidebarRow(_ section: SidebarSection) -> some View {
        if section == .general {
            HStack {
                Label(section.rawValue, systemImage: section.systemImage)
                Spacer()
                Circle()
                    .fill(daemonStatusColor)
                    .frame(width: 8, height: 8)
                    .help(viewModel.statusText)
            }
        } else {
            Label(section.rawValue, systemImage: section.systemImage)
                .badge(section == .connections ? viewModel.pending.count : 0)
        }
    }

    private var daemonStatusColor: Color {
        switch viewModel.connection {
        case .connected:    .green
        case .connecting:   .orange
        case .disconnected: .secondary
        case .incompatible: .orange
        }
    }
}

// MARK: - General

private struct GeneralDetailView: View {
    var viewModel: DaemonViewModel

    var body: some View {
        Form {
            if viewModel.isIncompatible {
                Section {
                    Label {
                        Text(viewModel.statusText)
                    } icon: {
                        Image(systemName: "exclamationmark.octagon.fill").foregroundStyle(.red)
                    }
                }
            } else {
                Section("Daemon") {
                    LabeledContent("Status") {
                        Label(viewModel.statusText, systemImage: connectionSymbol)
                            .foregroundStyle(viewModel.isConnected ? .primary : .secondary)
                    }
                    HStack(spacing: 8) {
                        Button("Start") { Task { await viewModel.startDaemon() } }
                            .disabled(viewModel.isConnected || viewModel.isIncompatible)
                        Button("Stop", role: .destructive) {
                            Task { await viewModel.stopDaemon() }
                        }
                        .disabled(!viewModel.isConnected)
                    }
                }

                if viewModel.isConnected {
                    Section("Info") {
                        LabeledContent("Input capture", value: viewModel.capturing ? "Active" : "Target only")
                        LabeledContent("Port", value: String(viewModel.port))
                        LabeledContent("Fingerprint") {
                            Text(viewModel.fingerprint)
                                .font(.system(.footnote, design: .monospaced))
                                .textSelection(.enabled)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                        if !viewModel.daemonVersion.isEmpty {
                            LabeledContent("Version", value: "v\(viewModel.daemonVersion)")
                        }
                    }
                }

                if viewModel.hasError {
                    Section {
                        Label(viewModel.lastError ?? "", systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(.orange)
                    }
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("General")
    }

    private var connectionSymbol: String {
        switch viewModel.connection {
        case .connected:    "checkmark.circle.fill"
        case .connecting:   "ellipsis.circle"
        case .disconnected: "xmark.circle"
        case .incompatible: "exclamationmark.octagon.fill"
        }
    }
}

// MARK: - Connections (Connect + Peers + Layout unified)

private struct ConnectionsDetailView: View {
    var viewModel: DaemonViewModel
    @State private var connectHost = ""
    private let edges = ["left", "right", "top", "bottom"]

    private var hasData: Bool {
        !viewModel.sessions.isEmpty || !viewModel.pending.isEmpty
        || !viewModel.peers.isEmpty || !viewModel.placements.isEmpty
    }

    var body: some View {
        if !viewModel.isConnected && !hasData {
            // Full-pane centered empty state — not inside a Form box.
            ContentUnavailableView(
                "Daemon Not Running",
                systemImage: "network.slash",
                description: Text("Start the daemon from General to connect to peers.")
            )
            .navigationTitle("Connections")
        } else {
            Form {
                if viewModel.isConnected {
                    connectSection
                }
                if !viewModel.pending.isEmpty  { incomingSection }
                if !viewModel.sessions.isEmpty { sessionsSection }
                if !viewModel.peers.isEmpty    { peersSection }
                if !viewModel.placements.isEmpty { layoutSection }
            }
            .formStyle(.grouped)
            .navigationTitle("Connections")
        }
    }

    // MARK: Subsections

    private var connectSection: some View {
        Section("Connect to host") {
            HStack(spacing: 8) {
                TextField("Host or IP address", text: $connectHost)
                    .onSubmit(submitConnect)
                Button("Connect", action: submitConnect)
                    .disabled(connectHost.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private var incomingSection: some View {
        Section("Incoming requests") {
            ForEach(viewModel.pending) { request in
                VStack(alignment: .leading, spacing: 6) {
                    Text(request.host).font(.headline)
                    Text(request.fingerprint)
                        .font(.system(.footnote, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    HStack {
                        Button("Accept") {
                            Task { await viewModel.accept(selector: request.fingerprint) }
                        }
                        .buttonStyle(.borderedProminent)
                        Button("Reject", role: .destructive) {
                            Task { await viewModel.reject(selector: request.fingerprint) }
                        }
                    }
                    .padding(.top, 2)
                }
                .padding(.vertical, 4)
            }
        }
    }

    private var sessionsSection: some View {
        Section("Active sessions") {
            ForEach(viewModel.sessions) { session in
                LabeledContent {
                    Button("Disconnect", role: .destructive) {
                        Task { await viewModel.disconnect(host: session.host) }
                    }
                } label: {
                    HStack(spacing: 6) {
                        if session.active {
                            Image(systemName: "dot.radiowaves.left.and.right")
                                .foregroundStyle(.green)
                        }
                        VStack(alignment: .leading, spacing: 1) {
                            Text(session.host)
                            Text(session.role.capitalized)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    private var peersSection: some View {
        Section("Known peers") {
            ForEach(viewModel.peers) { peer in
                LabeledContent {
                    Button("Forget", role: .destructive) {
                        Task {
                            await viewModel.removePeer(selector: peer.host ?? peer.fingerprint)
                        }
                    }
                } label: {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(peer.host ?? "(unnamed)")
                        Text(peer.fingerprint)
                            .font(.system(.caption2, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
            }
        }
    }

    private var layoutSection: some View {
        Section("Screen layout") {
            ForEach(viewModel.placements) { placement in
                Picker(placement.host, selection: edgeBinding(for: placement)) {
                    ForEach(edges, id: \.self) { edge in
                        Text(edge.capitalized).tag(edge)
                    }
                }
            }
        }
    }

    // MARK: Helpers

    private func submitConnect() {
        let host = connectHost.trimmingCharacters(in: .whitespaces)
        guard !host.isEmpty else { return }
        Task {
            await viewModel.connect(host: host)
            connectHost = ""
        }
    }

    private func edgeBinding(for placement: LayoutInfo) -> Binding<String> {
        Binding(
            get: { placement.edge },
            set: { edge in Task { await viewModel.setLayout(host: placement.host, edge: edge) } })
    }
}

// MARK: - System

private struct SystemDetailView: View {
    var viewModel: DaemonViewModel

    var body: some View {
        Form {
            Section("Clipboard") {
                Toggle("Share clipboard with connected peers", isOn: clipboardBinding)
                    .disabled(!viewModel.isConnected)
            }
        }
        .formStyle(.grouped)
        .navigationTitle("System")
    }

    private var clipboardBinding: Binding<Bool> {
        Binding(
            get: { viewModel.clipboardSharing },
            set: { enabled in Task { await viewModel.setClipboard(enabled: enabled) } })
    }
}

// MARK: - Update

private struct UpdateDetailView: View {
    var viewModel: DaemonViewModel
    @State private var isUpdating = false
    @State private var updateMessage: String?

    var body: some View {
        Form {
            if !viewModel.daemonVersion.isEmpty {
                Section {
                    LabeledContent("Installed version", value: "v\(viewModel.daemonVersion)")
                }
            }

            Section {
                Button {
                    Task { await runUpdate() }
                } label: {
                    if isUpdating {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text("Updating…")
                        }
                    } else {
                        Text("Update Omnipresent")
                    }
                }
                .disabled(isUpdating)

                if let message = updateMessage {
                    Text(message)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            } footer: {
                Text("Downloads and installs the latest release, then restarts the daemon.")
                    .font(.footnote)
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Update")
    }

    private func runUpdate() async {
        isUpdating = true
        updateMessage = nil

        // waitUntilExit() blocks the thread, so run off the main actor.
        let message: String = await Task.detached(priority: .userInitiated) {
            let candidates = [
                "/usr/local/bin/omni",
                "/opt/homebrew/bin/omni",
                "\(NSHomeDirectory())/.local/bin/omni",
                "\(NSHomeDirectory())/.cargo/bin/omni",
            ]
            guard let binaryPath = candidates.first(where: {
                FileManager.default.isExecutableFile(atPath: $0)
            }) else {
                return "Could not find the omni binary. Make sure Omnipresent is installed."
            }
            do {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: binaryPath)
                process.arguments = ["update"]
                try process.run()
                process.waitUntilExit()
                return process.terminationStatus == 0
                    ? "Update complete. The daemon will restart shortly."
                    : "Update exited with code \(process.terminationStatus)."
            } catch {
                return "Failed to run update: \(error.localizedDescription)"
            }
        }.value

        updateMessage = message
        isUpdating = false
    }
}
