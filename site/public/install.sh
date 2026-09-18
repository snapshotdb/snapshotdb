#!/bin/sh
# SnapshotDB installer — https://www.snapshotdb.io
#   curl -fsSL https://www.snapshotdb.io/install.sh | sh
set -e

REPO="${SNAPSHOTDB_REPO:-GitHoobar/anybranch}"   # repo rename to snapshotdb pending
BIN="snapshotdb"
PREFIX="${SNAPSHOTDB_PREFIX:-/usr/local/bin}"

say() { printf '\033[1msnapshotdb\033[0m %s\n' "$1"; }
err() { printf 'error: %s\n' "$1" >&2; exit 1; }

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$arch" in
  x86_64|amd64) arch=x86_64 ;;
  arm64|aarch64) arch=aarch64 ;;
  *) err "unsupported architecture: $arch" ;;
esac
case "$os" in
  darwin) target="$arch-apple-darwin" ;;
  linux)  target="$arch-unknown-linux-gnu" ;;
  *) err "unsupported OS: $os" ;;
esac

say "installing for $target"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
url="https://github.com/$REPO/releases/latest/download/$BIN-$target.tar.gz"

if curl -fsSL "$url" -o "$tmp/$BIN.tar.gz" 2>/dev/null; then
  tar -xzf "$tmp/$BIN.tar.gz" -C "$tmp"
  if [ -w "$PREFIX" ]; then
    mv "$tmp/$BIN" "$PREFIX/$BIN" && chmod +x "$PREFIX/$BIN"
  else
    say "writing to $PREFIX (needs sudo)"
    sudo mv "$tmp/$BIN" "$PREFIX/$BIN" && sudo chmod +x "$PREFIX/$BIN"
  fi
  say "installed to $PREFIX/$BIN"
elif command -v cargo >/dev/null 2>&1; then
  say "no prebuilt binary for $target — building from source with cargo"
  cargo install --git "https://github.com/$REPO" --locked
  say "installed via cargo"
else
  err "no prebuilt binary for $target and cargo not found.
  install Rust (https://rustup.rs) and re-run, or download a release:
  https://github.com/$REPO/releases"
fi

say "done — run '$BIN --help' · docs: https://www.snapshotdb.io/docs"
