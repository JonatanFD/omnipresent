#!/bin/sh
# Installer for omni (omnipresent) on macOS and Linux.
#
#   curl --proto '=https' --tlsv1.2 -LsSf \
#     https://github.com/JonatanFD/omnipresent/releases/latest/download/install.sh | sh
#
# Downloads the prebuilt `omni` binary for this machine from the latest GitHub
# release and installs it. No Rust toolchain or C compiler required.

set -eu

REPO="JonatanFD/omnipresent"
# Where to install. Override with OMNI_INSTALL_DIR=/some/bin sh install.sh
INSTALL_DIR="${OMNI_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
err() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}
need() { command -v "$1" >/dev/null 2>&1 || err "this installer needs '$1'"; }

need uname
need tar
# One of curl or wget is enough to download.
if command -v curl >/dev/null 2>&1; then
    fetch() { curl --proto '=https' --tlsv1.2 -LsSf "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO "$2" "$1"; }
else
    err "this installer needs 'curl' or 'wget'"
fi

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
    Darwin)
        case "$arch" in
            arm64 | aarch64) target="aarch64-apple-darwin" ;;
            x86_64) target="x86_64-apple-darwin" ;;
            *) err "unsupported macOS architecture: $arch" ;;
        esac
        ;;
    Linux)
        case "$arch" in
            x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
            *) err "unsupported Linux architecture: $arch (only x86_64 is published today)" ;;
        esac
        ;;
    *)
        err "unsupported OS: $os — on Windows use install.ps1"
        ;;
esac

asset="omni-$target.tar.gz"
url="https://github.com/$REPO/releases/latest/download/$asset"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

say "downloading $asset ..."
fetch "$url" "$tmp/$asset" || err "could not download $url"
tar -xzf "$tmp/$asset" -C "$tmp" || err "could not extract $asset"
[ -f "$tmp/omni" ] || err "the archive did not contain an 'omni' binary"

mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/omni" "$INSTALL_DIR/omni" 2>/dev/null ||
    { cp "$tmp/omni" "$INSTALL_DIR/omni" && chmod 755 "$INSTALL_DIR/omni"; }

say "installed omni to $INSTALL_DIR/omni"

# Nudge the user if the install dir is not on PATH.
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        say ""
        say "note: $INSTALL_DIR is not on your PATH. Add it, e.g.:"
        say "  echo 'export PATH=\"$INSTALL_DIR:\$PATH\"' >> ~/.profile && . ~/.profile"
        ;;
esac

say ""
say "done. Try:  omni --help"
