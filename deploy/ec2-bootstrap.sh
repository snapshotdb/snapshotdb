#!/usr/bin/env bash
# Cloud-init user data for a NEW Ubuntu 24.04 test instance, 16 GiB root + 32 GiB blank data disk.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq bubblewrap btrfs-progs build-essential pkg-config curl ca-certificates gnupg \
  git python3 sqlite3 postgresql-16 postgresql-client-16 mysql-server
systemctl disable --now postgresql mysql

curl -fsSL https://www.mongodb.org/static/pgp/server-8.0.asc | gpg --dearmor -o /usr/share/keyrings/mongodb-server-8.0.gpg
echo 'deb [arch=amd64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/8.0 multiverse' > /etc/apt/sources.list.d/mongodb-org-8.0.list
apt-get update -qq
apt-get install -y -qq mongodb-org-server mongodb-mongosh mongodb-database-tools
systemctl disable --now mongod || true
# The distribution profile restricts mysqld to /var/lib/mysql. Test branches have their own datadirs.
if [ -f /etc/apparmor.d/usr.sbin.mysqld ]; then
  ln -sf /etc/apparmor.d/usr.sbin.mysqld /etc/apparmor.d/disable/usr.sbin.mysqld
  apparmor_parser -R /etc/apparmor.d/usr.sbin.mysqld || true
fi

# Never format a disk with a filesystem or any partitions. This deployment attaches one
# distinct, blank 32 GiB EBS disk; the root disk is 16 GiB.
data_device=''
while read -r device kind; do
  [ "$kind" = disk ] || continue
  [ "$(blockdev --getsize64 "$device")" = 34359738368 ] || continue
  [ "$(lsblk -nr "$device" | wc -l)" = 1 ] || continue
  if blkid "$device" >/dev/null 2>&1; then continue; fi
  [ -z "$data_device" ] || { echo 'More than one candidate data disk; refusing to format'; exit 1; }
  data_device=$device
done < <(lsblk -dnpo NAME,TYPE)
[ -n "$data_device" ] || { echo 'No blank 32 GiB data disk found'; exit 1; }
mkfs.btrfs -q -L anybranch-data "$data_device"
mkdir -p /srv/anybranch-data
echo "UUID=$(blkid -s UUID -o value "$data_device") /srv/anybranch-data btrfs defaults,noatime 0 0" >> /etc/fstab
mount /srv/anybranch-data
useradd --create-home --shell /bin/bash anybranch
install -d -o anybranch -g anybranch -m 700 /srv/anybranch-data/server /srv/anybranch-data/bench /srv/anybranch-data/tmp
ln -s /srv/anybranch-data/server /srv/anybranch
install -d -o anybranch -g anybranch /opt/anybranch-src

fallocate -l 2G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/anybranch-rustup.sh
runuser -u anybranch -- sh /tmp/anybranch-rustup.sh -y --profile minimal
rm /tmp/anybranch-rustup.sh

umask 027
{
  printf 'ANYBRANCH_TOKEN=%s\n' "$(openssl rand -hex 32)"
  printf '%s\n' 'ANYBRANCH_HOME=/srv/anybranch' 'ANYBRANCH_SERVER=http://127.0.0.1:7432' \
    'PATH=/home/anybranch/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin'
} > /etc/anybranch.env
chown root:anybranch /etc/anybranch.env
chmod 640 /etc/anybranch.env
touch /var/lib/anybranch-bootstrap-ready
