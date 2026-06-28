# Omnipresent — Windows client (WinUI 3)

A native Windows GUI for the Omnipresent daemon, built with **C# / WinUI 3**
(Fluent design). It is a thin client of the daemon's local IPC — it renders state
and sends commands, and holds no business logic. See
[`../../docs/NATIVE_INTEGRATIONS.md`](../../docs/NATIVE_INTEGRATIONS.md) for the
design and the binding constraints.

> This project lives **outside** the Cargo workspace and has its own toolchain
> (.NET) and CI job. It is the only part of Omnipresent that is not Rust — the
> documented GUI-only exception.

## Layout

| Project              | What it is                                                                 |
| -------------------- | -------------------------------------------------------------------------- |
| `src/Omni.Ipc`       | The thin IPC client: protocol DTOs, JSON (matching the daemon's serde), the named-pipe transport, and the stable pipe-name derivation. No UI. |
| `src/Omni.App.Core`  | View models (MVVM). Plain .NET — no WinUI — so they are unit-testable without a UI thread. |
| `src/Omni.App`       | The WinUI 3 app: window + views, bound to the view models.                 |
| `tests/Omni.Ipc.Tests`  | Unit tests for the protocol (de)serialization and the pipe client (over a real named pipe). |
| `tests/Omni.App.Tests`  | Unit tests for the view models against a fake client.                   |

## Prerequisites

- **.NET SDK 10** (`dotnet --version` ≥ 10). On this machine it is managed by
  [mise](https://mise.jdx.dev/); make sure the mise shims are on `PATH` so
  `dotnet` resolves to the SDK and not the bare runtime.

## Build & test

```sh
cd clients/omni-windows

# Run the tests (the IPC client, the protocol, and the view models).
dotnet test tests/Omni.Ipc.Tests/Omni.Ipc.Tests.csproj
dotnet test tests/Omni.App.Tests/Omni.App.Tests.csproj

# Build the app. WinUI needs an explicit platform and runtime identifier.
dotnet build src/Omni.App/Omni.App.csproj -p:Platform=x64 -r win-x64
```

The app is **unpackaged** and bundles the **Windows App SDK**
(`WindowsAppSDKSelfContained`). The built executable is at
`src/Omni.App/bin/x64/Debug/net10.0-windows10.0.19041.0/win-x64/Omni.App.exe`.

### Running it

The default build is **framework-dependent** for the .NET runtime, so the exe
needs the **.NET 10 Desktop Runtime** to be discoverable at launch. Two ways:

```sh
# Dev: point the apphost at the .NET 10 that mise manages (it carries the runtime).
DOTNET_ROOT="$LOCALAPPDATA/mise/dotnet-root" ./src/Omni.App/bin/.../win-x64/Omni.App.exe

# Standalone: publish self-contained so it bundles the runtime and just runs
# anywhere (this is what end users get).
dotnet publish src/Omni.App/Omni.App.csproj -c Release -r win-x64 --self-contained
```

> If you launch the framework-dependent exe with only an older runtime on the box,
> it exits with *"You must install or update .NET to run this application"* — set
> `DOTNET_ROOT` or publish self-contained.

## How it finds the daemon

The daemon exposes a Windows named pipe whose name is a stable SHA-256 of its
state directory (`%APPDATA%\omni` by default, or `%OMNI_CONFIG_DIR%`). The client
reproduces that derivation in `OmniPaths` — the Rust side
(`crates/omni-runtime/src/config.rs`) and the C# side assert the **same** known
vector in their tests, so the two never drift.

## What works today

Live status (push, no polling), incoming-request **accept/reject** showing name +
fingerprint, connect/disconnect, peers + forget, layout (list and place past an
edge), clipboard opt-in toggle, and an "incompatible daemon, please update"
notice from the protocol-version handshake.

## Not done yet

- **Tray / menu-bar entry** for the accept prompt when the window is closed
  (constraint 10). Currently the prompt shows in the window.
- **mDNS discovery and pairing codes** — these are daemon-side (Phase 2) and not
  yet exposed over the IPC, so connecting is by host/address for now.
- **`doctor` in the UI** — needs a new IPC request first.
- **Packaging** (MSIX, signing) and **UI-automation tests** (WinAppDriver).
