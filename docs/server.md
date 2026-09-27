# Deploy SnapshotDB

```text
Developer CLI --HTTPS + token--> SnapshotDB server --replication--> Source database
                                  |
                                  +-- full replica on server storage
                                  +-- isolated branch A (shared blocks until writes)
                                  +-- isolated branch B (shared blocks until writes)

Developer application --private network / tunnel--> branch database port on server
```

The source connection string is sent to the server. The CLI does not download database
files or start database engines. A deployment is required; there is no local fallback.
Copy-on-write avoids copying the full replica for each branch. It does not eliminate the
initial transfer from the source or the replica's storage requirement.

For agent allocation from an already-synced database, use
[prepared snapshots and running branch pools](prepared-branches.md). This moves engine
startup and cloning into preparation and returns a private, writable endpoint at claim time.

## Server setup

Use a dedicated Linux host with Btrfs or XFS with reflink enabled. Put all SnapshotDB data
on that filesystem. Run as an ordinary service user with engine binaries on its PATH.
For PostgreSQL, use PostgreSQL 15+ and matching `pg_ctl`, `initdb`, `psql`, `pg_dump`, and
`pg_dumpall` binaries. For the other engines see the main README.
Install bubblewrap before creating PostgreSQL, MySQL or MongoDB branches. On Ubuntu,
run `sudo bash deploy/install-sandbox.sh`; it installs a scoped AppArmor permission for
bubblewrap without disabling the global user-namespace restriction. If namespaces or
bubblewrap are unavailable, branch startup fails instead of running unsandboxed.

Build this checkout on the server with `cargo build --release --locked`, then install
`target/release/snapshotdb` as `/usr/local/bin/snapshotdb`. The client must use the matching
client/server version; v0.3.0 and earlier clients execute databases locally.

Example server environment (store the token in a mode-0600 environment file):

```sh
export SNAPSHOTDB_HOME=/srv/snapshotdb
export SNAPSHOTDB_TOKEN='<at least 32 random printable characters>'
export PATH=/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/bin:/bin
snapshotdb serve --bind 127.0.0.1:7432 \
  --db-bind 0.0.0.0 --public-host branches.internal
```

Generate the token with `openssl rand -hex 32`. Terminate HTTPS at a reverse proxy in front
of `127.0.0.1:7432`. Set the request-body limit to 64 KiB. The CLI refuses plain HTTP to a
non-loopback address and does not follow redirects with credentials.

`branches.internal` must resolve to this host for both the server and client applications.
Database proxy ports are allocated dynamically. Allow them only on your private network or
VPN; the proxy forwards native database traffic and does not add database TLS. The API
token is an administrative credential for the whole deployment. This is a single-admin
deployment, not a multi-tenant public service.

A systemd unit is provided in [deploy/snapshotdb.service](../deploy/snapshotdb.service).
Create the service user and writable `/srv/snapshotdb` directory first, and set the token
and PostgreSQL PATH in `/etc/snapshotdb.env`. Adjust the hostname and paths in the unit for
your host. Starting `serve` restores existing branch proxies and synced roots.

## Client use

```sh
export SNAPSHOTDB_SERVER=https://snapshotdb.example.com
export SNAPSHOTDB_TOKEN='<server token>'
snapshotdb clone prod 'postgresql://replication-user:password@source.internal/app'
snapshotdb status prod
snapshotdb create feature-x --from prod --print-url
```

The resulting URL points at `branches.internal`, with the branch's own credentials. A
PostgreSQL clone returns while its initial copy is still running; wait until every table
is ready before treating it as a complete snapshot. Branch creation refuses a PostgreSQL
replica that is still copying or paused. Once ready, cloning briefly
stops the replica to get consistent files; it does not stop the production source.

Long-running operations can be submitted with `--detach`, which prints a job ID. Run
`snapshotdb job <id>` to wait later. Jobs continue if the client disconnects. Completed
results are stored in the server's private `.jobs` directory and may contain database
URLs. Remove old completed job files when they are no longer needed. After a server restart,
interrupted jobs require inspection before retrying; commands are never blindly replayed.

The server runs up to four workspace jobs concurrently, preserving submission order
within each workspace. A workspace can have at most eight running or queued jobs;
the shared pending queue holds 32. Full queues return HTTP 503 so callers can retry
after existing work finishes. Jobs have a one-hour execution deadline; read-only
commands and preflight checks are capped at 60 seconds. Set
`SNAPSHOTDB_JOB_TIMEOUT_SECONDS` (1–86400) to change the overall deadline.
Timeouts and shutdown interrupt the worker and its client subprocesses, returning
exit code 124. Changes already made are not rolled back: inspect the source or
branch before retrying. Queued work is marked unexecuted on orderly shutdown.

For an SSH-only setup, leave the API and database proxies bound to loopback, and forward
the API with `ssh -N -L 7432:127.0.0.1:7432 user@server`. Set the client API URL to
`http://127.0.0.1:7432`. Forward each returned database port separately before connecting.
All database files still remain on the remote server.

