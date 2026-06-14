# Omnipresent

One keyboard and mouse across multiple machines. Move your cursor to the edge of
one screen and it flows onto the next — your keyboard follows. macOS, Linux, and
Windows.

Every connection is encrypted and mutually authenticated (QUIC / TLS 1.3 over
UDP), with Trust-On-First-Use certificate pinning. No audio, and clipboard
sharing stays off unless you opt in.

## Install

Prebuilt binaries — no Rust toolchain or compiler needed.

**macOS / Linux**

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/JonatanFD/omnipresent/releases/latest/download/install.sh | sh
```

**Windows** (PowerShell)

```powershell
powershell -c "irm https://github.com/JonatanFD/omnipresent/releases/latest/download/install.ps1 | iex"
```

Both install a per-user `omni` and put it on your `PATH`. To pick the directory,
set `OMNI_INSTALL_DIR` first. Prefer a package you can verify? Grab the archive
and its `.sha256` from the [latest release](https://github.com/JonatanFD/omnipresent/releases/latest)
and unpack it yourself.

### From source

Needs a Rust toolchain and a C compiler (for the `ring` crypto backend — MSVC or
MinGW-w64 on Windows):

```sh
cargo install --git https://github.com/JonatanFD/omnipresent omni-cli
```

## Usage

Run the daemon on every machine, then connect from the one whose keyboard and
mouse you want to share:

```sh
omni start                 # start the background daemon
omni doctor                # check OS permissions and environment

omni connect <host>        # request control of a remote machine
omni accept <host>         # on the target: approve an incoming request
omni layout <host> right   # place a peer past an edge (left/right/top/bottom)

omni status                # daemon state and active sessions
omni peers                 # known peers and their status
omni disconnect <host>     # end a session
omni stop                  # stop the daemon

omni update                # update to the latest release
```

On first accept the peer's certificate fingerprint is pinned; later connections
from that peer are auto-accepted unless the fingerprint changes.

### OS permissions

- **macOS** — grant Accessibility to the terminal you run `omni start` from
  (System Settings → Privacy & Security → Accessibility).
- **Linux** — add your user to the `input` group and make sure `/dev/uinput` is
  writable. `omni doctor` prints the exact commands.
- **Windows** — works without elevation; run elevated only to control
  administrator windows.

## Documentation

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — module boundaries, ports, and
  data flow.
- [`docs/STATUS.md`](docs/STATUS.md) — what's implemented and what's next.

## License

MIT.
