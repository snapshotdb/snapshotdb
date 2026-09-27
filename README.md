![SnapshotDB — real data, independent branches. PostgreSQL, MySQL, MongoDB and SQLite.](docs/assets/readme-hero.svg)

<p align="center">
  <a href="https://snapshotdb.io"><b>Website</b></a> &nbsp; / &nbsp;
  <a href="https://snapshotdb.io/docs"><b>Documentation</b></a> &nbsp; / &nbsp;
  <a href="docs/server.md"><b>Deploy a server</b></a> &nbsp; / &nbsp;
  <a href="LICENSE"><b>Apache-2.0</b></a>
</p>

A writable database branch for every developer, agent, and test. SnapshotDB uses
copy-on-write storage so branches share unchanged data. Self-host it on your infrastructure;
database files and processes stay on the server.

### See it in 27 seconds

https://github.com/user-attachments/assets/00146060-9dd5-426f-9f81-8087fd3080ea

## One replica. Room to experiment.

![A production source replicates to your SnapshotDB server; Claude Code, Codex and OpenCode each work on a separate branch.](docs/assets/readme-branching.svg)

| Test migrations | Give agents real data | Isolate CI jobs |
|---|---|---|
| Change a schema on a branch. Reset ordinary branches when you need a fresh start. | Hand Claude Code, Codex, or OpenCode a separate URL and credentials. | Create a branch per job and remove it when the run finishes. |

PostgreSQL, MySQL, and MongoDB use a server-side replica. SQLite starts from a file or an
empty database. Branch URLs grant access to copied data: keep them private, apply masking
when needed, and enforce network isolation separately.

## Quick start: your own server

Install the CLI as described below and [deploy a server](docs/server.md) first. The
server needs storage for the full initial replica, its WAL/logs, and subsequent writes.

```sh
export SNAPSHOTDB_SERVER=https://snapshotdb.example.com
export SNAPSHOTDB_TOKEN='<your server access token>'

snapshotdb preflight postgres 'postgresql://user:pass@db.example.com:5432/app'
#   ✓ connection (PostgreSQL 16.4)  ✓ is writer  ✓ wal level (actual: logical)  ...
#   ✓ Preflight passed.

snapshotdb clone prod 'postgresql://user:pass@db.example.com:5432/app'
# postgresql://you:<branch-password>@branches.internal:57340/app
# prod replicates 41 tables from the source; initial copy continues in the background

snapshotdb status prod
# Wait until the initial copy is complete and replication is healthy before branching.

snapshotdb create feature-x --from prod --print-url
# postgresql://snapshotdb_agent:<branch-password>@branches.internal:57375/app
```

`prod` is a replica on the deployed server kept in sync with production, rows and schema changes alike, by the
engine's own replication. Every `create` is a copy-on-write clone of it with its own server on
its own port: real data and writable storage, with a separate restricted agent login. The first
replica requires a full transfer and enough server disk for the data. Subsequent branches
share filesystem blocks until pages change; metadata, startup, and writes still cost space and time. Idle
branches suspend after five minutes and resume on the next connection; the URL never changes.

Connect your application or database client to the returned branch URL over your private
network or encrypted tunnel. When finished, remove it with `snapshotdb rm feature-x`.

## Prepared branches for agents

After replication is ready, prepare capacity before handing work to agents:

```sh
snapshotdb prepare release-1 --from prod --count 3
snapshotdb create claude-code --from release-1 --print-url
snapshotdb create codex --from release-1 --print-url
snapshotdb create opencode --from release-1 --print-url
```

Each claim receives a separate, already-running database. Preparation takes time and
consumes resources; an exhausted pool returns an error. Refill with `prepare` using the
same snapshot name, or use a new name to capture newer source data. See
[prepared branches](docs/prepared-branches.md) for freshness, lifecycle, and SQLite usage.
Prepared children cannot be reset; claim a replacement before removing the old branch.

### Measured on a 1 TB PostgreSQL database

<table width="100%">
  <tr>
    <td align="center" valign="top" width="33%"><h3>64–106 ms</h3><b>Prepared claims</b><br>4 concurrent agents<br>Server-local range</td>
    <td align="center" valign="top" width="33%"><h3>997 ms</h3><b>Fresh / server</b><br>No prepared pool<br>Median of 20 trials</td>
    <td align="center" valign="top" width="33%"><h3>1,413 ms</h3><b>Fresh / laptop</b><br>No prepared pool<br>Median of 20 trials</td>
  </tr>
