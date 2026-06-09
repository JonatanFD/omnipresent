# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-06-09._

## Where we are

The project is in early **foundation** stage. The Cargo workspace and all
bounded-context crates exist and compile. Two are implemented and tested: the
shared-kernel **Protocol** crate and the **Topology** crate (virtual desktop and
edge crossings). The remaining crates are documented placeholders with no
behaviour yet. Nothing connects two machines yet.

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test`.

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages, and the postcard wire codec. 15 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-input`     | Scaffold      | Crate + responsibility doc only. No ports or adapters yet.                    |
| `omni-session`   | Scaffold      | Crate + responsibility doc only.                                             |
| `omni-security`  | Scaffold      | Crate + responsibility doc only.                                             |
| `omni-transport` | Scaffold      | Crate + responsibility doc only.                                             |
| `omni-runtime`   | Scaffold      | Crate + responsibility doc only.                                             |
| `omni-cli`       | Scaffold      | `omni` binary prints "not yet implemented".                                  |

### What `omni-protocol` provides

- **Identifiers** (`ids`): `MachineId`, `PeerId`, `SessionId`, and `Fingerprint`
  (a 32-byte SHA-256 digest that renders as lowercase hex for TOFU pinning).
- **Input events** (`input`): a platform-neutral `InputEvent` with `Key`,
  `Motion`, `Button`, and `Scroll` variants; `KeyCode` (USB HID usage codes),
  packed `Modifiers`, `MouseButton`, `MouseDelta`, `ScrollDelta`.
- **Control messages** (`control`): `ControlMessage` (`ConnectRequest`, `Accept`,
  `Reject`, `Disconnect`, `Heartbeat`) and `RejectReason`.
- **Wire codec** (`wire`): the `Message` envelope plus `encode`/`decode` over
  [postcard](https://docs.rs/postcard) — a compact varint binary format chosen
  for small datagrams and low-latency (de)serialization. Truncated or empty
  input is rejected.

### What `omni-topology` provides

- **Geometry** (`geometry`): `Screen`, `Point`, and `Edge` (with `opposite` and
  orientation helpers).
- **Layout** (`layout`): `Machine` and `VirtualLayout` — an edge-link arrangement
  where each machine knows the neighbor past each edge (kept symmetric). `advance`
  moves the cursor by a `MouseDelta` and either stays on screen, clamps at a
  neighborless edge, or crosses onto the neighbor's opposite edge, mapping the
  position along the shared edge proportionally so crossings stay seamless across
  differently sized screens.
- **Store** (`store`): the `LayoutStore` port plus an in-memory adapter.

## Tooling & dependencies

- Rust workspace, edition 2024, resolver 3.
- Third-party deps pinned once in `[workspace.dependencies]`: `serde` 1.0,
  `postcard` 1.1.
- Quality gate per change: `cargo fmt --all`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test`.

## Workflow

Gitflow, local only (no remote yet):

- `master` — production.
- `develop` — integration. Protocol and the workspace scaffold are merged here.
- `feature/<name>` — one per unit of work (typically one crate), branched off
  `develop` and merged back with `--no-ff`.

Commits follow Conventional Commits. New behaviour is written test-first (TDD)
against ports using in-memory adapters.

## What's next

Crates are built in dependency order so each one can be tested against the layer
below without stubbing the layers above. Suggested sequence:

1. **`omni-security`** — trust policy as pure domain: `AllowList`, TOFU pinning
   and fingerprint-change rejection, `TrustDecision`. Define the `TrustStore` and
   `CertProvider` ports; keep real cert/key handling behind adapters.
2. **`omni-session`** — session lifecycle and dynamic Controller/Target roles,
   reacting to Topology crossings and connect/disconnect. Define `SessionEvents`.
3. **`omni-input`** — the `InputSource`/`InputSink` ports with an in-memory test
   adapter first, then per-OS adapters (macOS CGEvent/IOKit, Linux evdev/uinput).
   This is the first crate that touches the OS, so it needs the privilege model
   from `CLAUDE.md`.
4. **`omni-transport`** — the UDP socket plus DTLS 1.3 channel, framing Protocol
   messages and enforcing the replay window. Requires choosing a DTLS-capable
   Rust stack (research per the "latest libraries" rule before implementing).
5. **`omni-runtime`** — wire every adapter into the ports, drive the
   capture→route→send and receive→inject pipelines, expose local IPC for the CLI,
   and apply least-privilege startup.
6. **`omni-cli`** — flesh out the `omni` subcommands against the Runtime IPC
   surface.

Cross-cutting, can come at any point:

- **CI**: a GitHub Actions workflow running fmt + clippy + test (currently these
  run only locally).
- **DTLS stack selection**: the single biggest open technical decision; it shapes
  Transport and Security and should be researched before step 5.

## Open decisions

- Which Rust DTLS 1.3 stack to standardize on (affects Transport + Security).
- Local IPC mechanism for CLI↔daemon (e.g. Unix domain socket) — to be fixed when
  Runtime starts.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now).
