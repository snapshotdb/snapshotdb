# Changelog

## 0.4.0 (unreleased)

This section describes source changes and website CLI bundles. It does not assert that
the hosted service has been deployed or passed production launch acceptance.

- Hosted GitHub workspaces, Free/Pro quota accounting, Linux cgroup limits, and Dodo
  checkout/webhooks with a customer portal for billing recovery.
- Persist delivered transfer bytes, preserve webhook millisecond ordering, and correlate
  checkout completion with its reservation. Stale activation cannot override revocation
  at the same timestamp.
- Up to four concurrent workspace jobs with per-workspace FIFO, bounded admission,
  execution deadlines, and worker process-group cleanup on timeout or shutdown.
- Hosted source validation rejects unchecked SRV discovery and special-use destinations.
  Connection-time network isolation remains a deployment requirement.
- Mobile dialogs keep actions reachable and restore background scrolling when closed.
- Website CLI bundles include all three published platforms, SHA-256 checksums, and a
  source revision manifest; CLI version and authenticated health expose the build revision.
- README launch video, corrected agent-skill installation path, and updated deployment,
  prepared-branch, and website development documentation.

- Explicit immutable snapshots and ready branch pools via `prepare`; agent claims return
  isolated, already-running databases. All four engines passed sub-second allocation plus
  first committed read/write, including a laptop-to-Mumbai test. See `docs/latency-results.md`.
- SQLite now returns a per-branch authenticated SQL-over-HTTP URL with atomic request
  transactions and isolation from other server files, replacing the server-local file URL.
- Fast job polling removes the previous fixed one-second delay. MySQL URL database names
  no longer break credential setup before the initial restore.
- Client/server architecture: database commands require `SNAPSHOTDB_SERVER` and an access
  token. Only an explicitly deployed `serve` process runs engines and stores database files.
- `clone <name> <connection-string>` creates a remote replica; `create --from` makes isolated
  copy-on-write branches on the server. Returned network URLs advertise its configured host.
- Branch creation refuses PostgreSQL replicas whose initial table copy is incomplete or
  whose replication is paused.
- MongoDB branch URLs allow 30 seconds for server selection so cold resume works on small
  servers; reset tests verify successful execution, discarded branch writes, and fresh data.
- Authenticated asynchronous jobs, detached submission, reconnect by job ID, bounded command
  queue, exclusive server storage lock, and restart recovery without automatic replay.
- Deployment instructions and a synthetic PostgreSQL scale-test runner with explicit SSH
  or local-server execution, Linux reflink and macOS APFS support, small smoke-test sizes,
  measured-size checks, replication checks, branch isolation, and cleanup. The retained
  1 TB PostgreSQL run passed; fresh laptop branches completed reads and committed writes
  in 1.413 seconds median and 1.830 seconds maximum across 20 trials.
- Scale-test source generation can resume from committed rows with `--resume --keep`,
  preserving existing data across host upgrades and refusing later-stage runs.

- A post-benchmark service verifies retained databases across restart, temporary branch
  reset/isolation, and full engine regression suites, recording failures separately.

## 0.3.0

- `reconcile <name>`: for Postgres sources where the event trigger cannot be created, add
  the columns the source gained to the replica and put new keyed tables into the publication
  and onto the replica. `repair` runs it automatically when a row arrives with a column the
  replica lacks. Verified with a non-superuser role in an `rds_superuser` group.
- Restricted roles: preflight requires read and ownership only for tables that will be
  published; the schema copy leaves out tables the role cannot read.

## 0.2.0

- `preflight <engine> <url>`: Ardent-style checklist (connection, version, writer, wal_level,
  slots, senders, replication privilege, table read and ownership, event trigger capability,
  replica identity, slot WAL limit, duplicate source) with fixes and a grant script; exit 2 on
  failure; `--format json`. `sync` runs it first; `--fix-replica-identity` applies
  `REPLICA IDENTITY FULL` on the source.
- Stable public port per branch owned by a proxy in the same binary; idle engines suspend
  after `SNAPSHOTDB_IDLE_MINUTES` (default 5) and resume on the next connection. Synced roots
  never suspend. `up` restores everything after a reboot; `service install` runs it at login.
- Replication safety: subscriptions use `disable_on_error`; `status` shows the poisoned
  transaction, WAL retained on the source, and initial-copy progress; `repair` skips the
  transaction (Postgres `ALTER SUBSCRIPTION ... SKIP`, MySQL empty GTID commit, MongoDB tailer
  restart). The Mongo tailer logs failed events and continues.
- Schema changes: Postgres DDL replay honours the source `search_path`, strips `CONCURRENTLY`,
  and never logs replication plumbing. MongoDB replicates index creation and drops, collection
  creation with options, and `collMod`.
- Automation: `--print-url`, `--format json` with `schema_version`, JSON-wrapped errors, exit
  codes 0/1/2, idempotent `create`, current branch (`switch`, `info`).
- Per-root settings: `default_db` (taken from the source URL), ordered `branch_sql` hooks run
  once per new branch, `source` rotation; `lock`/`unlock`.
- Agent skill (`skills/snapshotdb/SKILL.md`), GitHub Actions e2e on a Btrfs loop mount with
  Postgres, MySQL, and MongoDB, release binaries for macOS and Linux.
- Per-branch credentials: roots snapshotdb creates get a generated admin password and every
  clone rotates to its own; Postgres trusts only its Unix socket, MongoDB runs with a keyFile.
- Found at scale (88 GB Postgres, 4.5 GB MySQL, 9.8 GB MongoDB) and fixed: no publication
  refresh during the initial copy; 30-minute replication timeouts on both sides so a long DDL
  replay does not drop the link; `repair` re-enables without skipping when a pause was not
  tied to a transaction; `status` reports the replication worker's error, not snapshotdb's own.

## 0.1.0

- Copy-on-write branches for Postgres, MySQL, SQLite, and MongoDB via `cp -c` / `--reflink`.
- `sync` roots replicating from production: Postgres logical replication with event-trigger
  DDL replay, MySQL GTID replication, MongoDB change streams. Clones detach on first start and
  get their sequences advanced.
