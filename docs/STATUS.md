# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-06-10._

## Where we are

The project is **feature-complete for a first end-to-end build**. Every crate
is implemented: the shared-kernel **Protocol**, **Topology** (virtual desktop
and edge crossings), **Security** (allowlist + TOFU trust policy), **Session**
(lifecycle, roles, input routing), **Input** (real macOS and Linux adapters),
**Transport** (real QUIC adapter), the **Runtime** daemon that wires them all
together, and the **CLI** (`omni`) that drives it over local IPC. The full
pipeline — capture → route → QUIC datagram → inject, with TOFU handshake and
accept/reject flow — exists in code and compiles on macOS; what it has *not*
had yet is a live two-machine run (see "Not yet done").

The secure channel is **QUIC** (TLS 1.3 over UDP), via `quinn` + `rustls` —
adopted in place of the originally planned DTLS 1.3 (see "Open decisions").

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test` (95 tests).

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages (incl. screen sizes and `CursorWarp`), and the postcard wire codec. 16 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-security`  | **Implemented** | Allowlist + TOFU trust policy, `TrustStore`/`CertProvider` ports, self-signed identity generation. 15 tests. |
| `omni-session`   | **Implemented** | Session lifecycle, dynamic roles, active-target routing, `SessionEvents` port. 12 tests. |
| `omni-input`     | **Implemented** | Ports, in-memory adapters, and the real OS adapters: macOS (CGEvent tap + post) and Linux (evdev + uinput). 13 tests. |
| `omni-transport` | **Implemented** | `SecureChannel` port, framing, loopback channel, and the real QUIC adapter (quinn + rustls, mTLS, TOFU verifiers, datagrams + control stream). 12 tests. |
| `omni-runtime`   | **Implemented** | The daemon: config/paths, persistent identity, trust store, rate limiter, IPC types, and the composition root that runs the pipelines. 14 tests. |
| `omni-cli`       | **Implemented** | The full `omni` binary: start/stop/status, connect/disconnect, accept/reject, peers (+ remove), uninstall, over the daemon's Unix socket. |

### What `omni-protocol` provides

- **Identifiers** (`ids`): `MachineId`, `PeerId`, `SessionId`, and `Fingerprint`
  (a 32-byte SHA-256 digest that renders as lowercase hex for TOFU pinning).
- **Input events** (`input`): a platform-neutral `InputEvent` with `Key`,
  `Motion`, `Button`, and `Scroll` variants; `KeyCode` (USB HID usage codes),
  packed `Modifiers`, `MouseButton`, `MouseDelta`, `ScrollDelta`.
- **Control messages** (`control`): `ControlMessage` (`ConnectRequest`, `Accept`,
  `Reject`, `Disconnect`, `Heartbeat`, `CursorWarp`) and `RejectReason`.
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

### What `omni-runtime` provides

- **Paths & config** (`config`): everything lives in one directory
  (`~/.config/omnipresent` on Linux, `~/Library/Application Support/omnipresent`
  on macOS): `config.json` (UDP port, default 4733; optional screen-size
  override), certificate + key, `trust.json`, the IPC socket, and the log.
- **Identity** (`identity`): generates a self-signed certificate on first run
  (via `rcgen`), persists it with `0600` permissions, reloads it afterwards —
  so the machine's fingerprint is stable across restarts.
- **Trust store** (`trust`): the persistent TOFU store behind Security's
  policy and Transport's `HandshakePolicy` — thread-safe, JSON-backed, used by
  both the QUIC verifiers (reject unknown/changed certs at the handshake) and
  the accept/reject flow.
- **Rate limiter** (`ratelimit`): a token bucket capping injected events per
  session (default 2 000 events/s, burst 4 000) so a misbehaving peer cannot
  flood the local OS.
- **IPC types** (`ipc`): the JSON-lines request/response protocol the CLI
  speaks over the Unix socket (status, connect, accept, peers, ...).
