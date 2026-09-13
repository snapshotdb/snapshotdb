#!/usr/bin/env bash
# Run on the provisioned test instance after uploading the source archive.
set -euo pipefail
test -f /var/lib/anybranch-bootstrap-ready
tar -xzf /tmp/anybranch-source.tar.gz -C /opt/anybranch-src
chown -R anybranch:anybranch /opt/anybranch-src
runuser -u anybranch -- bash -c '
  export PATH=/home/anybranch/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin
  cd /opt/anybranch-src
  cargo build --release --locked -j 2
'
install -m 755 /opt/anybranch-src/target/release/anybranch /usr/local/bin/anybranch
sed 's/--db-bind 0.0.0.0 --public-host branches.internal/--db-bind 127.0.0.1 --public-host 127.0.0.1/' \
  /opt/anybranch-src/deploy/anybranch.service > /etc/systemd/system/anybranch.service
systemctl daemon-reload
systemctl enable --now anybranch
systemctl is-active --quiet anybranch
