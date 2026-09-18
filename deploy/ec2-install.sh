#!/usr/bin/env bash
# Run on the provisioned test instance after uploading the source archive.
set -euo pipefail
test -f /var/lib/snapshotdb-bootstrap-ready
tar -xzf /tmp/snapshotdb-source.tar.gz -C /opt/snapshotdb-src
chown -R snapshotdb:snapshotdb /opt/snapshotdb-src
bash /opt/snapshotdb-src/deploy/install-sandbox.sh
runuser -u snapshotdb -- bash -c '
  export PATH=/home/snapshotdb/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin
  cd /opt/snapshotdb-src
  cargo build --release --locked -j 2
'
install -m 755 /opt/snapshotdb-src/target/release/snapshotdb /usr/local/bin/snapshotdb
sed 's/--db-bind 0.0.0.0 --public-host branches.internal/--db-bind 127.0.0.1 --public-host 127.0.0.1/' \
  /opt/snapshotdb-src/deploy/snapshotdb.service > /etc/systemd/system/snapshotdb.service
systemctl daemon-reload
systemctl enable --now snapshotdb
systemctl is-active --quiet snapshotdb
