# Changelog

## 0.2.0

- `preflight <engine> <url>`: Ardent-style checklist (connection, version, writer, wal_level,
  slots, senders, replication privilege, table read and ownership, event trigger capability,
  replica identity, slot WAL limit, duplicate source) with fixes and a grant script; exit 2 on
  failure; `--format json`. `sync` runs it first; `--fix-replica-identity` applies
  `REPLICA IDENTITY FULL` on the source.
- Stable public port per branch owned by a proxy in the same binary; idle engines suspend
  after `ANYBRANCH_IDLE_MINUTES` (default 5) and resume on the next connection. Synced roots
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
- Agent skill (`skills/anybranch/SKILL.md`), GitHub Actions e2e on a Btrfs loop mount with
  Postgres, MySQL, and MongoDB, release binaries for macOS and Linux.

## 0.1.0

- Copy-on-write branches for Postgres, MySQL, SQLite, and MongoDB via `cp -c` / `--reflink`.
- `sync` roots replicating from production: Postgres logical replication with event-trigger
  DDL replay, MySQL GTID replication, MongoDB change streams. Clones detach on first start and
  get their sequences advanced.
