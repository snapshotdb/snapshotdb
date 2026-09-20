#!/bin/sh
# SnapshotDB installer — https://www.snapshotdb.io
#   curl -fsSL https://www.snapshotdb.io/install.sh | sh
set -e

BASE_URL="${SNAPSHOTDB_BASE_URL:-https://www.snapshotdb.io/dl}"
BIN="snapshotdb"
PREFIX="${SNAPSHOTDB_PREFIX:-/usr/local/bin}"

say() { printf '\033[1msnapshotdb\033[0m %s\n' "$1"; }
err() { printf 'error: %s\n' "$1" >&2; exit 1; }

os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$arch" in
  x86_64|amd64) arch=amd64 ;;
  arm64|aarch64) arch=arm64 ;;
  *) err "unsupported architecture: $arch" ;;
esac
case "$os" in
  darwin) asset="$BIN-darwin-$arch" ;;
  linux)
    [ "$arch" = amd64 ] || err "unsupported architecture for linux: $arch (only amd64 is published)"
    asset="$BIN-linux-$arch"
    ;;
  *) err "unsupported OS: $os" ;;
esac

say "installing $asset"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
url="$BASE_URL/$asset"

curl -fsSL "$url" -o "$tmp/$asset" || err "download failed: $url"
curl -fsSL "$BASE_URL/SHA256SUMS.txt" -o "$tmp/SHA256SUMS.txt" || err "checksum download failed"
awk -v asset="$asset" '$2 == asset {print}' "$tmp/SHA256SUMS.txt" > "$tmp/checksum"
[ "$(wc -l < "$tmp/checksum" | tr -d ' ')" = 1 ] || err "missing or duplicate checksum for $asset"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmp" && sha256sum -c checksum) || err "checksum mismatch"
else
  (cd "$tmp" && shasum -a 256 -c checksum) || err "checksum mismatch"
fi
mv "$tmp/$asset" "$tmp/$BIN"
chmod +x "$tmp/$BIN"

if [ -w "$PREFIX" ]; then
  mv "$tmp/$BIN" "$PREFIX/$BIN"
else
  say "writing to $PREFIX (needs sudo)"
  sudo mkdir -p "$PREFIX"
  sudo mv "$tmp/$BIN" "$PREFIX/$BIN"
fi

say "installed to $PREFIX/$BIN"
say "done — run '$BIN --help' · docs: https://www.snapshotdb.io/docs"