</table>

Timers include allocation, connection, read, committed write, and read-back. Prepared
claims exclude **4.913 seconds of preparation**. These are recorded measurements from
specific fixtures and hardware, not a latency guarantee. [Conditions and raw evidence](docs/latency-results.md).

## Install

Build from this checkout with Rust/Cargo:

```sh
git clone https://github.com/snapshotdb/snapshotdb.git
cd snapshotdb
cargo install --path . --locked
snapshotdb --version
```

The website also provides a checksum-verified installer for macOS (arm64 and x86_64)
and Linux (x86_64):

```sh
curl -fsSL https://snapshotdb.io/install.sh | sh
snapshotdb --version
```

Build from source on other supported architectures. A website deployment publishes the
tracked bundle in [`site/public/dl`](site/public/dl); its [`BUILD.json`](site/public/dl/BUILD.json)
and checksums identify the source revision. The served bundle can lag behind this checkout.
Operators should follow the [release verification procedure](docs/hosting/release-cli.md).

Deploy the server before using database commands: see [server deployment](docs/server.md).
For a customer-owned AWS deployment, see the [BYOC appliance and deployment template](deploy/byoc/README.md).
The [strict comparison protocol](docs/strict-comparison.md) separates measured branch
latency from unverified infrastructure and product parity.
Use a matching v0.4.0 client/server build; older v0.3.0 release binaries use local storage.
Without `SNAPSHOTDB_SERVER`, the client fails; it never falls back to a local database copy.
The client only needs the SnapshotDB binary. Engine binaries and copy-on-write storage belong
on the server. PostgreSQL, MySQL and MongoDB agent branches require Linux with bubblewrap;
startup fails if the sandbox is unavailable. SQLite remains available on other platforms.
`SNAPSHOTDB_HOME` controls **server** storage, not client storage.

Requests become authenticated server jobs. The CLI waits and prints the result; `--detach`
returns a job ID immediately, and `snapshotdb job <id>` reconnects to it. A lost client
connection does not cancel the server operation. PostgreSQL's initial table copy continues
after the clone job returns; use `status` to monitor it.

## Self-hosted and hosted console

| Mode | Where databases live | Access |
|---|---|---|
| Self-hosted / BYOC | Your server or cloud account | CLI with server URL and admin token; optional console in BYOC mode |
| SnapshotDB Cloud | Hosted infrastructure | GitHub sign-in through the console, with workspace quotas and Dodo billing |

GitHub CLI sign-in does **not** connect database commands to a Cloud workspace. The CLI
currently requires your own server URL and token. BYOC users operate their own backups,
network controls, and capacity; Cloud source data is copied to the hosted server.

For hosted operators, start with [deployment](docs/hosting/deployment.md),
[plan accounting](docs/hosting/pricing.md), and the [launch acceptance checklist](docs/hosting/launch-checklist.md).
Passing repository CI does not establish production isolation, payment processing, or recovery.

<details>
<summary><b>Full command reference</b></summary>

## Commands

```
snapshotdb clone <name> <connection-string>                  infer the engine and create a server-side replica
snapshotdb job <id>                                         wait for an existing server job
snapshotdb preflight <postgres|mysql|mongodb> <url> [--schemas a,b] [--format json]   check a source; creates nothing (exit 2 on failure)
snapshotdb import <postgres|mysql|sqlite|mongodb> <name> <datadir|file|--new>
snapshotdb sync   <postgres|mysql|mongodb> <name> <url> [--schemas a,b] [--fix-replica-identity]   root kept in sync with production
snapshotdb create <name> --from <parent> [--print-url] [--format json]
snapshotdb prepare <snapshot> --from <parent> --count <1-32>   freeze a snapshot and fill its ready branch pool
snapshotdb info   [name] [--print-url] [--format json]      details of a branch (default: current)
snapshotdb url    [name]
snapshotdb switch <name>                                    make a branch current
snapshotdb list   [--format json]
snapshotdb status <name> [--format json]                    replication state of a synced root
snapshotdb repair <name>                                    resume a paused replica (skip a poisoned transaction, or reconcile schema)
snapshotdb reconcile <name>                                 add columns/tables the source gained (for sources without the event trigger)
snapshotdb reset  <name>                                    re-clone from parent
snapshotdb settings <root> [set <key> <value> | remove <key>]   keys: default_db, branch_sql (@file or SQL), source
snapshotdb lock|unlock <name>                               protect a branch from rm
snapshotdb start|stop|rm <name>
```

