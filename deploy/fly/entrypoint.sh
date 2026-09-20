#!/usr/bin/env bash
# ponytail: single-admin demo deployment. Fly volumes are ext4 (no --reflink=always
# support), so we loop-mount an XFS(reflink=1) image file on top of the volume instead
# of reformatting the volume itself. Upgrade path: a bare-metal host with a native
# Btrfs/XFS root disk (see deploy/ec2-bootstrap.sh) skips this indirection entirely.
set -euo pipefail
umask 077

DATA=/data
MOUNT=/srv/anybranch
IMG="$DATA/anybranch.img"

mkdir -p "$DATA" "$MOUNT"
echo "kernel filesystems: $(tr '\n' ',' < /proc/filesystems)"

if ! mountpoint -q "$MOUNT"; then
  if grep -qw xfs /proc/filesystems; then FSTYPE=xfs
  elif grep -qw btrfs /proc/filesystems; then FSTYPE=btrfs
  else FSTYPE=""; fi

  if [ -n "$FSTYPE" ]; then
    if [ ! -f "$IMG" ]; then
      avail_kb=$(df --output=avail -k "$DATA" | tail -1)
      size_kb=$(( avail_kb - 262144 ))   # leave 256MB headroom on the outer ext4 volume
      [ "$size_kb" -gt 512000 ] || { echo "not enough space on $DATA for the anybranch image" >&2; FSTYPE=""; }
    fi
    if [ -n "$FSTYPE" ]; then
      if [ ! -f "$IMG" ]; then
        fallocate -l "${size_kb}K" "$IMG"
        if [ "$FSTYPE" = xfs ]; then mkfs.xfs -q -m reflink=1 "$IMG"; else mkfs.btrfs -q "$IMG"; fi
      fi
      if ! mount -o loop "$IMG" "$MOUNT"; then
        echo "loop-mount of $FSTYPE image failed; falling back to $DATA directly (no reflink)" >&2
        FSTYPE=""
      fi
    fi
  fi

  if [ -z "$FSTYPE" ]; then
    # ponytail: no reflink-capable filesystem available on this kernel. Branches that
    # rely on `cp --reflink=always` (src/main.rs) will fail loudly instead of silently
    # copying. Upgrade path: a host kernel/volume with xfs or btrfs support.
    MOUNT="$DATA/anybranch"
    mkdir -p "$MOUNT"
  fi
fi

id -u anybranch >/dev/null 2>&1 || useradd --system --create-home --shell /usr/sbin/nologin anybranch
chown -R anybranch:anybranch "$MOUNT" "$DATA"

TOKEN_FILE="$DATA/anybranch.token"
# Keep existing Fly secrets valid during upgrades from the old binary name.
SNAPSHOTDB_TOKEN="${SNAPSHOTDB_TOKEN:-${ANYBRANCH_TOKEN:-}}"
if [ -z "${SNAPSHOTDB_TOKEN:-}" ]; then
  if [ ! -f "$TOKEN_FILE" ]; then
    head -c32 /dev/urandom | od -An -tx1 | tr -d ' \n' > "$TOKEN_FILE"
  fi
  SNAPSHOTDB_TOKEN=$(cat "$TOKEN_FILE")
fi
export SNAPSHOTDB_TOKEN
export SNAPSHOTDB_HOME="$MOUNT"

PUBLIC_HOST="${SNAPSHOTDB_PUBLIC_HOST:-${ANYBRANCH_PUBLIC_HOST:-${FLY_APP_NAME:-anybranch}.internal}}"
echo "snapshotdb serve starting; public-host=$PUBLIC_HOST; token stored at $TOKEN_FILE"
exec runuser -u anybranch -- env "PATH=$PATH" "SNAPSHOTDB_HOME=$SNAPSHOTDB_HOME" "SNAPSHOTDB_TOKEN=$SNAPSHOTDB_TOKEN" \
  snapshotdb serve --bind 0.0.0.0:7432 --db-bind 0.0.0.0 --public-host "$PUBLIC_HOST"
