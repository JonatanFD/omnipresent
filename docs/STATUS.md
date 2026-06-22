# Omnipresent — Project Status

A snapshot of what exists today and what comes next. For the *why* behind the
module boundaries, see [`ARCHITECTURE.md`](ARCHITECTURE.md); for product scope
and rules, see [`../CLAUDE.md`](../CLAUDE.md) and
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md).

_Last updated: 2026-06-22 (double-click now works when injecting on macOS. The
sink hardcoded each mouse event's `kCGMouseEventClickState` to 1, so a quick
two-click sequence from a remote controller (Windows → Mac) arrived as two
single clicks; it now tracks the previous click and stamps the running
single/double/triple count. Earlier the same day: input path tuned for latency
under network congestion. Cursor motion is sent as absolute positions on
unreliable
datagrams; on a congested or busy link a backlog of stale positions used to
queue and replay, so the remote cursor visibly lagged. The peer task now
**coalesces** a run of queued positions down to the most recent before sending
(clicks, keys, and scrolls are order-sensitive, so they are never dropped and
they break a run), quinn's datagram send buffer is shallow (it drops the oldest
stale positions to admit a fresh one), and the connection uses the **BBR**
congestion controller to keep the bottleneck queue — and the added latency —
small. Automatic reconnection on a dropped link is the next piece. Earlier the
same day: clipboard **image** sharing now works between two machines. Images
travel on the reliable control stream, but that stream capped
every frame at 64 KiB — fine for tiny signalling, far too small for a
screenshot's raw RGBA bytes — so the receiver rejected the frame as
`ControlFrameTooLarge` and tore the session down the moment an image was copied.
The control-frame limit now admits a full clipboard payload (`MAX_CLIPBOARD_BYTES`
plus framing overhead), so text and images both sync. Validated on Windows and
macOS; Linux clipboard image sync is still unverified (see "Not yet done").
Earlier: clipboard sharing can be toggled at runtime with
`omni clipboard on|off` — the daemon flips the opt-in guard, wakes or parks the
polling task, and persists the choice to the config so it survives a restart;
`omni status` shows the current state. The poll task now parks for free while
sharing is off, so the opt-in default costs nothing. Earlier the same day: a
TOFU stale-pin fix from a Windows↔macOS run:
accepting a host whose certificate rotated now **replaces** its pin instead of
appending a second one, and the trust store collapses any duplicate host pins
on load. Before this, a stale pin could shadow the current one, so dialing a
peer that had reconnected the other way refused its certificate
(`ApplicationVerificationFailure`); the connect error now points at
`omni peers remove`. Earlier the same day: a high-DPI fix making the Windows
process per-monitor DPI aware so screen size, captured deltas, and the parked
cursor all use real pixels — without it a scaled 2K/4K laptop dragged the
remote cursor into a corner and left the keyboard stuck on the remote; plus
cursor-visibility fixes — the macOS sink warps the cursor so a remote-driven
move stays drawn, and the Windows source parks the local cursor instead of
hiding it with the OS-global, crash-persistent `SetSystemCursor`.)_

## Where we are

The project is **feature-complete for a first end-to-end build, on all three
target platforms**. Every crate is implemented: the shared-kernel **Protocol**,
**Topology** (virtual desktop and edge crossings), **Security** (allowlist +
TOFU trust policy), **Session** (lifecycle, roles, input routing), **Input**
(real macOS, Linux, *and Windows* adapters), **Transport** (real QUIC adapter),
the **Runtime** daemon that wires them all together, and the **CLI** (`omni`)
that drives it over local IPC. The full pipeline — capture → route → QUIC
datagram → inject, with TOFU handshake and accept/reject flow — exists in code
and builds on macOS, Linux, and Windows; what it has *not* had yet is a live
two-machine run (see "Not yet done").

The secure channel is **QUIC** (TLS 1.3 over UDP), via `quinn` + `rustls` —
adopted in place of the originally planned DTLS 1.3 (see "Open decisions").

