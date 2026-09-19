# 1 TB benchmark — passed, data retained

A real decimal 1 TB (`1,000,000,000,000` bytes of PostgreSQL tables/indexes) benchmark
started at `2026-09-12T22:26:55.990285+00:00`. The core benchmark passed all 31 checks;
the retained follow-up checks and engine regression suite also passed. Initial copying
took **10,161.477 seconds (2 h 49 m 21 s)**, excluding dummy-data generation. This was
the measured initial-copy phase of this run, not an estimate for all 1 TB databases.
See [current branch latency results](latency-results.md) and the
[Ardent comparison](ardent-comparison.md) for the subsequent optimized branch tests.

| Resource | Current setting |
| --- | --- |
| Account / region | `686255969284` / `ap-south-1` |
| EC2 | `i-0c0d826d940910b44`, temporarily resized to `c6i.4xlarge` |
| CPU / memory | 16 vCPUs / 32 GiB |
| Temporary benchmark volume | `vol-03e3c3899559d783e`, 3,072 GiB encrypted gp3 |
| Volume performance | 6,000 IOPS, 500 MiB/s |
| Filesystem | Btrfs without compression, `/srv/snapshotdb-tb` |
| Run directory | `/srv/snapshotdb-tb/scale-xrgarhz2` |
| Existing demo | Preserved on its original 32 GiB volume |

The user requested retention: **keep the test databases and extra disk after the run**.
The benchmark uses `--target-gb 1000 --keep`, with a 48-hour initial-copy timeout.
The systemd job has a 72-hour overall runtime limit and does not restart/replay automatically.
The first short generation attempt was stopped and removed to switch to retention mode;
the run directory above is the active run.

## Performance upgrade and resume

The initial 125 MiB/s volume was saturated while data grew at roughly 36–38 MB/s.
The disk was raised online to 500 MiB/s and 6,000 IOPS, and EC2 was resized to
`c6i.4xlarge`, which has 625 MB/s baseline EBS throughput. These are hardware limits,
not promised database throughput.

The run was paused with **82,010,734,592 bytes** reported as generated. Its existing
source directory was retained for resume. Before resizing, a deliberately interrupted
small benchmark resumed the original database and passed 29 checks; all 7 Python
unit tests also passed. The production benchmark uses `--resume` on the same directory.
The progress file retains the original start time and records a separate resume time
and attempt duration. Final report timings must be interpreted as a run across different
hardware configurations.

After resume, generation measured **152.9 MB/s** over
45.4 seconds, compared with roughly 36–38 MB/s before the upgrades.
At the end of that sample, **103,162,642,432 bytes** had been generated
(10.316%). This is a short observation, not a full-run throughput
guarantee. The sample is recorded in `.local/aws-mumbai/tb-speed-up-report.json`.

## Read progress

From this checkout:

```sh
ssh -F .local/aws-mumbai/ssh-config snapshotdb-mumbai \
  'sudo cat /var/lib/snapshotdb-benchmark/progress.json'
```

On EC2:

```sh
sudo systemctl status snapshotdb-benchmark-1tb
sudo tail -n 20 /var/lib/snapshotdb-benchmark/benchmark.log
sudo cat /var/lib/snapshotdb-benchmark/progress.json
```

The wrapper stores progress, logs, and the final `report.json` under
`/var/lib/snapshotdb-benchmark`, outside the large test volume. Execution continues if
SSH disconnects or the laptop sleeps. A separate read-only laptop process mirrors
progress to `.local/aws-mumbai/tb-progress.json` while the laptop is available and
copies the final report to `.local/aws-mumbai/tb-report.json` when it observes completion.
The authoritative status remains on EC2.

The scale test checks filesystem reflinks, measured table/index size, complete initial
copy, row counts for all eight large tables, live updates, schema changes, two isolated
branches, and suspend/resume. Retention mode omits the destructive replication-slot
cleanup check; the retained replica must keep its source slot to continue replicating.
A failed run is preserved for investigation.

