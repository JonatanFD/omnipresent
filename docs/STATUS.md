# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-06-09._

## Where we are

The project is in early **foundation** stage. The Cargo workspace and all
bounded-context crates exist and compile. Four are fully implemented and tested:
the shared-kernel **Protocol** crate, the **Topology** crate (virtual desktop and
edge crossings), the **Security** crate (allowlist + TOFU trust policy), and the
**Session** crate (lifecycle, roles, and input routing). **Input** and
**Transport** have their ports and in-memory/loopback adapters; their real
adapters (per-OS input, and the QUIC connection) are still to come. **Runtime**
and **CLI** remain documented placeholders. Nothing connects two machines yet.

The secure channel is **QUIC** (TLS 1.3 over UDP), via `quinn` + `rustls` —
adopted in place of the originally planned DTLS 1.3 (see "Open decisions").

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test`.

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages, and the postcard wire codec. 15 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-security`  | **Implemented** | Allowlist + TOFU trust policy, `TrustStore`/`CertProvider` ports, self-signed identity generation. 15 tests. |
| `omni-session`   | **Implemented** | Session lifecycle, dynamic roles, active-target routing, `SessionEvents` port. 12 tests. |
| `omni-input`     | **Implemented** | Ports, in-memory adapters, and the real OS adapters: macOS (CGEvent tap + post) and Linux (evdev + uinput). 13 tests. |
| `omni-transport` | **Implemented** | `SecureChannel` port, framing, loopback channel, and the real QUIC adapter (quinn + rustls, mTLS, TOFU verifiers, datagrams + control stream). 12 tests. |
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
- **Handshake payloads**: `ConnectRequest` carries the initiator's screen size
  and `Accept` carries the target's machine id and screen size, so each side can
  place the other in its virtual desktop layout.

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
  Real certificate handling is deferred to Transport's QUIC adapter, which feeds
  this material into rustls and enforces TOFU via a custom certificate verifier.

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
- **Suppression**: `InputSource::set_suppressed` — while input is routed to a
  remote machine the source still reports events but withholds them from the
  local OS, so input never acts on two machines at once.
- **macOS adapters** (`macos`): `MacosSource` captures through a CGEvent tap on
  a dedicated run-loop thread (suppression drops the event before the OS acts;
  the tap re-enables itself if the OS disables it) and `MacosSink` injects with
  `CGEventPost`, stamping its events so the tap never re-captures them. Needs
  the Accessibility permission — never root. A `kVK ↔ HID` keymap covers the
  full ANSI layout.
- **Linux adapters** (`linux`): `LinuxSource` reads keyboards and mice from
  `/dev/input` (one thread per device; suppression = `EVIOCGRAB`), `LinuxSink`
  injects through a uinput virtual device that the capture side knows to skip.
  Needs only `input`-group membership — never root. A `KEY_* ↔ HID` keymap
  mirrors the macOS one. (Compile-checked against a Linux target; needs live
  hardware to exercise.)

### What `omni-transport` provides

- **Secure channel** (`channel`): the `SecureChannel` port — an established,
  per-peer connection that sends and receives datagram payloads. QUIC provides
  the cryptography, so the port deals only in already-protected bytes. A
  `LoopbackChannel` pair stands in for a real connection in tests.
- **Message framing** (`transport`): `Transport` encodes a Protocol `Message` and
  sends it as one (unreliable) datagram, and decodes received datagrams back,
  surfacing channel vs codec failures via `TransportError`.
- **QUIC adapter** (`quic` + `tls`, with the `policy` port): `QuicEndpoint` owns
  one UDP socket that both dials and listens (roles are dynamic), `QuicConnection`
  is the production `SecureChannel` (unreliable datagrams for input), and
  `ControlStream` frames signalling over one reliable bidirectional stream.
  Mutual TLS 1.3 is mandatory; custom rustls certificate verifiers enforce the
  `HandshakePolicy` port (implemented by the Runtime over Security's trust store),
  so an unauthorized or fingerprint-changed peer never completes the handshake.
  Exercised by live two-endpoint tests over localhost.

## Tooling & dependencies

- Rust workspace, edition 2024, resolver 3.
- Third-party deps pinned once in `[workspace.dependencies]`: `serde`, `postcard`,
  and the network/crypto stack (`quinn`, `rustls` + `ring`, `rcgen`, `sha2`,
  `tokio`, `bytes`).
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

1. **`omni-runtime`** — wire every adapter into the ports, drive the
   capture→route→send and receive→inject pipelines, expose local IPC for the CLI,
   and apply least-privilege startup.
2. **`omni-cli`** — flesh out the `omni` subcommands against the Runtime IPC
   surface.

Cross-cutting, can come at any point:

- **CI**: a GitHub Actions workflow running fmt + clippy + test (currently these
  run only locally).

## Open decisions

- **Secure channel: decided — QUIC** (TLS 1.3 over UDP) via `quinn` + `rustls`,
  replacing the originally planned DTLS 1.3. Rationale: no production-ready
  *pure-Rust* DTLS 1.3 + mTLS exists (rustls has no DTLS; `rusty-dtls` is PSK-only;
  the webrtc `dtls` crate is 1.2 only; wolfSSL/OpenSSL mean a C dependency). QUIC
  keeps every required property (UDP-only, mutual cert auth, TOFU, anti-replay,
  modern crypto), carries input over unreliable datagrams (RFC 9221), and has the
  most mature pure-Rust implementation.
- Local IPC mechanism for CLI↔daemon (e.g. Unix domain socket) — to be fixed when
  Runtime starts.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now).