Local IPC is a **Unix-domain socket** on macOS/Linux and a **named pipe** on
Windows, behind one transport abstraction; sessions now exchange
**heartbeats** and drop a silently-dead peer; the screen arrangement is
**configurable** (`omni layout`) rather than a fixed left/right chain; and a
**daemon-level integration test** drives two daemons through the real QUIC +
IPC path. A **GitHub Actions** workflow runs the quality gate on all three
platforms.

The whole workspace builds clean under `cargo fmt`, `cargo clippy -D warnings`,
and `cargo test` (98 tests), including on Windows.

## Crate status

| Crate            | Status        | What's there                                                                 |
| ---------------- | ------------- | ---------------------------------------------------------------------------- |
| `omni-protocol`  | **Implemented** | Ids, input events, control messages (incl. screen sizes and `CursorWarp`), clipboard payloads (`ClipboardData`/`ClipboardImage`, size-capped + overflow-checked), and the postcard wire codec. 21 tests. |
| `omni-topology`  | **Implemented** | Virtual desktop layout, edge crossings, and the `LayoutStore` port. 13 tests. |
| `omni-security`  | **Implemented** | Allowlist + TOFU trust policy, `TrustStore`/`CertProvider` ports, self-signed identity generation. 15 tests. |
| `omni-session`   | **Implemented** | Session lifecycle, dynamic roles, active-target routing, `SessionEvents` port. 12 tests. |
| `omni-input`     | **Implemented** | Ports, in-memory adapters, permission diagnostics, and the real OS adapters: macOS (CGEvent tap + post; the sink warps the cursor so a remote-driven move stays visible and stamps the click-count so double/triple clicks register), Linux (evdev + uinput), and Windows (low-level hooks + SendInput; the local cursor is parked, not hidden). Cursor **hiding** on suppression where it is safe and self-restoring (macOS `CGDisplayHideCursor`, Linux X11 empty-cursor on the root window) and true Linux cursor-position query (`XQueryPointer`). 17 tests. |
| `omni-clipboard` | **Implemented** | Opt-in clipboard sharing (text + images) over a ports-and-adapters design: `arboard` adapter, in-memory mock, echo-loop guard, strict opt-in toggle queryable at runtime. 8 tests. |
| `omni-transport` | **Implemented** | `SecureChannel` port, framing, loopback channel, and the real QUIC adapter (quinn + rustls, mTLS, TOFU verifiers, datagrams + control stream). The control-frame limit admits a full clipboard payload so images sync over the reliable stream. The input path is tuned for latency: a shallow datagram send buffer (drops oldest stale positions) and the BBR congestion controller. 13 tests. |
| `omni-runtime`   | **Implemented** | The daemon: config/paths, persistent identity, trust store, rate limiter, cross-platform IPC (Unix socket / Windows named pipe), heartbeats, configurable layout, opt-in clipboard sync over the control stream (toggleable at runtime and persisted), doctor checks, and the composition root that runs the pipelines. The peer task coalesces a backlog of queued cursor positions to the latest one (keeping clicks/keys/scrolls intact) so a congested link does not make the cursor lag. 25 tests + a two-daemon integration test. |
| `omni-cli`       | **Implemented** | The full `omni` binary: start/stop/status, doctor, connect/disconnect, accept/reject, peers (+ remove), layout, clipboard on/off, uninstall, over the daemon's Unix socket. |

### What `omni-protocol` provides

- **Identifiers** (`ids`): `MachineId`, `PeerId`, `SessionId`, and `Fingerprint`
  (a 32-byte SHA-256 digest that renders as lowercase hex for TOFU pinning).
- **Input events** (`input`): a platform-neutral `InputEvent` with `Key`,
  `Motion`, `Button`, and `Scroll` variants; `KeyCode` (USB HID usage codes),
  packed `Modifiers`, `MouseButton`, `MouseDelta`, `ScrollDelta`.
- **Control messages** (`control`): `ControlMessage` (`ConnectRequest`, `Accept`,
  `Reject`, `Disconnect`, `Heartbeat`, `CursorWarp`) and `RejectReason`.