- **The daemon** (`daemon`): the composition root. A capture thread polls the
  OS input source and advances the virtual cursor through Topology; an edge
  crossing flips Session's active target, suppresses local input, and warps
  the peer's cursor to the entry point. Input events ride unreliable QUIC
  datagrams to the active peer; signalling (connect/accept/disconnect/warp)
  rides the reliable control stream. Each peer connection runs in its own
  task; incoming requests from unknown peers wait (up to 120 s) for
  `omni accept`, trusted peers are auto-accepted. The Unix socket (mode 0600)
  serves the CLI.

### What `omni-cli` provides

The complete `omni` surface from the README: `start` (spawns the daemon
detached via a hidden `daemon` subcommand and waits for the socket), `stop`,
`status` (fingerprint, port, sessions with the input-here marker, pending
requests), `connect` / `disconnect <host>`, `accept` / `reject
<host|fingerprint>`, `peers` / `peers remove <host>`, and `uninstall` (stops
the daemon, removes the config dir, deletes the binary). Each command is one
JSON line to the daemon and one line back; errors land on stderr with a
non-zero exit.

## Tooling & dependencies

- Rust workspace, edition 2024, resolver 3.
- Third-party deps pinned once in `[workspace.dependencies]`: `serde`, `postcard`,
  the network/crypto stack (`quinn`, `rustls` + `ring`, `rcgen`, `sha2`, `tokio`,
  `bytes`), and the daemon/CLI layer (`clap`, `serde_json`, `dirs`, `rand`,
  `tracing` + `tracing-subscriber`).
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

## Not yet done

Everything below is known, deliberate, and ordered roughly by importance:

- **Live two-machine validation.** The full pipeline compiles and its parts are
  unit-tested, but `omni connect` between two real machines (macOS ↔ Linux) has
  not been exercised yet. This is the next step, and will likely shake out
  small bugs in edge-crossing feel, warp coordinates, and reconnect behaviour.
- **Layout is fixed, not configurable.** The daemon places a peer you dial past
  your **right** edge and a peer that dials you past your **left** edge. There
  is no `omni layout` command to choose edges, stack machines vertically, or
  arrange more than a simple left/right chain. Topology supports all of it;
  the configuration surface doesn't exist yet.
- **Heartbeats are defined but unused.** `ControlMessage::Heartbeat` exists in
  the protocol, but the daemon neither sends them nor times out a silent peer —
  a dead connection is noticed only when QUIC itself gives up.
- **No daemon-level integration test.** The runtime's support modules (config,
  identity, trust, rate limiter, IPC types) are tested; `daemon.rs` itself —
  the task wiring and the IPC loop — is exercised only by hand. A
  two-daemon-in-one-process or socket-level test is wanted.
- **`omni start` is plain detach.** No launchd/systemd service files yet, so
  the daemon does not survive a reboot or restart on crash.
- **Linux is compile-checked, not hardware-tested.** The evdev/uinput adapters
  and the daemon build for Linux but have not run against real devices.
- **CI**: a GitHub Actions workflow running fmt + clippy + test (currently
  these run only locally).
- **Cursor hiding on the inactive machine** while input is routed away (today
  the remote cursor moves, but the local one merely stops — it is suppressed,
  not hidden).
- **Clipboard sharing** — out of scope for now; will be opt-in when it comes.

## Open decisions

- **Secure channel: decided — QUIC** (TLS 1.3 over UDP) via `quinn` + `rustls`,
  replacing the originally planned DTLS 1.3. Rationale: no production-ready
  *pure-Rust* DTLS 1.3 + mTLS exists (rustls has no DTLS; `rusty-dtls` is PSK-only;
  the webrtc `dtls` crate is 1.2 only; wolfSSL/OpenSSL mean a C dependency). QUIC
  keeps every required property (UDP-only, mutual cert auth, TOFU, anti-replay,
  modern crypto), carries input over unreliable datagrams (RFC 9221), and has the
  most mature pure-Rust implementation.
- **Local IPC: decided — Unix domain socket** in the config directory, mode
  0600, JSON lines (one request, one response). Simple, debuggable with `nc`,
  and access-controlled by file permissions.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now).
