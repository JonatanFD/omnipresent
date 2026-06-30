//
//  omni_macosApp.swift
//  omni-macos
//
//  The app: a main window plus a menu-bar entry. The menu-bar entry surfaces
//  status and the accept/reject prompt even when the window is closed — a
//  security decision point cannot depend on an open window.
//

import SwiftUI

@main
struct OmniMacOSApp: App {
    @State private var viewModel = DaemonViewModel(client: UnixSocketDaemonClient())

    var body: some Scene {
        Window("Omnipresent", id: "main") {
            MainView(viewModel: viewModel)
                .task { viewModel.start() }
                .frame(minWidth: 660, minHeight: 460)
        }
        .windowResizability(.contentMinSize)

        MenuBarExtra {
            MenuBarContent(viewModel: viewModel)
                .task { viewModel.start() }
        } label: {
            Image(systemName: viewModel.pending.isEmpty ? "display.2" : "exclamationmark.triangle.fill")
        }
        .menuBarExtraStyle(.window)
    }
}