- **Wire codec** (`wire`): the `Message` envelope (`Input`, `Control`,
  `Clipboard`) plus `encode`/`decode` over
  [postcard](https://docs.rs/postcard) — a compact varint binary format chosen
  for small datagrams and low-latency (de)serialization. Truncated or empty
  input is rejected.
- **Handshake payloads**: `ConnectRequest` carries the initiator's screen size
  and `Accept` carries the target's machine id and screen size, so each side can
  place the other in its virtual desktop layout.
- **Clipboard payloads** (`clipboard`): `ClipboardData` (text or `ClipboardImage`)
  with overflow-checked dimension validation and a 64 MiB payload cap, so a
  malformed or oversized payload is rejected before it is sent or applied.

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
  `CGEventPost`, stamping its events so the tap never re-captures them. Each
  move also warps the cursor (`CGWarpMouseCursorPosition`) to the same spot,
  because macOS otherwise blanks the cursor for purely synthesized motion;
  that keeps the remote-driven cursor visible. Needs the Accessibility
  permission — never root. A `kVK ↔ HID` keymap covers the full ANSI layout.
- **Linux adapters** (`linux`): `LinuxSource` reads keyboards and mice from
  `/dev/input` (one thread per device; suppression = `EVIOCGRAB`), `LinuxSink`
  injects through a uinput virtual device that the capture side knows to skip.
  Needs only `input`-group membership — never root. A `KEY_* ↔ HID` keymap
  mirrors the macOS one. (Compile-checked against a Linux target; needs live
  hardware to exercise.)
- **Windows adapters** (`windows`): `WindowsSource` captures through
  `WH_KEYBOARD_LL` / `WH_MOUSE_LL` low-level hooks on a dedicated message-loop
  thread (suppression swallows the event before the OS acts; mouse motion is
  turned into relative deltas, with the cursor parked at screen centre while
  controlling a remote so deltas never stall at an edge), and `WindowsSink`
  injects with `SendInput`, stamping events so the hooks never re-capture them.
  Needs no elevation for ordinary windows — only to drive administrator windows
  (User Interface Privilege Isolation). A `VK_* ↔ HID` keymap mirrors the other
  platforms. The process declares **per-monitor DPI awareness (v2)** at startup
  (`platform::prepare_process`), so on a scaled high-DPI display the hook
  deltas, `SetCursorPos`/`GetCursorPos` parking, and `GetSystemMetrics` screen
  size all share one physical-pixel space — without it the mismatch biased
  every delta and pinned a remote-controlled cursor to a corner. (Built and
  unit-tested on Windows; needs live hardware to exercise the full pipeline.)

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
  on macOS, `%APPDATA%\omnipresent` on Windows): `config.json` (UDP port,
  default 4733; optional screen-size override; per-host edge placements),
  certificate + key, `trust.json`, the IPC socket (or named pipe on Windows),
  and the log.
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
- **Identity, keys & secrets** (`identity`, `secure`): the self-signed
  certificate is generated once and reused; the private key is written
  owner-only — mode `0600` on Unix, an inheritance-stripped owner-only ACL on
  Windows.
- **IPC** (`ipc`, `ipc_transport`): the JSON-lines request/response protocol
  the CLI speaks (status, connect, accept, peers, layout, ...), over one
  transport abstraction — a Unix-domain socket on macOS/Linux, a per-state-dir
  named pipe (local clients only, first-instance claimed) on Windows. Status
  reports whether input capture is live, so a target-only daemon is visible.
- **Heartbeats**: each session sends a `Heartbeat` every 2 s and tears itself
  down if nothing arrives from the peer within 8 s, so a silently-dropped peer
  is noticed without waiting for QUIC's own idle timeout.
- **Layout** (`omni layout`): per-host edge placements, applied live to an open
  session and persisted to `config.json` for next time, so machines can be
  arranged on any edge instead of a fixed left/right chain.
- **Doctor** (`doctor`): environment checks behind `omni doctor` — the
  platform's input-permission diagnostics (Accessibility on macOS; evdev and
  uinput access on Linux) plus screen-size and state-directory checks.
