# Omnipresent

Omnipresent is a software that lets you control multiple PCs from a single keyboard and mouse, with seamless cursor movement across machines.

# Description

Omnipresent lets you use one keyboard and mouse to control multiple computers at the same time — for example, a Mac Mini and a Linux laptop side by side. When your mouse reaches the edge of one screen, it flows into the next machine and your keyboard input follows. No audio support is planned for now.

The goal is to do this better than existing solutions (Synergy, Barrier, Input Leap, etc.) — faster, more reliable, and simpler to set up.

# Architecture

The core is split into bounded-context modules (Input, Topology, Session,
Security, Transport), a shared-kernel **Protocol** crate, and a **Runtime**
daemon that wires them together and serves the CLI. See
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full design, ports,
data flow, and crate layout. See [`docs/STATUS.md`](docs/STATUS.md) for the
current implementation status and what's next to build.

# CLI Interface

All interaction happens through the `omni` binary.

## Daemon

```
omni start          # start the background daemon
omni stop           # stop the running daemon
omni status         # show whether the daemon is running and list active connections
omni doctor         # check that the OS permissions and environment the daemon needs are in place
```

## Connections

```
omni connect <host>        # request control of a remote machine (initiator side)
omni disconnect <host>     # end an active session with a remote machine
```

## Incoming requests (target side)

When another machine requests control, the daemon notifies the user. They respond with:

```
omni accept <host|fingerprint>   # approve an incoming connection request
omni reject <host|fingerprint>   # deny an incoming connection request
```

On first accept, the peer's certificate fingerprint is pinned (TOFU). Subsequent connections from that peer are auto-accepted unless the fingerprint changes.

## Peers

```
omni peers                  # list all known peers and their status (connected / trusted / blocked)
omni peers remove <host>    # remove a peer from the trusted list
```

## Layout

By default a peer you dial sits past your right edge, and a peer that dials you
sits past your left edge. Override that per host:

```
omni layout                      # list where each peer sits in the virtual desktop
omni layout <host> <edge>        # place a peer past left | right | top | bottom
```

The placement is saved, so it also applies the next time that peer connects.

## Clipboard

Clipboard sharing is opt-in and off by default. Toggle it at runtime (no
restart needed); the choice is saved and applies on the next start too.

```
omni clipboard on    # share clipboard copies with connected peers
omni clipboard off   # stop sharing the clipboard
```

While off, the daemon never reads the local clipboard nor applies a remote one.
`omni status` shows the current state.

## Update

```
omni update         # download and install the latest release, then restart the daemon
```

## Uninstall

```
omni uninstall   # stop the daemon, remove all config, certs, and peer data, then delete the binary
```

---

# Scope (current)

- Keyboard and mouse sharing across machines
- Seamless cursor transition between screens
- Cross-platform: macOS, Linux, and Windows
- No audio redirection

# Technical constraints

## Language & tooling

- Rust — see `.claude/rules/constrains.md` for full coding rules.

## Network protocol

- **Transport:** UDP only — low latency is critical for real-time input events.
- **Security layer:** QUIC (TLS 1.3 over UDP). Every connection is encrypted and
  mutually authenticated by QUIC's built-in TLS 1.3; there is no plaintext path.
- **Input delivery:** unreliable QUIC datagrams (RFC 9221) carry input events, so
  a lost packet is dropped rather than retransmitted — stale input is never worth
  resending. Control/session signalling can use a reliable QUIC stream.

  > QUIC replaces the originally planned DTLS 1.3. It keeps every required
  > property (UDP-only, mutual cert auth, TOFU, anti-replay, modern crypto) while
  > having the most mature pure-Rust implementation (`quinn` + `rustls`), with no
  > C-library dependency. See `docs/STATUS.md` for the rationale.

## Security practices

- **Mutual authentication (mTLS):** both the controller and each target machine present certificates. No anonymous connections.
- **Trust On First Use (TOFU):** on first connection, pin the peer's certificate fingerprint locally. Reject any cert change on subsequent connections.
- **Anti-replay protection:** rely on QUIC's built-in AEAD packet protection and packet-number replay detection. Drop duplicate or out-of-window packets.
- **Allowlist only:** each machine maintains an explicit list of allowed peer addresses/fingerprints. Reject everything else at the network boundary before any processing.
- **Input rate limiting:** cap input events per second per session to prevent event flooding or amplification abuse.
- **Least privilege:** the daemon runs with the minimum OS permissions needed — no root unless strictly required by the input subsystem, drop privileges after binding.
- **No sensitive features by default:** clipboard sharing is disabled unless explicitly opted in, to limit accidental data exposure.
- **Key material never logged:** TLS keys, session secrets, and certificate private keys must never appear in logs or debug output.
