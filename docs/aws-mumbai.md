# Mumbai test deployment

## Retained 1 TB run

The instance is currently resized to `c6i.4xlarge` with an additional 3 TiB benchmark
volume. See [current benchmark status, retention, and cost](tb-benchmark.md). The
small-host configuration and prices below describe the initial deployment.

This is a small, single-host development deployment in the Alphawave AWS account.
It is sized for functional tests and a 1 GB dataset, not the 1 TB benchmark.

| Setting | Value |
| --- | --- |
| AWS account / profile | `686255969284` / `default` |
| Region | `ap-south-1` (Mumbai) |
| EC2 instance | `i-0c0d826d940910b44` |
| Instance type | `t3.medium`, 2 vCPUs, 4 GiB RAM, standard CPU credits |
| OS | Ubuntu 24.04, x86_64 |
| Root disk | 16 GiB, encrypted gp3 |
| Database disk | 32 GiB, encrypted gp3, Btrfs |
| Database storage | `/srv/anybranch-data/server`, linked from `/srv/anybranch` |
| Test storage | `/srv/anybranch-data/bench` and `/srv/anybranch-data/tmp` |
| Control API | Server loopback port `7432` |
| SSH security group | `sg-0e8eeac4e88efa1e3`, port 22 from the operator's launch-time public IP only |

The API and database proxies bind loopback. Access uses SSH tunnels; no database or API
ports are exposed to the internet. The source checkout and build live in `/opt/anybranch-src`.
PostgreSQL 16, MySQL 8, MongoDB 8, and SQLite are installed. Their distribution services are
disabled so Anybranch manages its own instances. The `anybranch` systemd service starts at boot.

The later Ardent comparison has a separate TLS source endpoint on port 15450,
restricted to Ardent's reported egress IP and requiring client certificate authentication.
See [comparison configuration and status](ardent-comparison.md).

Local connection files are in the ignored, private `.local/aws-mumbai/` directory. They
include `deployment.json`, an SSH key, SSH configuration, and any client environment files
created during setup. Do not commit these files. The server API token lives in
`/etc/anybranch.env`, readable by root and the `anybranch` service group.

## Connect and inspect

The initial setup leaves `demo-source`, `demo-replica`, and `demo-branch` on the server,
with a 1,000-row `public.anybranch_demo` table. The laptop-to-server test changes row 1
only in the branch and rows 2–3 only in the source/replica.

While the setup's SSH tunnels are running, use the existing demo from this checkout:

```sh
source .local/aws-mumbai/client.env
./target/release/anybranch list
psql "$DEMO_DATABASE_URL" -c 'SELECT * FROM public.anybranch_demo WHERE id <= 3'
```

The private `client.env` contains the API token and branch connection URL. The initial
local forwards use port `17432` for the API and `15433` for the demo branch. SSH tunnel
process IDs are recorded in `.local/aws-mumbai/tunnel-pids.json`.

From this checkout:

```sh
ssh -F .local/aws-mumbai/ssh-config anybranch-mumbai
```

On the server:

```sh
sudo systemctl status anybranch
sudo journalctl -u anybranch -n 50
df -h /srv/anybranch-data
sudo -u anybranch bash -c 'set -a; source /etc/anybranch.env; anybranch list'
```

If the setup tunnels have exited, forward the API and existing demo branch from another terminal:

```sh
ssh -F .local/aws-mumbai/ssh-config -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -N -L 17432:127.0.0.1:7432 -L 15433:127.0.0.1:44841 anybranch-mumbai
```

The laptop CLI then uses `ANYBRANCH_SERVER=http://127.0.0.1:17432` and the server token.
Database URLs contain the server's loopback address. Forward each database port separately
before connecting from the laptop. This preserves the client/server architecture: database
files and engines remain on EC2.

## Verified results

The deployed 0.4.0 source passed 8 Rust tests, 5 Python tests, and all 94 engine
end-to-end checks across PostgreSQL, MySQL, and MongoDB. SQLite branching and remote
job execution are covered by the Rust tests.

The PostgreSQL scale run passed all 28 checks using **1,001,914,368 bytes** of table/index
data across 483,616 rows in eight tables. Initial replication took 63.8 seconds;
the complete run took 123.5 seconds. Two isolated branches took 7.0 and 1.0 seconds
to create. These are single-run observations on this small host, not a capacity guarantee.
The test verified row counts, live updates, schema changes, isolation, cold resume,
and replication-slot cleanup. Test databases were removed after completion.

The report remains at `/srv/anybranch-data/bench/scale-1ztby0rd/report.json` (read as
`anybranch` or with `sudo`), with a local copy at `.local/aws-mumbai/benchmark-report.json`.
Its filesystem-wide free-space delta includes background allocation/reclamation and
must not be interpreted as branch-exclusive storage usage. Copy-on-write support was
verified separately with a mandatory reflink copy.

Nine laptop-to-EC2 checks confirmed the demo's replication and isolation, including
that the client created no local database directory. Five additional checks passed after
a systemd service restart: API access, persisted branch writes, continued replication,
branch isolation, and absence of client database files. SSH keepalives are enabled in
the local configuration. The 1 TB benchmark has not been
run; it needs a larger data volume and a suitably sized host.

## Repeat tests

```sh
sudo -u anybranch bash /tmp/ec2-test.sh
```

The reproducible scripts are [bootstrap](../deploy/ec2-bootstrap.sh),
[install](../deploy/ec2-install.sh), and [test](../deploy/ec2-test.sh). Bootstrap formats
only a new, blank 32 GiB data disk and is intended for a new instance. Installation uses
the uploaded source archive, so it includes the current unreleased client/server changes.

## Cost and lifecycle

AWS Price List API rates checked at deployment: compute `$0.0448/hour`, gp3 storage
`$0.0912/GiB-month`, and public IPv4 `$0.005/hour`. With 48 GiB total EBS storage and
730 running hours, the estimate is **$40.73/month**, excluding tax, data transfer, and
optional services. Standard CPU credits avoid unlimited-mode surplus CPU charges.

Stop the instance when it is not needed:

```sh
aws --profile default ec2 stop-instances --region ap-south-1 --instance-ids i-0c0d826d940910b44
```

Stopping retains the EBS volumes and their storage charges. After starting it again, get
its new public IP with `describe-instances` and update the local SSH configuration.
If the operator's public IP changes, update this security group's SSH source rule.

Terminating the instance deletes both attached volumes, including databases and reports:

```sh
aws --profile default ec2 terminate-instances --region ap-south-1 --instance-ids i-0c0d826d940910b44
```

The dedicated security group and imported key pair can be removed afterward using the IDs
in `.local/aws-mumbai/deployment.json`. Existing account instances are unrelated to this deployment.
