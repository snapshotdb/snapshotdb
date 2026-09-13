#!/usr/bin/env bash
# Bake a clean Ubuntu 24.04 amd64 appliance. No databases or credentials are baked.
set -euo pipefail
test "$(id -u)" = 0
source /etc/os-release
test "$ID:$VERSION_ID:$(dpkg --print-architecture)" = ubuntu:24.04:amd64
profile=${1:-postgres}
case "$profile" in postgres|all) ;; *) echo 'profile must be postgres or all' >&2; exit 1;; esac
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq btrfs-progs build-essential pkg-config curl ca-certificates \
  gnupg python3 sqlite3 postgresql-16 postgresql-client-16
systemctl disable --now postgresql
bash "$repo/deploy/install-sandbox.sh"
if [ "$profile" = all ]; then
  apt-get install -y -qq mysql-server
  systemctl disable --now mysql
  if [ -f /etc/apparmor.d/usr.sbin.mysqld ]; then
    mkdir -p /etc/apparmor.d/disable
    ln -sf /etc/apparmor.d/usr.sbin.mysqld /etc/apparmor.d/disable/usr.sbin.mysqld
    apparmor_parser -R /etc/apparmor.d/usr.sbin.mysqld
  fi
  curl -fsSL https://www.mongodb.org/static/pgp/server-8.0.asc | gpg --dearmor --yes -o /usr/share/keyrings/mongodb-server-8.0.gpg
  echo 'deb [arch=amd64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/8.0 multiverse' > /etc/apt/sources.list.d/mongodb-org-8.0.list
  apt-get update -qq
  apt-get install -y -qq mongodb-org-server mongodb-mongosh mongodb-database-tools
  systemctl disable --now mongod
fi
id anybranch >/dev/null 2>&1 || useradd --create-home --shell /bin/bash anybranch
install -d -o anybranch -g anybranch /opt/anybranch-src
cp -a "$repo/Cargo.toml" "$repo/Cargo.lock" "$repo/src" /opt/anybranch-src/
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/anybranch-rustup.sh
runuser -u anybranch -- sh /tmp/anybranch-rustup.sh -y --profile minimal
rm /tmp/anybranch-rustup.sh
chown -R anybranch:anybranch /opt/anybranch-src
runuser -u anybranch -- bash -c 'cd /opt/anybranch-src; /home/anybranch/.cargo/bin/cargo build --release --locked -j 2'
install -m 755 /opt/anybranch-src/target/release/anybranch /usr/local/bin/anybranch
install -m 755 "$here/initialize.sh" /usr/local/sbin/anybranch-initialize
install -m 644 "$here/anybranch.service" /etc/systemd/system/anybranch.service
install -d /usr/share/anybranch
sha256sum /usr/local/bin/anybranch > /usr/share/anybranch/binary.sha256
printf '%s\n' "$profile" > /usr/share/anybranch/engine-profile
dpkg-query -W > /usr/share/anybranch/packages.txt
systemctl daemon-reload
# Deliberately disabled until a customer explicitly initializes their data volume.
systemctl disable anybranch
test ! -e /etc/anybranch.env
test ! -e /srv/anybranch
