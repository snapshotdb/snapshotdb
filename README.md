# anybranch

Branch your production database like code. One binary, no dependencies, any engine.

```sh
anybranch sync postgres prod 'postgresql://user:pass@db.example.com:5432/app'
# postgresql://you@127.0.0.1:57340/postgres
# prod replicates 41 tables from the source; initial copy continues in the background

anybranch create feature-x --from prod
# postgresql://you@127.0.0.1:57375/postgres
# branch feature-x ready in 0.52s
```

`prod` is a local replica kept in sync with production, rows and schema changes alike, by
the engine's own replication. Every `create` is a copy-on-write clone of it with its own
server on its own port: real data, writable, isolated, and holding no production
credentials. A 50 GiB branch costs the same as a 50 MiB one because nothing is copied until
a page changes.

This is the open-source shape of what hosted products such as Ardent sell: a replica of
production plus instant branches off it. The differences: it runs on your machine, uses
Postgres logical replication, MySQL GTID replication, and MongoDB change streams instead of
a proprietary pipeline, and is not limited to Postgres.

## Commands

```
anybranch import <postgres|mysql|sqlite|mongodb> <name> <datadir|file|--new>
anybranch sync   <postgres|mysql|mongodb> <name> <production-url> [schema,schema]   root kept in sync with production
anybranch create <name> --from <parent>
anybranch reset  <name>          re-clone from parent
anybranch status <name>          replication state of a synced root
anybranch list
anybranch url|start|stop|rm <name>
```

`import` makes a root from a stopped data directory, a SQLite file, or `--new`.
`sync` makes a root that replicates from a live database. Both are branched the same way.

## Engines

| Engine   | roots                     | sync from production                    | schema changes | branch time (M5 Pro, APFS) |
|----------|---------------------------|-----------------------------------------|----------------|----------------------------|
| postgres | `--new`, stopped data dir | logical replication                     | event trigger replay | 0.2 s stopped parent, 0.5 s running |
| mysql    | `--new`, stopped datadir  | GTID replication                        | native (binlog) | 0.4 s, 1.7 s |
| mongodb  | `--new`, stopped dbpath   | `mongodump` + change streams            | n/a            | 0.3 s, 0.9 s |
| sqlite   | `--new`, a `.sqlite` file | not applicable                          | n/a            | milliseconds, no process |

Binaries are found on `PATH`: `pg_ctl initdb psql pg_dump pg_dumpall`, `mysqld mysql mysqldump`,
`mongod mongosh` plus `mongodump mongorestore` for sync. Every mongod runs as a single-node
replica set, so transactions and change streams work on branches too.

Verified with `./e2e.sh` on macOS (APFS) for all four engines, and on Linux (Btrfs) for Postgres.

## What `sync` needs from production

**Postgres**: `wal_level = logical` (RDS: `rds.logical_replication = 1`), a user that can read
the schema and own the tables, and a PRIMARY KEY on each table. Tables without one are skipped
and the `ALTER TABLE ... REPLICA IDENTITY FULL` to include them is printed, because publishing
such a table would make UPDATE/DELETE fail on production itself.

On the source anybranch creates, all named `anybranch_<name>`: a publication, a replication
slot, a schema holding a `ddl` log table, and an event trigger that writes each DDL statement
into that table. The table is replicated and a trigger on the replica replays it, so
migrations on production appear on the replica and new tables join replication on the next
`status` or `create`. The event trigger swallows its own errors, so it can never fail your
DDL; creating it needs superuser, and without it rows still replicate and `status` says
schema changes are not tracked. `rm` removes everything it created.

**MySQL**: `gtid_mode = ON`, `enforce_gtid_consistency = ON`, binary logging, and a user with
`REPLICATION SLAVE` plus read access. User databases are dumped once with
`mysqldump --single-transaction`; system schemas are never mirrored. Nothing is created on
the source. DDL replicates natively.

**MongoDB**: a replica set (Atlas always is) and a user that can read all databases and open
a change stream. A resume token is taken, `mongodump | mongorestore` copies the data, then a
tailer applies every change as an upsert or delete by `_id`, so the overlap with the dump is
harmless. Nothing is created on the source.

Branches cut from a replica are detached on first start: the inherited subscription or
replica channel is removed, and Postgres sequences are moved past the replicated rows so
inserts do not collide. A branch never contacts production.

## How it works

```
~/.anybranch/<name>/
  engine   postgres | mysql | sqlite | mongodb
  parent   optional
  source   optional production URL (mode 0600); makes this a synced root
  data/    the engine's data directory; this is what gets cloned
  run/     port, pid, socket, log, tailer state; never cloned
```

Cloning is `cp -cpR` on macOS (clonefile(2) per file) and `cp -a --reflink=always` on Linux
(Btrfs, XFS with reflink, bcachefs). Branching a running parent stops it, clones, and restarts
it, because per-file clones are only a consistent snapshot while nothing is writing. Servers
bind `127.0.0.1` only. Override the location with `ANYBRANCH_HOME`.

## Build and test

```sh
cargo build --release   # target/release/anybranch
cargo test              # SQLite end-to-end and URL parsing
./e2e.sh                # fake production -> synced replica -> schema change -> detached branch -> teardown
```

## Known limits

- Postgres DDL replay uses the whole client query string. Migration tools send one statement
  per query, which is exact; a hand-written `psql -c "ddl; insert ..."` batch replays its DML
  too. Statements that cannot run in a transaction (`CREATE INDEX CONCURRENTLY`) are recorded
  as failed in `status` and skipped.
- No daemon: no auto-suspend of idle branches and no proxy. Locally an idle server costs
  nothing; `anybranch stop` is the one-liner.
- Auth is whatever the source had. `--new` Postgres trusts local connections; `--new` MySQL
  has `root` with no password; `--new` MongoDB has no auth.

Apache-2.0.