## Automatic checks after the benchmark

After the scale benchmark passes, the completion hook starts the retained service and
queues `snapshotdb-post-benchmark.service` without waiting inside the shutdown hook.
No further user prompt is needed. The follow-up stage:

- Checks authentication and the four retained databases after service handoff.
- Creates two temporary branches from the retained replica, checks endpoints in all
  eight large tables, and verifies write isolation, suspend/resume, locks, and reset.
- Restarts the retained service and checks persisted writes and continued replication.
- Removes only the temporary branches, keeping the source, replica, `branch-a`, and
  `branch-b`. A small uniquely named `public.post_check_*` fixture table is retained
  on the source and replica for inspection.
- Runs all Rust and Python tests and the PostgreSQL/MySQL/MongoDB end-to-end suite.
  SQLite branching is covered by Rust tests. Temporary directories use Btrfs.

The stage has a six-hour runtime limit. Its durable report is
`/var/lib/snapshotdb-benchmark/post-test-report.json`; private command output is in
`post-test-report.log`. The progress file's `passed` field covers the core benchmark;
**only `all_tests_passed: true` confirms that the follow-up stage also passed**.
Interrupted or failed post-test services are marked failed explicitly. The laptop
monitor also mirrors the final follow-up report to `.local/aws-mumbai/tb-post-test-report.json`.

```sh
sudo systemctl status snapshotdb-post-benchmark
sudo cat /var/lib/snapshotdb-benchmark/post-test-report.json
```

The automation was validated on a separate small fixture: 45 retained-database checks,
94 engine checks, 7 Python tests, and 8 Rust tests. The first Rust run found that `/tmp`
did not support reflinks; setting `TMPDIR` to Btrfs fixed it and the affected suite passed
on rerun. Three additional checks verified that timeouts and service errors cannot mark
all tests passed. The subsequent retained 1 TB run passed 45 checks, followed by
94 engine checks, 7 Python tests, and 8 Rust tests.

## Use the retained databases after success

After a successful run, `snapshotdb-tb.service` starts automatically and is enabled at
boot. It uses the retained run directory and a new API token from `/etc/snapshotdb-tb.env`.
The existing demo service remains separate. The retained API binds server loopback
port `7433`; its database proxies also bind loopback.

```sh
sudo systemctl status snapshotdb-tb
sudo -u snapshotdb bash -c 'set -a; source /etc/snapshotdb-tb.env; export SNAPSHOTDB_SERVER=http://127.0.0.1:7433; snapshotdb list'
```

The expected retained databases are `source`, `replica`, `branch-a`, and `branch-b`.
Use SSH forwarding for laptop access. Credentials remain private; do not publish logs
or `/etc/snapshotdb-tb.env`.

## Cost and lifecycle

AWS Price List API rates checked for Mumbai: `c6i.4xlarge` Linux `$0.68/hour`, gp3
`$0.0912/GiB-month`, and public IPv4 `$0.005/hour`. Including the original 48 GiB of
volumes plus the new 3,072 GiB volume, 375 additional MiB/s at `$0.0456/MiB/s-month`,
and 3,000 additional IOPS at `$0.0057/IOPS-month`, this is approximately **$1.12/hour,
$27/day, or $819/month** at 730 running hours, before tax and transfer.

Both the larger instance and the extra disk remain provisioned. No automatic deletion
or downsizing is scheduled. Stopping EC2 stops compute charges but keeps storage charges;
this 3 TiB disk with its performance settings is about **$314/month** until deleted. See
[AWS EBS pricing](https://aws.amazon.com/ebs/pricing/) for the billing model.

The original `t3.medium` size is recorded in `.local/aws-mumbai/tb-deployment.json` for
later downsizing. Deleting this benchmark volume destroys the retained 1 TB datasets;
do so only when they are no longer wanted. The original demo volume is separate.