- **The daemon** (`daemon`): the composition root. A capture thread polls the
  OS input source and advances the virtual cursor through Topology; an edge
  crossing flips Session's active target, suppresses local input, and warps
  the peer's cursor to the entry point. While a remote peer is active, pointer
  motion travels as the cursor's **absolute position on the peer's screen**
  (mapped through the virtual desktop using both machines' sizes), not raw
  relative deltas — so the two cursors cannot drift apart and control stays
  correct across different resolutions. Input events ride unreliable QUIC
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
<host|fingerprint>`, `peers` / `peers remove <host>`, `layout` (list or set
where each peer sits), `update` (self-update to the latest GitHub release —
stops the daemon, swaps the binary, restarts it), `doctor` (prints every permission/environment
check and the daemon's own capture state, non-zero exit when something is
unmet), and `uninstall` (stops the daemon, removes the config dir, deletes
the binary). Each command is one JSON line to the daemon and one line back;
errors land on stderr with a non-zero exit.

## Tooling & dependencies

- Rust workspace, edition 2024, resolver 3.
- Third-party deps pinned once in `[workspace.dependencies]`: `serde`, `postcard`,
  the network/crypto stack (`quinn`, `rustls` + `ring`, `rcgen`, `sha2`, `tokio`,
  `bytes`), the daemon/CLI layer (`clap`, `serde_json`, `dirs`, `rand`,
  `tracing` + `tracing-subscriber`), and the per-OS input/IPC backends
  (`core-graphics`/`core-foundation` on macOS, `evdev` on Linux, `windows-sys`
  on Windows).
- Quality gate per change: `cargo fmt --all`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test` — run locally and in CI
  (GitHub Actions) across Linux, macOS, and Windows.
- Building on Windows needs a linker: native MSVC (the standard runner toolchain
  CI uses), or the self-contained `x86_64-pc-windows-gnu` toolchain plus a
  MinGW-w64 C toolchain for the `ring` crypto backend on a box without MSVC.

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

- **Automatic reconnection.** `omni connect` between two real machines
  (Windows ↔ macOS) is validated and works, including clipboard. What is missing
  is recovery from a *dropped* link: when the connection fails (network blip,
  idle timeout, peer restart) the session is torn down and the user must
  reconnect by hand. The intended behaviour is for the dialing side (Controller)
  to retry with exponential backoff until the peer returns or the user runs
  `omni disconnect`. This is the active next piece of work.
- **Linux live run.** macOS ↔ Linux ↔ Windows triangles have not been run; the
  Linux adapters build and unit-test but are not exercised on real hardware
  (see also "Linux clipboard sharing").
- **`omni start` is plain detach.** No launchd/systemd/Windows-service files
  yet, so the daemon does not survive a reboot or restart on crash.
- **Linux and Windows are not hardware-tested.** The evdev/uinput adapters and
  the Win32 hook/`SendInput` adapters build and unit-test, but have not run
  against real devices on a live desktop.
- **Linux clipboard sharing.** Text and image sync are validated between two
  real machines on Windows and macOS. Linux is not supported yet: the `arboard`
  adapter is built with `default-features = false`, so its X11/Wayland clipboard
  backends are not enabled or exercised — wiring and verifying Linux clipboard
  (text and image) is the remaining clipboard work.

## Open decisions

- **Secure channel: decided — QUIC** (TLS 1.3 over UDP) via `quinn` + `rustls`,
  replacing the originally planned DTLS 1.3. Rationale: no production-ready
  *pure-Rust* DTLS 1.3 + mTLS exists (rustls has no DTLS; `rusty-dtls` is PSK-only;
  the webrtc `dtls` crate is 1.2 only; wolfSSL/OpenSSL mean a C dependency). QUIC
  keeps every required property (UDP-only, mutual cert auth, TOFU, anti-replay,
  modern crypto), carries input over unreliable datagrams (RFC 9221), and has the
  most mature pure-Rust implementation.
- **Local IPC: decided — Unix domain socket** in the config directory (mode
  0600) on macOS/Linux, and a **named pipe** (local clients only, first
  instance claimed) on Windows, behind one transport abstraction. JSON lines,
  one request, one response. Simple, debuggable, and access-controlled by the
  platform's own mechanism.
- Wire-format versioning: whether to prepend a protocol version byte in Transport
  framing (deliberately left out of the Protocol codec for now).