## Scale test

The runner starts a separate test server and a dummy PostgreSQL source on the remote host,
then uses the real client/API path to create the replica and two branches. It does not use
an existing production source. SSH is used only to launch the benchmark and report results;
normal product commands use the deployed API directly.

Remote prerequisites: the built v0.4.0 binary, Python 3, PostgreSQL 15+ tools, `findmnt`,
and a writable reflink filesystem without compression. Run as a non-root user. A real
1 TB source plus its independent replica needs at least 2 TB before WAL and branch writes.
The runner requires **3 × target bytes + 2 GB free**, so provision more than 3 TB free for
the 1 TB run. This is a minimum preflight reserve, not a guaranteed maximum disk budget.

For development, explicitly run the same test server locally on macOS/APFS or Linux:

```sh
python3 scripts/benchmark.py --local --home /tmp/snapshotdb-bench \
  --binary "$PWD/target/release/snapshotdb" \
  --engine-bin /opt/homebrew/opt/postgresql@17/bin --target-mb 100
```

The CLI still talks to a separate authenticated server process. `--local` chooses where
the disposable test server runs; ordinary product commands still require a configured
server and SSH test runs never fall back locally. macOS uses APFS cloning; Linux uses
reflink cloning. Small tests retain at least 10,000 rows per table for the write-isolation
checks, so measured size can exceed a small target. The same capacity check applies locally.
For a local 1 TB test, select an external APFS or reflink volume with enough free space
using `--home` and set `--target-gb 1000`.

Start with a 1 GB smoke run:

```sh
python3 scripts/benchmark.py --host user@server --home /srv/snapshotdb-bench \
  --binary /usr/local/bin/snapshotdb --engine-bin /usr/lib/postgresql/16/bin \
  --target-gb 1
```

Then run **1 TB = 1,000,000,000,000 bytes**:

```sh
python3 scripts/benchmark.py --host user@server --home /srv/snapshotdb-bench \
  --binary /usr/local/bin/snapshotdb --engine-bin /usr/lib/postgresql/16/bin \
  --target-gb 1000 --timeout-hours 48
```

Data is generated in bounded transactions across eight keyed tables. PostgreSQL PLAIN
column storage and disabled filesystem compression keep repetitive synthetic payloads from
collapsing into a tiny dataset. The target is measured table/index storage, not nominal
row count, an empty allocated file, or a production performance model. See PostgreSQL's
[storage options](https://www.postgresql.org/docs/17/sql-altertable.html) and
[subscription readiness states](https://www.postgresql.org/docs/16/catalog-pg-subscription-rel.html).

The report records measured size, row counts, generation/copy/branch/resume timings,
filesystem free-space change, live replication, DDL replay, detached subscriptions,
isolation from writes in both directions, and slot cleanup. Filesystem free-space change
includes other activity on the volume; it is not an exact per-branch physical-space metric.
The test does not validate performance under concurrent application load or across all
three network database engines.

The runner prints its remote `scale-*/report.json` path. Successful runs delete the test
databases but retain the report; `--keep` retains the databases too. Failures attempt
cleanup and record incomplete teardown. If SSH or the host dies abruptly, inspect the
printed run directory and restart a server against that directory to clean up retained
databases. No 1 TB pass has been recorded until this command completes on suitable hardware.

To resume an interrupted **source-generation** phase, stop the old server/processes,
then use the same target and run directory with `--resume` and `--keep`:

```sh
python3 scripts/benchmark.py --host user@server --home /srv/snapshotdb-bench \
  --binary /usr/local/bin/snapshotdb --engine-bin /usr/lib/postgresql/16/bin \
  --target-gb 1000 --keep --resume /srv/snapshotdb-bench/scale-EXISTING
```

Resume verifies the saved target, source marker, and contiguous committed row IDs.
It continues from PostgreSQL's committed data even if the progress log lagged the last
transaction. It refuses runs that already contain a replica or branch, and preserves
existing data on failure. The new report identifies a resumed run; generation timings
cover the resumed attempt, not the combined time across machines. Prior reports are
archived beside it.

## Public data or dummy data

[Criteo's 1 TB Click Logs](https://ailab.criteo.com/download-criteo-1tb-click-logs-dataset/)
is a public dataset. Its official [Hugging Face repository](https://huggingface.co/datasets/criteo/CriteoClickLogs)
lists a CC-BY-NC-SA-4.0 license. These are downloadable records, not a running PostgreSQL
database with replication credentials; they need importing into a supported engine first.
File encoding and compression also mean download size does not establish database size.

For testing SnapshotDB, the built-in synthetic PostgreSQL generator avoids that download
and import step. It measures actual table/index bytes and uses the same replication and
branch operations as a real source. A source hosted elsewhere would still require space
for a full replica on the SnapshotDB server. Copy-on-write saves space on subsequent branches.
