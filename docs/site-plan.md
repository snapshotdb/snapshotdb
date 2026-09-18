# snapshotdb docs site + web console build plan

Goal: reach parity with Ardent on the **developer-facing surface** — a real docs site and an
expanded web console — while staying honest about what snapshotdb is (one self-hosted server,
one admin token, no tenancy) and what it adds (multi-engine, branch-from-any-branch, prepared
snapshot pools). Every outline below is grounded in `README.md`, `CHANGELOG.md`, `src/main.rs`,
`src/remote.rs`, and `web/console.html`. Where we lack an Ardent feature, the outline says so.

Companion: `docs/ardent-parity.md` (the full parity matrix).

---

## Part A — Docs site

Proposed structure (mirrors Ardent's IA; Postgres-specific pages generalize to our four engines).
Legend for each page: **[real]** grounded in shipped behavior · **[gap]** documents something we
do not yet have (note honestly or omit until built).

### Getting started

**`quickstart.md`** [real]
- Three commands end to end: `export SNAPSHOTDB_SERVER` + `SNAPSHOTDB_TOKEN`, then `preflight` → `clone`/`sync` → `create --print-url`.
- Emphasize the client/server split: the CLI only submits jobs; database files and engines live on the deployed server.
- Show a Postgres example and note the same shape works for MySQL and MongoDB.
- Link to server deployment (below) as a prerequisite; the client fails without `SNAPSHOTDB_SERVER`.

**`install.md`** [real]
- Client install: `cargo install --path . --locked`; the client only needs the snapshotdb binary.
- Note the client/server change is unreleased; v0.3.0 release binaries still use local storage.
- Engine binaries required on the **server** `PATH` (`pg_ctl initdb psql pg_dump pg_dumpall`, `mysqld mysql mysqldump`, `mongod mongosh` + `mongodump mongorestore`).
- `SNAPSHOTDB_HOME` controls **server** storage, not client storage.

**`server-deployment.md`** [real] (fold in existing `docs/server.md`)
- `snapshotdb serve --bind <addr:port> --public-host <hostname> [--db-bind <ip>]`; set `SNAPSHOTDB_TOKEN` on the server.
- Storage lock (one server per `SNAPSHOTDB_HOME`), restart recovery (queued/running jobs marked interrupted, not auto-retried), the command queue.
- Public host / branch port model: each branch owns a stable public port via a proxy; `SNAPSHOTDB_IDLE_MINUTES` for suspend.
- Reverse-proxy/TLS termination guidance; scale-test runner reference (`docs/server.md#scale-test`).

### Connectors (sources)

**`connectors/overview.md`** [real]
- What a "root" is: created by `sync` (live source), `clone` (inferred-engine alias), or `import` (stopped datadir / file / `--new`).
- The preflight → sync loop; `--schemas` selection; `--fix-replica-identity`.
- Per-engine capability table (from README): roots, sync mechanism, schema-change handling, branch time.
- Honest note: no managed IP allowlist, no BYOC placement — you run the server, so its network is yours.

**`connectors/postgres-self-hosted.md`** [real]
- Requirements: PG 13+ (14+ for the event trigger niceties), `wal_level=logical`, `max_wal_senders`/`max_replication_slots`, a free slot, a replication role with read access.
- Run `preflight postgres <url> --schemas public`; apply the printed grant SQL; re-run until it passes.
- DDL replication via the event trigger (needs superuser to create; `rds_superuser`/Supabase `postgres` work); without it, `status` says schema changes aren't tracked and `reconcile` catches up.
- Teardown (`rm`) leaves 0 slots / 0 publications / 0 triggers on the source.

**`connectors/aws-rds.md`** [gap→real] (RDS is self-hosted Postgres/MySQL you point us at)
- Parameter group: set `rds.logical_replication=1` (Postgres) / row-based binlog + `rds_replication` (MySQL), then reboot.
- Use the **writer** endpoint (writable primary), not a read replica; WAL/binlog retention.
- Preflight from a host with network reach to the instance; grants via the printed script.
- Note: snapshotdb adds no RDS-specific automation beyond preflight; you apply the param group yourself.

**`connectors/supabase.md`** [gap→real]
- Use the **direct** connection string (not the pooler — the pooler can't do replication and preflight fails on it).
- IPv4 reachability; Supabase's `postgres` role can create the event trigger.
- Then `sync postgres <name> <direct-url> --schemas public`.
- Honest note: no `settings supabase link` local-linking yet (Ardent has it; see parity §6).

**`connectors/mysql.md`** [real] (covers PlanetScale-style MySQL)
- Requirements: MySQL 8.0+, `gtid_mode=ON`, `enforce_gtid_consistency=ON`, row-format binary logging, a user with `REPLICATION SLAVE` + read.
- Initial copy via `mysqldump --single-transaction`; system schemas never mirrored; nothing created on the source; DDL replicates natively.
- PlanetScale specifics: dashboard replication toggle; URL without explicit port (CLI default).

**`connectors/mongodb.md`** [real]
- Requirements: MongoDB 6.0+, a replica set (Atlas always is), a user that can read all DBs and open a change stream.
- Resume token → `mongodump | mongorestore` → tailer applies changes as upsert/delete by `_id`; overlap with the dump is harmless; nothing created on the source.
- Every branch mongod runs as a single-node replica set, so transactions/change streams work on branches too.

**`connectors/sqlite.md`** [real] (snapshotdb-only; no Ardent analog)
- `import sqlite <name> <file>` or `--new`; branches return a per-branch authenticated SQL-over-HTTP URL.
- Request format in `docs/prepared-branches.md`; native SQLite file drivers cannot open that URL.

### Branching & recovery

**`branches.md`** [real]
- `create <name> --from <parent>` = copy-on-write clone with its own server + port; real, writable, isolated, no production credentials.
- Detach-on-first-start (subscription/replica channel removed; Postgres sequences advanced); a branch never contacts production.
- Auto-suspend after 5 min idle, resume on next connection, stable URL; branch from any branch (git-like tree).
- `--print-url`, `--format json` (`schema_version`), exit codes 0/1/2, idempotent create, `--detach` + `job <id>`.

**`prepared-branches.md`** [real] (fold in existing `docs/prepared-branches.md`)
- `prepare <snapshot> --from <parent> --count <1-32>` freezes an immutable snapshot and prestarts a ready pool.
- `create --from <snapshot>` claims a ready, already-running branch (sub-second first read/write); snapshots are immutable.
- Latency evidence (`docs/latency-results.md`); the SQLite `/v1/query?token=` request format.

**`replication-and-repair.md`** [real]
- Why a stream pauses (a transaction the replica can't apply — e.g. a row you inserted that production later inserts).
- `status` shows the error + WAL retained on the source; `repair` skips the one poison transaction and resumes (Postgres `ALTER SUBSCRIPTION … SKIP`, MySQL empty commit, Mongo logs to `run/tail.errors`).
- `reconcile` adds columns/tables the source gained (for sources without the event trigger); `reset` re-clones from parent.
- Migration caveat: single query strings mixing DDL+DML replay DML on the replica and can collide — send statement by statement, or `rm` + `sync`.

### Concepts

**`architecture.md`** [real]
- Client/server split: thin CLI submits authenticated jobs; only a deployed `serve` process runs engines and stores files.
- Copy-on-write branching (`cp -c`/`--reflink`), the per-branch proxy owning a stable public port, suspend/resume.
- Native replication per engine (Postgres logical, MySQL GTID, MongoDB change streams) — no proprietary/Kafka pipeline.
- Contrast with Ardent's split control-plane/data-plane + BYOC: here the whole data plane is the server you run.

**`security.md`** [real] (consolidate README credential/limits + `docs/server.md`)
- Per-branch credentials: generated admin password per root, rotated per branch on first start; Postgres trusts only its Unix socket; Mongo keyFile + `root` user.
- Source footprint: slot + publication + one metadata schema/event trigger (Postgres); nothing created for MySQL/Mongo; `rm` cleans up.
- Single admin token, no tenant isolation, no per-user authz — state this plainly.
- No telemetry (contrast Ardent's always-on PostHog). Branch URLs carry credentials but can't reach the source.

**`faq.md`** [real]
- Which databases (Postgres, MySQL, MongoDB, SQLite — broader than Ardent's Postgres-only).
- Branch speed (sub-second to ~2.8s by engine/state; prepared pools sub-second); do branches stay in sync (no — point-in-time from a synced replica, then isolated).
- Idle branches (suspend after 5 min, resume instantly); safe to hand branch URLs to agents (isolated creds, no source access).
- Can I self-host / what are the limits (one server, one token, no tenancy).

### CLI & API reference

**`cli-reference.md`** [real]
- Every command from `USAGE`: `serve`, `clone`, `job`, `preflight`, `import`, `sync`, `create`, `prepare`, `info`, `url`, `switch`, `list`, `status`, `repair`, `reconcile`, `reset`, `settings`, `lock`/`unlock`, `start`/`stop`/`rm`.
- Global behaviors: `--print-url`, `--format json`, `--detach`, `--schemas`, `--fix-replica-identity`, `--hook`.
- Note the gaps vs Ardent honestly: no `--url-type direct|pooled|prisma`, no `login/logout/status` (token via env), no `project`/`org`/`invite`.

**`configuration.md`** [real]
- Client env: `SNAPSHOTDB_SERVER`, `SNAPSHOTDB_TOKEN`. Server env: `SNAPSHOTDB_HOME`, `SNAPSHOTDB_TOKEN`, `SNAPSHOTDB_IDLE_MINUTES`, `SNAPSHOTDB_DB_BIND`.
- Exit codes: 0 success, 1 failed while running, 2 wrong command / preflight failed; `--format json` errors as `{"error":"…"}`.
- `settings <root>` keys: `default_db`, `branch_sql` (`@file` or SQL, multiple via `--hook`, run in name order), `source` (credential rotation).

**`api-reference.md`** [real]
- Bearer `SNAPSHOTDB_TOKEN`. `GET /v1/health` → `{version}`; `POST /v1/commands` (CLI-verb array) → `202 {id,state}`; `GET /v1/jobs/{id}` → job state/exit/stderr.
- The job model = Ardent's operations; show a create-then-poll example.
- Per-branch SQLite `/v1/query?token=<branch-secret>` request format.
- Honest gaps: no REST-native resource routes yet, no per-key API-key management, `?wait` long-poll not implemented (see parity §12, gaps #4/#7).

### Workflows

**`workflows/ai-agents.md`** [real] (link `skills/snapshotdb/SKILL.md`)
- Hand an agent only a branch URL: `AGENT_DATABASE_URL="$(snapshotdb create agent-task --from prod --print-url)"`; abort if empty.
- Prepared pools for agent bursts: `prepare` capacity ahead of time, then each agent claims a ready branch via `create --from <snapshot>`.
- Isolation guarantee: a leaked branch URL can't reach production; agents must not commit URLs (they carry credentials).

**`workflows/local-dev.md`** [real]
- Start a feature: `snapshotdb create my-feature --from prod`; work freely; `snapshotdb url` to connect; `snapshotdb rm` when done.
- Multiple features via `list` + `switch`; branches auto-suspend so laptop cleanup isn't urgent.
- `branch_sql` hooks to seed/anonymize each new branch (`settings prod set branch_sql @seed.sql --hook 10-seed`).

**`workflows/ci-cd.md`** [gap→real]
- A GitHub Actions recipe: set `SNAPSHOTDB_SERVER`/`SNAPSHOTDB_TOKEN` secrets, `create pr-${{ github.event.number }} --print-url` → `DATABASE_URL`, run tests, `rm` with `if: always()`.
- Treat an empty `--print-url` as a hard failure; use `--detach` + `job` for long clones.
- Gap #9: ship this as a reusable composite Action (we only have an internal e2e workflow today).

**`workflows/regression-testing.md`** [real]
- Branch from the synced replica, export the URL, run the suite (`npm test` / `pytest` reading `DATABASE_URL`), tear down.
- Unlocks destructive/migration testing (`ALTER`, `DROP`, `DELETE`, `TRUNCATE`) against real data with production untouched.

### Reference / comparison (keep existing)
- `ardent-comparison.md` (latency benchmarks — already exists), `latency-results.md`, `benchmarks/`, `aws-mumbai.md`, `tb-benchmark.md`, `prepared-branches.md`. Cross-link from the docs nav.

**Docs pages we deliberately will NOT create** (Ardent-only, ⚪ N/A): `cli/auth`, `cli/projects`,
`cli/org`, org/team/invites, environments/BYOC, billing — no accounts, projects, teams, or
managed cloud in a single self-hosted server.

---

## Part B — Web console views

Today `web/console.html` is a single-page app: connect (base URL + token) → health → a "Sources"
list → new-source (sync) form → new-branch (create) → per-source status/copy-URL/delete →
settings (default database). It drives the server through `POST /v1/commands` + `GET /v1/jobs/{id}`.
Below, each Ardent-style view is mapped; **[real]** = backed by a shipped capability,
**[N/A]** = doesn't apply to one self-hosted server.

| Console view | Maps to snapshotdb capability | Build status | Notes |
|---|---|---|---|
| **Connection / server config** (base URL + token) | Bearer token to `serve` | ✅ exists | Keep; add a health/version + auth-valid indicator (parity gap #8). |
| **Sources list** (roots: synced replicas + local roots) | `list` | ✅ exists | Keep; add engine badge (pg/mysql/mongo/sqlite) and root vs branch grouping. |
| **Source detail** (replication health, WAL lag, errors, schema-tracking) | `status <name>` | 🟡 build | New view: render `status` — replication state, WAL retained on source, worker error, whether the event trigger tracks schema changes. Parity gap #2. |
| **New source wizard** (preflight → sync → schema select → replica-identity) | `preflight`, `sync --schemas --fix-replica-identity` | 🟡 build | Current form is one-shot sync. Add an in-UI preflight step (show the pass/warn/fail checklist + grant SQL) and per-table replica-identity choices. Gaps #5, #11. |
| **Import source** (datadir / file / `--new`) | `import <engine> <name> <datadir\|file\|--new>` | ❌ build | No UI today; server-side paths. Useful for SQLite/`--new`. |
| **Branches list / tree** | `list` (branches under a root) | 🟡 build | Dedicated branch list with parent → child tree, state (running/suspended/snapshot), age/idle. Gap #2. |
| **Branch detail** (URL, copy, status, start/stop, reset, lock) | `info`/`url`, `start`/`stop`, `reset`, `lock`/`unlock`, `rm` | 🟡 build | Per-branch page: copy URL, suspend/resume state, `reset` (re-clone), lock/unlock, delete. |
| **Create branch** (from a root or another branch; `--print-url`) | `create --from <parent>` | ✅ exists | Extend to pick any parent (root or branch) and show the returned URL + JSON. |
| **Prepared snapshots & pools** (prepare, pool depth, claim) | `prepare <snapshot> --from --count`; claim via `create --from <snapshot>` | ❌ build | snapshotdb-only, high-value for agents: show snapshots, ready-pool depth, refill button. No Ardent analog. |
| **Quarantine / paused replication** panel | `status` (paused) + `repair` | ❌ build | Surface a paused stream and a one-click `repair`/`reconcile`/`reset`. Gap #6. |
| **Settings editor** (`default_db`, `branch_sql` hooks, `source` rotation) | `settings <root> set/remove` | 🟡 build | Today only `default_db`. Add a `branch_sql` hooks editor (ordered by hook name) and source-URL rotation. |
| **Jobs / operations** view (queue, progress, reconnect) | `POST /v1/commands`, `GET /v1/jobs/{id}`, `--detach` | 🟡 build | Show queued/running/done jobs with exit code + stderr; reconnect to a detached job. Gap #12. |
| **Server tokens / API keys** | single `SNAPSHOTDB_TOKEN` | 🟡 build | Only viable once multi-token support lands (gap #4). Until then, a read-only "current token" display. |
| Projects · Org/team/members/invites/roles · Environments/BYOC · Billing | — | ⚪ N/A | No tenancy, no managed cloud, no billing in a single self-hosted server. Do not build. |

### Console build order (mirrors the parity gap ranking)
1. **Source detail + status view** (gap #2) — the most-requested missing surface.
2. **Branch list/tree + branch detail** (gap #2) — makes the console usable day to day.
3. **In-UI preflight step in the new-source wizard** (gaps #5, #11) — de-risks setup.
4. **Jobs/operations view** (gap #12) — visibility into async work.
5. **Prepared snapshots & pools panel** — showcase the snapshotdb-only capability for agents.
6. **Quarantine/repair panel** (gap #6) and **settings/branch_sql editor**.
7. **Server-token management** (gap #4) once the backend supports named tokens.