`import` makes a root from a stopped data directory, a SQLite file, or `--new` on the server.
File paths, including `@file` SQL hooks, refer to the server filesystem. `sync` makes a
root that replicates from a live database. Both are branched the same way. `create` and `sync`
make the new branch current, so `snapshotdb info --print-url` needs no name.

</details>

## Engines

| Engine | Replica / root | Branch connection |
|---|---|---|
| PostgreSQL | Logical replication; tracked DDL with documented limits | PostgreSQL protocol |
| MySQL | GTID replication, including native DDL | MySQL protocol |
| MongoDB | `mongodump` plus change streams | MongoDB protocol |
| SQLite | Server-side `.sqlite` file or `--new`; no production replication | Authenticated SQL-over-HTTP |

All four support prepared pools. Agent branches require managed roots; importing an
unmanaged native database directory does not automatically make it safe to hand to an agent.

Binaries are found on `PATH`: `pg_ctl initdb psql pg_dump pg_dumpall`, `mysqld mysql mysqldump`,
`mongod mongosh` plus `mongodump mongorestore` for sync. Every mongod runs as a single-node
replica set, so transactions and change streams work on branches too.

The current server is verified by `./e2e.sh` on Linux (Btrfs) in CI,
including preflight, sync, schema changes, a poisoned transaction repaired, suspend and
resume, a simulated reboot, credentials, settings, and teardown.

<details>
<summary><b>Historical engine benchmarks and scale-test notes</b></summary>

## Historical engine benchmarks

Historical engine measurements from v0.3.0 on an M5 Pro (APFS), before the client/server
change, with datasets generated by pgbench, `INSERT ... SELECT` doubling, and `insertMany`
of random documents. These are not measurements of the new remote API or of a 1 TB database.

| | Postgres, 88 GB, 600 M rows | MySQL, 4.5 GB, 4.2 M rows | MongoDB, 9.8 GB, 5 M docs |
|---|---|---|---|
| Branch from a stopped parent | 0.34 s | 0.93 s | 1.60 s |
| Branch from a running parent | 0.60 s | 2.79 s | 2.21 s |
| Disk for two clones of the parent | none measurable | 168 MB | none measurable |
| Modify 4 M rows on a clone | 2.1 GB of new disk, parent untouched | | |
| Resume a suspended branch, first query | 0.15 s | | |
| `sync` initial copy | 75 GB in 59 min (single logical-replication stream) | 205 s | 136 s |
| Live change after the copy | replicated | replicated | replicated |
| DDL on the largest table | column added, then an index over 600 M rows replayed in 162 s with the stream connected | native | index and collection options replicated |
| Branch off the replica | 0.69 s | 1.95 s | 2.52 s |
| Teardown | 0 slots, 0 publications, 0 triggers left on the source | | |

Two bugs came out of these runs and are fixed: `status` used to refresh the publication
every time it ran, which stalled the apply worker during a long initial copy; and the
default 60-second replication timeouts dropped the link whenever a DDL replay on a big table
took longer than that. Both sides of the link now allow 30 minutes.

