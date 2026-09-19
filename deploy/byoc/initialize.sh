#!/usr/bin/env bash
# Customer first boot: use an explicitly mounted, empty Btrfs directory.
# This command never formats disks. A second invocation never rotates credentials.
set -euo pipefail
test "$(id -u)" = 0
data=${1:?Usage: snapshotdb-initialize /mounted/btrfs/directory}
data=$(realpath -e "$data")
test -d "$data"
test "$(stat -f -c %T "$data")" = btrfs || { echo 'A mounted Btrfs data volume is required' >&2; exit 1; }
exec 9>/run/snapshotdb-initialize.lock
flock -x 9
if [ -e /etc/snapshotdb.env ] || [ -e /srv/snapshotdb ] || [ -L /srv/snapshotdb ]; then
  echo 'Already initialized or partially configured; refusing to overwrite storage or credentials.' >&2
  exit 1
fi
test -z "$(find "$data" -mindepth 1 -maxdepth 1 -print -quit)" || { echo 'Data directory must be empty' >&2; exit 1; }
# Verify actual copy-on-write support on the destination before writing configuration.
probe=$(mktemp -d "$data/.reflink-check.XXXXXX")
trap 'rm -rf "$probe"' EXIT
dd if=/dev/urandom of="$probe/source" bs=4096 count=1 status=none
cp --reflink=always "$probe/source" "$probe/clone"
rm -rf "$probe"
install -d -o snapshotdb -g snapshotdb -m 700 "$data/server"
ln -s "$data/server" /srv/snapshotdb
umask 077
token=$(openssl rand -hex 32)
printf 'SNAPSHOTDB_TOKEN=%s\nSNAPSHOTDB_HOME=/srv/snapshotdb\n' "$token" > /etc/snapshotdb.env
chmod 600 /etc/snapshotdb.env
systemctl enable --now snapshotdb
for _ in $(seq 1 100); do
  if curl -fsS -H "Authorization: Bearer $token" http://127.0.0.1:7432/v1/health >/dev/null; then
    echo 'SnapshotDB ready on loopback port 7432; credentials are in /etc/snapshotdb.env.'
    exit 0
  fi
  sleep .1
done
echo 'Service did not become healthy; inspect journalctl -u snapshotdb.' >&2
exit 1
