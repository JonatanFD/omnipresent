# Omnipresent — Native GUI Integrations

How native graphical clients fit into Omnipresent. This builds on the design in
[`ARCHITECTURE.md`](ARCHITECTURE.md), the product scope in
[`../CLAUDE.md`](../CLAUDE.md), and the engineering rules in
[`../.claude/rules/constrains.md`](../.claude/rules/constrains.md). It defines
the **constraints** every native client must obey, the **features** they expose,
and the **phased plan** to build them.

Target clients: a **macOS** app (Swift / SwiftUI) and a **Windows** app
(C# / WinUI 3). Linux stays CLI-only for now (see "Out of scope").

## Foundation: a GUI is just another IPC client

The daemon already exposes everything a client needs over **local IPC** — a
JSON-lines request/response protocol (`omni-runtime/src/ipc.rs`) over the
platform's native primitive: a **Unix-domain socket** (mode `0600`) on macOS and
a **named pipe** (local clients only) on Windows
(`omni-runtime/src/ipc_transport.rs`). The `omni` CLI is nothing more than a thin
client of this surface.

```
  macOS:   SwiftUI app  ┐
  Windows: WinUI 3 app  ┼── JSON / local IPC ──> omni daemon (Rust)
  CLI:     omni binary  ┘                          (Input/Topology/Session/
                                                    Security/Transport)
```

A native GUI is therefore **another client of the same IPC**. It reuses all the
core logic and touches none of it. The protocol is language-agnostic (JSON over a
socket/pipe), so a client can be written in Swift, C#, or Rust without changing
anything below the IPC.

## Constraints

These are binding rules for any native client.

### Architecture

1. **A GUI is only an IPC client.** It never imports, links, or re-implements the
   core crates (Input, Topology, Session, Security, Transport, Runtime).
2. **The JSON-lines IPC protocol is the only contract.** Clients render state and
   send commands; they never duplicate business logic (trust decisions, layout
   math, edge crossings, fingerprinting).
3. **New capabilities are added by extending the IPC protocol**, never by
   side-channels. New `Request` / `Response` / `Event` variants and new fields are
   **backward-compatible**: new fields are optional (`#[serde(default)]`) and new
   variants are additive. The protocol carries a **version**, sent at the start of
   a session, so a client can detect an incompatible daemon and tell the user to
   update rather than misbehave.
4. **The daemon owns all state.** Clients never read or write `trust.json`,
   `config.json`, certificates, or the log directly — the daemon is the single
   writer, with the correct permissions. Everything goes through the IPC.

### Language (the one exception to "Rust only")

5. The rule *"the language of the project is Rust"* still holds for **everything
   below the IPC**. The **GUI layer is the only exception**: Swift/SwiftUI on
   macOS, C#/WinUI 3 on Windows.
6. Native frontends live **outside the Cargo workspace** (e.g.
   `clients/omni-mac/`, `clients/omni-windows/`), each with its own toolchain and
   its own CI job (Xcode on a macOS runner, `dotnet` on a Windows runner). They
   must not appear in `Cargo.toml`'s `members` or affect `cargo build`.

### Security (clients inherit the daemon's posture and may not weaken it)

7. **Discovery and pairing are addressing, not authorization.** Finding a peer by
   mDNS, or pasting a pairing code, only makes a machine *reachable*. It never
   bypasses the explicit **accept/reject** step or **TOFU**. The first connection
   from any peer still waits for a human decision.
8. **No privilege elevation.** A client inherits the IPC's local-only access
   control (owner-only socket / pipe that rejects remote clients). It never runs
   as root/admin.
9. **Key material is never shown or logged.** Certificate keys and session secrets
   stay inside the daemon. The certificate **fingerprint** may be shown — it is
   exactly what the user verifies.
10. **The accept prompt must show the peer's name and fingerprint clearly.** This
    is the human verification point for TOFU on a first connection; it cannot be
    auto-dismissed or hidden behind an open window (hence a tray/menu-bar entry,
    below).
11. **Clipboard stays opt-in.** A client only flips it via the IPC
    (`Clipboard { enabled }`); it never reads the OS clipboard itself.

### Product

12. **The GUI does not replace the CLI.** Both are equal clients of the same
    daemon and must coexist.
13. The GUI **surfaces `doctor`** (permissions/environment) so a user can fix, for
    example, a missing macOS Accessibility grant.

## IPC evolution: a live event channel

The current IPC is request → response with no push. A responsive, *lightweight*
GUI needs live state and instant notification of incoming requests. Polling (the
GUI asking `Status` every second forever) wastes work while idle and still lags;
an **event channel** does nothing until something changes and notifies instantly,
so it is both lighter at runtime and better UX.

Plan: add a `Request::Subscribe` that keeps the connection open, after which the
daemon **pushes `Event` lines** as state changes — session opened/closed, active
target changed, an incoming request arrived or was resolved, a discovered peer
appeared/disappeared, clipboard toggled. This is a small, well-scoped Rust change
in the daemon, written test-first, and it benefits the CLI too (e.g. a future
`omni watch`).

## Connection experience (no IPs in the user's face)

The user should rarely, if ever, see an IP. Three mechanisms, all resolving to an
address **under the hood** (shown only in an "advanced/details" view):

1. **Local-network discovery (mDNS / Bonjour).** The daemon advertises itself and
   browses for other omni daemons on the LAN, so the GUI lists peers by **friendly
   name** ("Jonatan's MacBook") and the user clicks to connect — no IPs, no codes.
   This is the product's primary case (machines side by side). A pure-Rust mDNS
   crate (e.g. `mdns-sd`) keeps the dependency tree free of C libraries. Clicking a
   discovered peer still triggers the normal accept/TOFU flow (constraint 7).
2. **Pairing code (reversible, fingerprint-bound).** For when discovery does not
   apply or the address is shared out of band (chat, voice). A machine shows its
   code; the other pastes it and connects. The code **encodes the address plus
   enough of the machine's certificate fingerprint to verify it**, so the dialer
   can reject a man-in-the-middle presenting a different certificate — turning the
   copy/paste into an out-of-band authenticated channel. It is *not* a one-way
   hash (a hash cannot be dialed); it is a compact **encoding**. Format: short
   **alphanumeric** in groups, Crockford base32 (no ambiguous `0/O/1/I`), e.g.
   `OMNI-7F3K-9Q2M-…`. The fingerprint verifies the cert but **does not** replace
   the accept step (constraint 7).
3. **Friendly names + saved peers.** Every machine has an editable name. After the
   first pairing the peer is remembered (the trust store already persists it), so
   the user reconnects by name and never sees an address again.

These connection features are mostly **daemon + protocol** work (Rust), and they
also enrich the CLI (`omni discover`, `omni code`, `omni connect-code <code>`).

> **Scope:** discovery and codes target the **LAN** case. Connecting **across the
> internet** (NAT traversal, relays) is a separate, larger effort and is out of
> scope for now.

## Feature inventory

Where each feature lives: **[D]** daemon, **[P]** IPC protocol, **[G]** GUI.

### Connection
- mDNS discovery: advertise + list LAN peers. **[D][P][G]**
- Pairing code: generate mine / connect by pasting one; fingerprint-bound and
  verified. **[D][P][G]**
- Editable friendly name per machine. **[D][P][G]**
- Reconnect by name from saved peers. **[P][G]**

### Session / control
- Live state (sessions, role, which peer has input) via the event channel.
  **[D][P][G]**
- Accept/reject popup showing **name + fingerprint**. **[P][G]**
- Connect / disconnect / forget a peer. **[P][G]**

### Layout
- Visual edge editor (a friendlier `omni layout`). **[G]** — already covered by the
  current protocol.

### System
- Clipboard opt-in toggle. **[P][G]** — protocol already exists.
- `doctor` (permissions/environment) in the UI. **[P][G]** — already exists.
- Version indicator + "incompatible daemon, please update" notice. **[P][G]**

## Layout editor: scope

Topology currently models the desktop as **edge links** — each peer sits *past*
the left/right/top/bottom edge of another — not as free 2-D pixel positions. The
visual editor therefore maps a drag to **one of the four edges** (it is a nicer
`omni layout <host> <edge>`), which covers the common case ("put the Mac to the
left of Windows"). A free 2-D arrangement with arbitrary offsets would require
changes in Topology and the protocol and is deferred (see "Out of scope").

## App model

A **window plus a tray / menu-bar entry**:
- The **window** hosts the peer list, the visual layout editor, settings, and the
  pairing-code / discovery views.
- The **tray / menu-bar** entry gives quick status and, crucially, delivers the
  **accept/reject prompt even when the window is closed** — a security decision
  point cannot depend on the window being open (constraint 10).

## Phased plan

1. **Event channel + protocol version (daemon, Rust).** `Request::Subscribe`, an
   `Event` enum, and a version handshake. Test-first. Unblocks both GUIs and
   improves the CLI.
2. **Connection backend (daemon, Rust).** mDNS advertise/browse, machine name in
   config, pairing-code generate/parse with fingerprint verification, and the IPC
   surface for all three. Mirrored in the CLI.
3. **macOS app (Swift / SwiftUI).** Unix-socket client; menu-bar + window;
   discovery and pairing-code views; visual edge editor; accept/reject; doctor.
   Packaging (signing, notarization) as a sub-task.
4. **Windows app (C# / WinUI 3).** Named-pipe client; tray + window; same views.
   Packaging (signing) as a sub-task.

## Out of scope (for now)

- **Free 2-D layout** with arbitrary offsets (needs Topology + protocol changes).
- **Internet connectivity** (NAT traversal / relays); discovery and codes are
  LAN-oriented.
- **Linux GUI.** Linux stays CLI-only; a native or cross-platform Linux client is
  deferred.
