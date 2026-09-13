#!/usr/bin/env bash
# Run as anybranch. Every test directory lives on the dedicated Btrfs EBS volume.
set -euo pipefail
export PATH=/home/anybranch/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin
export TMPDIR=/srv/anybranch-data/tmp
cd /opt/anybranch-src
for program in pg_ctl psql mysqld mysql mongod mongosh mongodump mongorestore sqlite3; do
  command -v "$program" >/dev/null
done
cargo test --locked -j 2
python3 -m unittest discover -s scripts -p 'test_*.py'
ANYBRANCH_HOME=$(mktemp -d /srv/anybranch-data/tmp/e2e.XXXXXX) ./e2e.sh
python3 scripts/benchmark.py --local --home /srv/anybranch-data/bench \
  --binary /usr/local/bin/anybranch --engine-bin /usr/lib/postgresql/16/bin --target-gb 1
