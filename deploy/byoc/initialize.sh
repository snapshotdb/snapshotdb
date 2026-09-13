#!/usr/bin/env bash
# Customer first boot: use an explicitly mounted, empty Btrfs directory.
# This command never formats disks. A second invocation never rotates credentials.
set -euo pipefail
test "$(id -u)" = 0
data=${1:?Usage: anybranch-initialize /mounted/btrfs/directory}
data=$(realpath -e "$data")
test -d "$data"
test "$(stat -f -c %T "$data")" = btrfs || { echo 'A mounted Btrfs data volume is required' >&2; exit 1; }
exec 9>/run/anybranch-initialize.lock
flock -x 9
if [ -e /etc/anybranch.env ] || [ -e /srv/anybranch ] || [ -L /srv/anybranch ]; then
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
install -d -o anybranch -g anybranch -m 700 "$data/server"
ln -s "$data/server" /srv/anybranch
umask 077
token=$(openssl rand -hex 32)
printf 'ANYBRANCH_TOKEN=%s\nANYBRANCH_HOME=/srv/anybranch\n' "$token" > /etc/anybranch.env
chmod 600 /etc/anybranch.env
systemctl enable --now anybranch
for _ in $(seq 1 100); do
  if curl -fsS -H "Authorization: Bearer $token" http://127.0.0.1:7432/v1/health >/dev/null; then
    echo 'Anybranch ready on loopback port 7432; credentials are in /etc/anybranch.env.'
    exit 0
  fi
  sleep .1
done
echo 'Service did not become healthy; inspect journalctl -u anybranch.' >&2
exit 1
