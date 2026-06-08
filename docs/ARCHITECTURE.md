# Omnipresent — Architecture

This document defines the core architecture of Omnipresent. It builds on the
product scope and constraints in [`CLAUDE.md`](../CLAUDE.md) and the engineering
rules in [`.claude/rules/constrains.md`](../.claude/rules/constrains.md)
(Rust, DDD, SOLID, TDD).

## Guiding principles

- **DDD bounded contexts.** Each core module is a bounded context with its own
  domain. Modules talk through explicit contracts, never through shared mutable
  state.
- **Ports & adapters (hexagonal).** Every module exposes its domain logic behind
  traits (*ports*). Platform- and IO-specific code lives in *adapters* that
  implement those ports. This keeps the domain pure and testable, and satisfies
  the Dependency Inversion principle.
- **Dependencies point inward.** Domain depends on nothing external. Application
  logic depends on the domain. Infrastructure (OS APIs, sockets, cert stores)
  depends on application and domain — never the reverse.
- **One responsibility per module.** Transport moves bytes; Security decides
  trust; Input touches the OS; etc. No module does two jobs.

## Module overview

| Module        | Was            | Responsibility                                              |
| ------------- | -------------- | ---------------------------------------------------------- |
| **Input**     | InputManager   | Capture local input events; inject remote events into the OS |
| **Topology**  | PCManager      | Track each machine's screen geometry and position; resolve edge crossings |
| **Session**   | StateManager   | Manage active sessions and dynamic Controller/Target roles |
| **Security**  | Security       | Identity, trust policy (mTLS, TOFU, allowlist, anti-replay) |
| **Transport** | *(new)*        | UDP socket + DTLS channel; framing and (de)serialization   |
| **Protocol**  | *(new, shared)*| Wire message and event type definitions (shared kernel)    |
| **Runtime**   | *(new)*        | Composition root: the daemon that wires modules and serves the CLI |

The first five are bounded contexts. **Protocol** is a *shared kernel* — a small
crate of common value objects every context speaks. **Runtime** is the
composition root that owns the process lifecycle.

## Roles (dynamic)

A session always has two roles, assigned per session and reversible:

- **Controller** — the machine whose physical keyboard and mouse are the source
  of input. (Your "client" — the Mac in your example.)
- **Target** — the machine that receives and injects the events. (Your "server"
  — the laptop.)

Any machine can be Controller in one moment and Target the next. **Session** owns
this assignment; no role is hard-coded at build or config time.

---

## 1. Input

**Responsibility:** the only module allowed to touch the OS input subsystem.

- *Capture* — read keyboard (keydown/keyup, modifiers) and mouse (motion,
  buttons, scroll) events from the local OS.
- *Inject* — synthesize those same events into the local OS when this machine is
  the Target.

**Domain types:** `InputEvent` (key, button, motion, scroll), `KeyCode`,
`Modifier`, `MouseDelta`.

**Ports**

- `InputSource` — `fn poll(&mut self) -> Option<InputEvent>` (capture).
- `InputSink` — `fn inject(&mut self, event: InputEvent)` (injection).

**Adapters:** `macos` (CGEvent / IOKit), `linux` (evdev / uinput). Adapters are
swapped per platform; the domain never sees a platform type.

**Depends on:** Protocol (for `InputEvent`).

## 2. Topology

**Responsibility:** model the virtual desktop formed by all connected machines
and decide *when* and *where* the cursor crosses between them.

- Holds each machine's screen resolution and relative position
  (top/left/right/bottom neighbors).
- Tracks the cursor's virtual position across the combined layout.
- When the cursor reaches an edge linked to a peer, reports the crossing and the
  entry coordinates on the peer's screen — the basis of seamless movement.

**Domain types:** `Machine`, `Screen { width, height }`, `EdgeLink`,
`VirtualLayout`, `Crossing { peer, entry_point }`.

**Ports**

- `LayoutStore` — load/persist the configured arrangement of machines.

**Depends on:** Protocol.

## 3. Session

**Responsibility:** own the lifecycle and role assignment of active sessions.

- Establishes a session when a connection is accepted; tears it down on
  disconnect or loss.
- Assigns and flips **Controller/Target** roles dynamically (driven by Topology
  edge crossings and by explicit connect/disconnect).
- Tracks which Target is currently *active* (receiving input) at any moment.

**Domain types:** `Session`, `Role { Controller, Target }`, `SessionId`,
`ActiveTarget`.

**Ports**

- `SessionEvents` — emit lifecycle/role-change events for the Runtime to react
  to (e.g. notify the CLI).

**Depends on:** Protocol, Topology (to react to crossings).

## 4. Security