The [remote scale test](docs/server.md#scale-test) generates synthetic PostgreSQL data up to
1 TB, verifies its measured size, and checks replication, schema changes, and isolation.
The subsequent [1 TB run](docs/tb-benchmark.md) and [prepared/fresh branch measurements](docs/latency-results.md)
have their own recorded evidence. They do not establish the performance of a new deployment.

</details>

<details>
<summary><b>Source requirements, replication repair, and branch settings</b></summary>

## What `sync` needs from production

Run `preflight` first. It prints a pass/warn/fail checklist and, when grants are the problem,
a ready-to-run grant script. Nothing is created or stored.

**Postgres**: version 13+, `wal_level = logical` (RDS: `rds.logical_replication = 1`), a free
replication slot, and a user that can read the tables and owns them (a publication needs the
owner). Tables without a PRIMARY KEY are skipped with the `ALTER TABLE ... REPLICA IDENTITY
FULL` printed, because publishing them would make UPDATE/DELETE fail on production itself;
`--fix-replica-identity` applies those statements for you.

On the source snapshotdb creates, all named `snapshotdb_<name>`: a publication, a replication
slot, a schema holding a `ddl` log table, and an event trigger that writes each DDL statement
into that table. The table is replicated and a trigger on the replica replays it, so
migrations on production appear on the replica and new tables join replication on the next
`status` or `create`. The event trigger swallows its own errors, so it can never fail your
DDL; creating it needs superuser (RDS: `rds_superuser`; Supabase's `postgres` role can too).
Without it rows still replicate, `status` says schema changes are not tracked, and after a
migration `snapshotdb reconcile <name>` adds the columns and new tables the source gained.
A row arriving with a column the replica lacks pauses the stream, and `repair` runs the
reconcile and resumes. Tables the role cannot read are left out of the schema copy and the
publication. `rm` attempts to remove its source-side resources; if cleanup fails, follow
the printed manual-cleanup instructions so an orphaned slot does not retain WAL.

**MySQL**: 8.0+, `gtid_mode = ON`, `enforce_gtid_consistency = ON`, row-format binary logging,
and a user with `REPLICATION SLAVE` plus read access. User databases are dumped once with
`mysqldump --single-transaction`; system schemas are never mirrored. Nothing is created on the
source. DDL replicates natively.

**MongoDB**: 6.0+, a replica set (Atlas always is), and a user that can read all databases and
open a change stream. A resume token is taken, `mongodump | mongorestore` copies the data, then
a tailer applies every change as an upsert or delete by `_id`, so the overlap with the dump is
harmless. Nothing is created on the source.

## When replication breaks

A transaction the replica cannot apply (say, a row you inserted on the replica that production
later inserts too) pauses the stream instead of retrying forever. `status` shows the error and
`repair <name>` skips that one transaction and resumes. For Postgres this is
`ALTER SUBSCRIPTION ... SKIP`; for MySQL an empty commit under the failing GTID. MongoDB
records failed attempts in `run/tail.errors`, retains its checkpoint, and retries rather
than silently advancing past the failed event. Inspect the reported error before repair.
`status` also shows how much WAL
the slot is retaining on the source; a stopped Postgres root keeps retaining WAL until it is
started again or removed.

## Branches

Branches cut from a replica are detached on first start: the inherited subscription or replica
channel is removed, and Postgres sequences are moved past the replicated rows so inserts do
not collide. Branch writes do not replicate back to the source. Enforce network egress
restrictions separately when running untrusted workloads.

Per-root settings shape every new branch:

```sh
snapshotdb settings prod set default_db app                              # database name in branch URLs
snapshotdb settings prod set branch_sql @anonymize.sql --hook 10-anonymize  # run once on every new branch, never on the source
snapshotdb settings prod set branch_sql @fixtures.sql  --hook 20-fixtures   # several hooks run in name order
snapshotdb settings prod set source 'postgresql://...'                   # rotate credentials without re-syncing
snapshotdb lock prod                                                     # rm refuses until unlock
```

Re-running `create` with the same name and parent returns the existing branch and URL, so
scripts can retry safely. Exit codes follow the usual convention: 0 success, 1 something
failed while running, 2 the command was wrong or preflight did not pass; with `--format json`
errors are printed as `{"error": "..."}`. Client TLS for the source goes in the URL itself
(`?sslmode=verify-full&sslrootcert=...&sslcert=...&sslkey=...` for Postgres).

Idle branches stop their engine after the server's `SNAPSHOTDB_IDLE_MINUTES` (default 5, 0 disables) and
resume when a client connects; the proxy on the branch's port stays. Synced roots never
suspend. Starting the server after a reboot restores proxies and synced roots.

</details>

## Agents and CI

Give an agent a branch, not production. The [agent skill](skills/snapshotdb/SKILL.md)
describes the workflow and connection rules. For Claude Code, install it with:

```sh
mkdir -p .claude/skills/snapshotdb && curl -fsSL https://raw.githubusercontent.com/snapshotdb/snapshotdb/main/skills/snapshotdb/SKILL.md -o .claude/skills/snapshotdb/SKILL.md
```

In CI, a fresh database per job:

```sh
DATABASE_URL="$(snapshotdb create "pr-$PR" --from prod --print-url)" || exit 1
[ -n "$DATABASE_URL" ] || exit 1
export DATABASE_URL
# ... migrate and test ...
snapshotdb rm "pr-$PR"
```

<details>
<summary><b>Storage layout and copy-on-write internals</b></summary>

## How it works

```
<server SNAPSHOTDB_HOME>/<name>/
  engine       postgres | mysql | sqlite | mongodb
  parent       optional
  source       optional production URL (mode 0600); makes this a synced root
  default_db, branch_sql, lock     per-root settings
  data/        the engine's data directory; this is what gets cloned
  run/         public port (proxy), engine port, pids, socket, logs, tailer state; never cloned
```

Cloning is `cp -cpR` on macOS (clonefile(2) per file) and `cp -a --reflink=always` on Linux
(Btrfs, XFS with reflink, bcachefs). Branching a running parent holds it, stops it, clones,
and restarts it, because per-file clones are only a consistent snapshot while nothing is
writing. Engines bind `127.0.0.1` on the server. The database proxies bind the server's
`--db-bind` address, and returned URLs use `--public-host`. The control API uses a bearer
token; expose it through HTTPS or an SSH tunnel. See [deployment](docs/server.md).

</details>

## Build and test

```sh
cargo build --release   # target/release/snapshotdb
cargo test
python3 -m unittest discover -s scripts -p 'test_*.py'
./e2e.sh                # fake production -> preflight -> sync -> schema change -> repair -> branches -> teardown
```

On Linux, point `TMPDIR` at a reflink-capable volume when running the Rust tests. The
integration tests deliberately start a disposable server fixture; client storage remains
separate and is checked for absence. `e2e.sh` also starts a disposable server and sends all
commands through its API.

## Known limits

- PostgreSQL replays top-level DDL separately from streamed DML, including mixed batches,
  quoted strings and dollar-quoted function bodies. Procedural/dynamic DDL and `CREATE
  TABLE AS` require explicit reconciliation; recorded errors block branching. This is
  not a complete PostgreSQL grammar or full source-role/RLS fidelity guarantee.
  `CREATE INDEX CONCURRENTLY` becomes a plain index on the replica.
- The BYOC token is administrative for the whole deployment. Hosted workspaces add
  gateway-derived identity, plan accounting, and resource limits, but the filesystem
  sandbox shares host networking. Neither mode provides managed failover or a VM boundary.
  Shared hosting requires connection-time egress isolation, encrypted database transport,
  hard disk limits, and tested recovery; see [launch acceptance](docs/hosting/launch-checklist.md).
- SQLite returns a private SQL-over-HTTP URL. It uses the request format in
  [prepared agent branches](docs/prepared-branches.md); native SQLite file drivers cannot
  open that URL. PostgreSQL, MySQL, and MongoDB use their native network protocols.
- Jobs run in order within a workspace, with up to four workspaces executing concurrently.
  Admission limits and deadlines bound work; a timeout does not undo changes already made.
  Queued/running jobs are marked interrupted after a server restart and are not automatically
  retried. Inspect branch state before resubmitting. See [job limits](docs/server.md#client-use).
- Managed roots expose an administrative URL for the deployment operator. Child URLs use
  `snapshotdb_agent`: PostgreSQL object ownership without superuser/server-file roles,
  MySQL privileges on application databases, and MongoDB read/write and database-admin
  roles without user administration. Maintenance credentials stay in private server files.
  PostgreSQL branch ownership is reassigned for migrations; exact source-role/RLS fidelity
  is not guaranteed. Unmanaged imported directories cannot produce agent branches.
- Initialization hooks must succeed before a URL or proxy connection is usable. Prepared
  children inherit initialized snapshot data without executing the hooks twice. Recorded
  PostgreSQL DDL failures and unhealthy MySQL/MongoDB replication block fresh branches.
  MongoDB repair retries its failed event instead of advancing past it.

See [security fixes, upgrade instructions and measured latency](docs/security-hardening.md).
For synced PostgreSQL roots, new branches also wait for a replicated commit marker;
see [freshness guarantees and timeout configuration](docs/postgres-freshness.md).

## Contributing and support

Open a [bug report or feature request](https://github.com/snapshotdb/snapshotdb/issues)
with the build revision, engine version, and a minimal reproduction using synthetic data.
Never post tokens, connection strings, billing details, or database contents in public issues.
Security reports should use GitHub private vulnerability reporting when enabled; otherwise
contact a maintainer privately before sharing details.

The [website development guide](site/README.md) covers the console and documentation app.
See the [changelog](CHANGELOG.md) for changes and [Apache-2.0 license](LICENSE) for use terms.
