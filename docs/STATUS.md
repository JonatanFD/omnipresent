# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-06-09._

## Where we are

The project is in early **foundation** stage. The Cargo workspace and all
bounded-context crates exist and compile. Four are implemented and tested: the
shared-kernel **Protocol** crate, the **Topology** crate (virtual desktop and
edge crossings), the **Security** crate (allowlist + TOFU trust policy), and the
**Session** crate (lifecycle, roles, and input routing). **Input** has its ports
and in-memory adapters; its real per-OS adapters are still to come. **Transport**,
**Runtime**, and **CLI** remain documented placeholders. Nothing connects two
machines yet.

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test`.

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages, and the postcard wire codec. 15 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-security`  | **Implemented** | Allowlist + TOFU trust policy, `TrustStore`/`CertProvider` ports. 13 tests. |
| `omni-session`   | **Implemented** | Session lifecycle, dynamic roles, active-target routing, `SessionEvents` port. 12 tests. |
| `omni-input`     | **Ports + test adapter** | `InputSource`/`InputSink` ports and in-memory adapters. 5 tests. Real OS adapters pending. |
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

### What `omni-security` provides

- **Trust policy** (`trust`): `AllowList`, `PeerIdentity`, and a pure `evaluate`
  function returning a `TrustDecision` — allowlist gate first, then TOFU (unseen
  → `TrustOnFirstUse`, matching pin → `Trusted`, changed pin →
  `FingerprintMismatch`). `TrustAuthority` applies it against a store and records
  approvals (`accept`/`forget`).
- **Store** (`store`): the `TrustStore` port (allowlist + pinned fingerprints)
  plus an in-memory adapter.
- **Identity** (`identity`): the `CertProvider` port and `LocalIdentity`, whose
  `Debug` redacts key and certificate bytes so material never leaks into logs.
  Real certificate/DTLS cryptography is deferred to Transport's adapter.

### What `omni-session` provides

- **Sessions and roles** (`session`): `Role` (reversible Controller/Target),
  `Session`, `ActiveTarget` (`Local` vs `Remote(peer)`), and `SessionManager` —
  establishes and closes sessions, reverses roles, and switches the active target
  in response to Topology `Crossing`s (crossing onto a peer routes input there;
  crossing back home routes it local). Target-change events are deduplicated.
- **Events** (`events`): the `SessionEvents` port (lifecycle, role, and
  active-target changes) plus a recording adapter for tests.

### What `omni-input` provides

- **Ports** (`port`): `InputSource` (non-blocking `poll` to capture) and
  `InputSink` (`inject` to synthesize), each with an associated error type so
  real OS adapters can report failures.
- **In-memory adapters** (`memory`): `QueuedSource` replays a scripted sequence
  of events; `RecordingSink` records what is injected. Together they stand in for
  hardware and exercise the capture→send and receive→inject pipelines.
- **Pending:** the real per-OS adapters (macOS CGEvent/IOKit, Linux evdev/uinput)
  — see "What's next".

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

1. **`omni-transport`** — the UDP socket plus DTLS 1.3 channel, framing Protocol
   messages and enforcing the replay window. Requires choosing a DTLS-capable
   Rust stack (research per the "latest libraries" rule before implementing).
2. **`omni-runtime`** — wire every adapter into the ports, drive the
   capture→route→send and receive→inject pipelines, expose local IPC for the CLI,
   and apply least-privilege startup.
3. **`omni-cli`** — flesh out the `omni` subcommands against the Runtime IPC
   surface.

Cross-cutting, can come at any point:

- **`omni-input` real OS adapters** — macOS (CGEvent/IOKit) and Linux
  (evdev/uinput) implementations of `InputSource`/`InputSink`, with the
  least-privilege model from `CLAUDE.md`. Deferred because they need platform
  APIs and live hardware to exercise.
- **CI**: a GitHub Actions workflow running fmt + clippy + test (currently these
  run only locally).
- **DTLS stack selection**: the single biggest open technical decision; it shapes
  Transport and Security and should be researched before step 1.

## Open decisions

- Which Rust DTLS 1.3 stack to standardize on (affects Transport + Security).
- Local IPC mechanism for CLI↔daemon (e.g. Unix domain socket) — to be fixed when
  Runtime starts.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now).