**Responsibility:** the trust authority. Owns *policy*, not the socket.
Implements the rules in `CLAUDE.md`:

- mTLS identity — load/verify this machine's cert and the peer's.
- TOFU — pin a peer's fingerprint on first accept; reject changes thereafter.
- Allowlist — only listed peers may establish a channel.
- Anti-replay policy — configure and enforce the DTLS replay window.

**Domain types:** `PeerIdentity`, `Fingerprint`, `TrustDecision`,
`AllowList`.

**Ports**

- `TrustStore` — persist pinned fingerprints and the allowlist.
- `CertProvider` — supply this machine's certificate/key material.

**Depends on:** Protocol. Security *informs* Transport which peers and certs are
acceptable; Transport calls into Security to authorize a handshake.

## 5. Transport

**Responsibility:** move encoded messages between machines. The *pipe*.

- Owns the UDP socket and the DTLS 1.3 channel over it.
- Frames and (de)serializes Protocol messages to/from datagrams.
- Enforces the anti-replay window (configured by Security) and drops anything
  failing the DTLS layer.

**Domain types:** `Datagram`, `Channel`, `Endpoint`.

**Ports**

- `Socket` — abstract UDP send/recv (real socket in prod, in-memory in tests).
- `SecureChannel` — DTLS wrap/unwrap, authorized via Security.

**Adapters:** UDP socket adapter; DTLS adapter over the chosen Rust crate
(to be selected per the "latest libraries" rule — e.g. `tokio` + a DTLS-capable
TLS stack).

**Depends on:** Protocol, Security (for handshake authorization).

## 6. Protocol (shared kernel)

**Responsibility:** the common vocabulary. A small, dependency-light crate
holding the value objects and wire messages every other module exchanges:

- `InputEvent` and its parts.
- Control messages: `ConnectRequest`, `Accept`, `Reject`, `Disconnect`,
  `Heartbeat`.
- Shared identifiers: `PeerId`, `MachineId`, `Fingerprint`, `SessionId`.
- (De)serialization format definition (e.g. a compact binary encoding).

**Depends on:** nothing internal. This is the leaf of the dependency graph.

## 7. Runtime (composition root)

**Responsibility:** the long-running daemon. Wires concrete adapters into the
ports above and drives the event loop. The only place that knows about every
module.

- Starts/stops the daemon (`omni start` / `omni stop`).
- Runs the capture → route → send and receive → inject pipelines.
- Exposes a **local IPC** surface (e.g. a Unix domain socket) so the `omni` CLI
  can issue `connect`, `accept`, `reject`, `status`, etc., and receive
  notifications of incoming requests.
- Applies least-privilege startup (drop privileges after binding), per
  `CLAUDE.md`.

**Depends on:** all modules. Nothing depends on Runtime.

---

## Data flow

**Sending (this machine is Controller):**

```
OS input
  → Input (capture)            InputEvent
  → Topology                   is the cursor crossing an edge? to which peer?
  → Session                    which Target is active? (may flip on crossing)
  → Protocol                   encode InputEvent
  → Transport (DTLS send)      datagram over UDP
  → network
```

**Receiving (this machine is Target):**

```
network
  → Transport (DTLS recv)      verify channel, drop replays/invalid
  → Security                   peer authorized? (channel established under policy)
  → Protocol                   decode InputEvent
  → Session                    validate this belongs to an active session
  → Input (inject)             synthesize into local OS
```

Edge crossing is what makes movement seamless: **Topology** detects the cursor
hitting a linked edge, **Session** flips the active Target, and Topology supplies
the entry coordinates so the cursor appears at the right spot on the next screen.

## Dependency graph

```
Protocol  ← (everything depends on it)
   ↑
Topology   Security
   ↑          ↑
Session    Transport
   ↑          ↑
   └──────────┴──────  Runtime  ←──  CLI (omni, via local IPC)
                                       Input (adapters wired by Runtime)
```

Arrows mean "depends on". The graph is acyclic; Runtime is the only place
allowed to depend on everything.

## Suggested crate layout

A Cargo workspace with one crate per module enforces these boundaries at compile
time (you cannot create a cycle the borrow checker won't reject), which directly
serves the DDD goal:

```
omni/
├── crates/
│   ├── omni-protocol/    # shared kernel
│   ├── omni-input/
│   ├── omni-topology/
│   ├── omni-session/
│   ├── omni-security/
│   ├── omni-transport/
│   ├── omni-runtime/     # daemon / composition root
│   └── omni-cli/         # the `omni` binary
└── Cargo.toml            # workspace
```

Each crate keeps its domain (`domain/`), application (`application/`), and
infrastructure (`infra/`) layers internally, per DDD. Tests are written first
(TDD) against the ports, using in-memory adapters.
