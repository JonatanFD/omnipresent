# Project: omnipresent Control System Upgrades

## Architecture
The system utilizes a ports-and-adapters architecture:
- **Input**: OS-specific capture and injection.
- **Topology**: Virtual desktop layout and crossing logic.
- **Session**: Dynamic Controller/Target role management.
- **Security**: TOFU, mTLS trust authority.
- **Transport**: Reliable/unreliable streams/datagrams over QUIC.
- **Protocol**: postcard-encoded shared vocabulary.
- **Runtime**: Daemon composition root and IPC.
- **Clipboard (New Crate)**: Domain-driven clipboard monitor and synchronizer.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|---|---|---|---|
| 1 | Global Cursor Hiding | Windows, macOS, Linux cursor hiding on suppression. | None | DONE |
| 2 | Linux Cursor Sync | X11/Wayland coordinate query in `cursor_position()`. | M1 | DONE |
| 3 | `omni-clipboard` Crate | Crate design, TDD test cases, arboard adapter. | None | DONE |
| 4 | Clipboard Daemon Wiring | Config flags, QUIC control stream integration. | M3 | DONE |
| 5 | Integration Verification | Compile check, clippy, tests, layout verification. | M1, M2, M4 | DONE |

## Interface Contracts
### `omni-clipboard` ↔ `omni-runtime`
- Config structure updates: `clipboard_sharing_enabled: bool`.
- Event listener: `fn on_clipboard_changed(data: ClipboardData)`.
- IPC/Control message updates: `ClipboardPayload` containing Text or Image options.
- Wire serialization format: postcard envelope containing `InputEvent` or `ControlMessage` or `ClipboardData`.
