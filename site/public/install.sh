#!/usr/bin/env bash
# Installs the anybranch CLI. See https://www.snapshotdb.io/docs/install
set -euo pipefail

BASE_URL="https://www.snapshotdb.io/dl"
INSTALL_DIR="${ANYBRANCH_INSTALL_DIR:-/usr/local/bin}"

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Darwin)
    case "$arch" in
      arm64) asset="anybranch-darwin-arm64" ;;
      x86_64) asset="anybranch-darwin-amd64" ;;
      *) echo "error: unsupported macOS architecture: $arch" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    case "$arch" in
      x86_64) asset="anybranch-linux-amd64" ;;
      *) echo "error: unsupported Linux architecture: $arch (only x86_64 is published)" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "error: unsupported OS: $os" >&2
    exit 1
    ;;
esac

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
echo "Downloading $asset..."
curl -fsSL "$BASE_URL/$asset" -o "$tmp"
chmod +x "$tmp"

dest="$INSTALL_DIR/anybranch"
if [ -w "$INSTALL_DIR" ]; then
  mv "$tmp" "$dest"
else
  echo "Need sudo to write to $INSTALL_DIR"
  sudo mv "$tmp" "$dest"
fi

echo "Installed anybranch to $dest"
"$dest" --help 2>&1 | head -1 || true
echo
echo "Set these before using database commands:"
echo "  export ANYBRANCH_SERVER=https://your-server"
echo "  export ANYBRANCH_TOKEN='<server access token>'"
