# Ardent → snapshotdb feature parity

One-to-one parity between **Ardent** (hosted, multi-tenant Postgres-only branching SaaS) and
**snapshotdb** (open-source, self-hosted, single-server, multi-engine branching tool).
Sourced entirely from local disk: Ardent inventory + docs + `openapi.json` (v1), and our
`README.md`, `CHANGELOG.md`, `src/main.rs`, `src/remote.rs`, `web/console.html`.

Status legend: **✅ have** · **🟡 partial** · **❌ missing** · **⚪ N/A-by-design**
(a deliberate consequence of being one self-hosted server with one admin token and no tenancy).

## Summary

Approximate tally across the matrices below (CLI 39, REST 35, plus feature/page/docs rows):

| Status | Count (approx.) | What it means here |
|---|---:|---|
| ✅ have | ~34 | Core branching surface: preflight, replica/sync, CoW branches, URLs, suspend/resume, settings, lock, status. |
| 🟡 partial | ~30 | Present but shaped differently (generic `/v1/commands` RPC vs REST resources; `--fix-replica-identity` vs per-table decisions; `repair`/`reset` vs `retry-setup`/`quarantine`). |
| ❌ missing | ~10 | Real developer-facing gaps: a docs site, URL variants (pooled/prisma), Supabase local linking, scoped/rotatable tokens, richer console. |
| ⚪ N/A-by-design | ~40 | Multi-tenancy: accounts, projects, orgs/teams/invites/roles, BYOC environments, "current connector/project", telemetry. Self-hosting makes these moot. |

**snapshotdb does several things Ardent does not**: multi-engine (Postgres, MySQL, MongoDB,
SQLite — Ardent is Postgres-only); branch-from-any-branch (git-like, not just from source);
prepared immutable snapshots with prestarted ready-branch pools (`prepare` + claim-on-`create`);
`import` from a stopped data directory / file / `--new`; fully self-hosted data plane by default;
no telemetry; Apache-2.0.

### Highest-value gaps to close (ranked)

1. **A structured docs site.** Ardent has ~22 docs pages (quickstart, per-provider connectors, CLI ref, REST ref, architecture, security, FAQ, 4 workflow guides). snapshotdb has only `README.md` + a few `docs/*.md`. This is the single biggest developer-facing gap. → `site-plan.md`.
2. **Expanded web console.** `console.html` is a one-page source list + create/delete + connection config. Missing: source (connector) detail with replication health/WAL lag, branch list + branch detail, preflight-in-UI, job/operation progress, settings editor. → `site-plan.md`.
3. **Branch URL variants (`--url-type direct|pooled|prisma`).** We emit one native URL. ORMs (Prisma) and poolers (PgBouncer) are common; at minimum document the single URL, ideally add variants.
4. **Scoped / rotatable server tokens ("API keys").** One static `SNAPSHOTDB_TOKEN` today. Add multiple named tokens, revoke, and (optionally) read-only scope. High value for CI + agents.
5. **Per-table replica-identity decisions.** We have a blanket `--fix-replica-identity`; Ardent asks per table (PK / unique / `REPLICA IDENTITY FULL` / exclude). Add per-table control and clearer preflight reporting.
6. **First-class quarantine + retry-setup UX.** We fold paused replication into `status`/`repair`/`reset`/`reconcile`. Name and surface a "paused / quarantined" state and a one-command resume, in CLI and console.
7. **REST-native resource endpoints.** Everything runs through the generic `POST /v1/commands` job RPC. Add `GET /v1/branches`, `GET /v1/sources`, etc. so integrators build dashboards without shelling the CLI.
8. **Auth/identity introspection.** `GET /v1/health` returns version only; add a `whoami`/auth-check (token valid? server identity?) and a CLI `status`-style command + console indicator.
9. **CI/CD reusable action + agent workflow docs.** Ardent ships a `.github/workflows/*.yml` recipe and an agent skill page. We have `skills/snapshotdb/SKILL.md` but no workflow docs or reusable Action.
10. **Supabase local-dev linking (`settings supabase link/unlink/status`).** Point local Supabase at a branch. Niche but a genuine dev-loop convenience; ❌ today.
11. **Discover + selection as explicit steps.** We select schemas at `sync` time via `--schemas`; expose discovery (what schemas/tables exist) and selection independently for large sources.
12. **Operation/job progress surfacing.** We have `--detach` + `job <id>`; add stage labels / progress (as Ardent's operations do) and show them in the console.

Everything ranked below #10 is polish; items #1–#2 dwarf the rest in developer impact.

---

## 1. Auth & accounts

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `ardent login` — GitHub OAuth in browser, creates/opens an account | ⚪ N/A-by-design | No user accounts. One deployment, one admin `SNAPSHOTDB_TOKEN`. |
| `ardent login --token <t>` — headless token auth; `ARDENT_TOKEN` env | 🟡 partial | Token auth exists (`SNAPSHOTDB_TOKEN`, Bearer to the server), but there is no `login` command that stores it; it lives in the environment. |
| `ardent logout` — clears `~/.ardent/config.json` (does not revoke) | ⚪ N/A-by-design | No stored session to clear; unset the env var. |
| `ardent status` — authenticated?/account/email/org/token preview | 🟡 partial | `GET /v1/health` proves reachability + version. No identity/auth-check. Gap #8: add a `whoami`/auth verify. |
| Accounts, sessions, GitHub OAuth, `~/.ardent/config.json` state | ⚪ N/A-by-design | Self-hosted; no identity provider. |
| PostHog CLI telemetry (`POST /v1/posthog/event`, always on) | ⚪ N/A-by-design | snapshotdb sends no telemetry — a deliberate plus. |
| Offline cache of last known state (`⚠ Offline - showing cached data`) | ❌ missing | CLI is thin and online-only; every command is a server job. Low value. |

## 2. Projects

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `project create` / `list` / `switch` / `delete` — top-level grouping of connectors | ⚪ N/A-by-design | No project grouping inside one server. Separation is per-deployment (`SNAPSHOTDB_HOME` / distinct `serve` instances). |
| "Current project" concept remembered by the CLI | ⚪ N/A-by-design | snapshotdb addresses roots/branches by name directly; only a "current branch" exists (`switch`). |
| `POST/GET/PATCH/DELETE /v1/projects[/{id}]` | ⚪ N/A-by-design | No project resource. |

## 3. Connectors / Sources

Ardent "connector" = a configured Postgres source with managed replication setup.
snapshotdb equivalent = a **root** created by `sync` (live source), `clone` (alias of sync), or
`import` (stopped datadir / file / `--new`).

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `connector preflight <type> <url> [--schemas]` — non-destructive readiness + grant script | ✅ have | `snapshotdb preflight <postgres\|mysql\|mongodb> <url> [--schemas a,b] [--format json]`; exit 2 on fail; prints grant SQL. Multi-engine (Ardent is Postgres-only). |
| `connector create` — store config + start setup (discover → select → engine-setup) | ✅ have | `snapshotdb sync <engine> <name> <url> [--schemas]` builds the replica and starts replication; `clone` is the inferred-engine alias. Also `import` for datadir/file/`--new`. |
| `connector discover` — async schema discovery, poll operation | 🟡 partial | Schemas are chosen at `sync` time via `--schemas`; there is no standalone discover step. Gap #11. |
| `connector selection` (`selected_paths`) — pick DBs/schemas to replicate | 🟡 partial | `--schemas a,b` at sync. No post-hoc re-selection API. |
| `replica-identity-decisions` — per-table PK/unique/`REPLICA IDENTITY FULL`/exclude | 🟡 partial | Blanket `--fix-replica-identity`; preflight flags tables. No per-table decision model. Gap #5. |
| `connector engine-setup` — provision the branch target | ⚪ N/A-by-design | No separate branch target; the replica **is** the parent, branched by copy-on-write. |
| `connector status` — readiness / `branching_engine_status` / errors | ✅ have | `snapshotdb status <name>` shows replication state, WAL retention on the source, and the worker's error. |
| `connector list` (`* = current`) | ✅ have | `snapshotdb list` lists roots + branches (no "current connector" marker; branches have a current). |
| `connector switch` — set current connector | ⚪ N/A-by-design | No current-connector concept. |
| `connector update` — change URL / TLS certs / branch settings | 🟡 partial | `settings <root> set source <url>` rotates credentials; TLS goes in the URL. No `--ssl-cert/--ssl-key` flags. |
| `connector retry-setup` — retry branch setup | 🟡 partial | Covered by `repair` (resume paused replica / skip poison txn), `reconcile` (add gained columns/tables), `reset` (re-clone). Different verbs, same intent. Gap #6. |
| `connector quarantine list` / `release` — paused CDC deployments | 🟡 partial | A stuck transaction pauses the stream; `status` shows it, `repair` resumes. No named "quarantine" list/release. Gap #6. |
| `connector lock` / `unlock` — deletion protection | ✅ have | `snapshotdb lock\|unlock <name>` protects a root/branch from `rm`. |
| `connector delete` (`--force` skips replication wait) | ✅ have | `snapshotdb rm <name>` tears down engine + slots/publications/triggers on the source. No `--force`-style wait toggle (rm is immediate). |
| Static outbound IP allowlist for the source | ⚪ N/A-by-design | Your server has whatever egress you give it; no managed IP. See `environment --allowlist` (§4). |
| Extension exclusion prompt (`--drop-extensions`) during setup | 🟡 partial | Unsupported extensions handled implicitly on clone; no interactive exclusion prompt. |

## 4. Environments / BYOC

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| "Managed, your cloud" (BYOC) data plane — replicator + Kafka + replica + branches in your account | ⚪ N/A-by-design | snapshotdb is self-hosted by definition: you already run the whole data plane. No Kafka pipeline — it uses the engine's native replication (Postgres logical, MySQL GTID, MongoDB change streams). |
| `environment show <id>` / `--allowlist` (source-side IP allowlist JSON) | ⚪ N/A-by-design | No environment resource; source access is whatever your server's network allows. |
| `connector create --byoc --environment-id --private-link-id --allow-high-rtt-placement` | ⚪ N/A-by-design | Placement/private-link are hosted-multi-region concerns; a single server has one location. |
| Fully-managed vs your-cloud deployment matrix; data residency | ⚪ N/A-by-design | Residency is inherent: data never leaves the box you deployed. |

## 5. Branches

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `branch create <name>` — isolated branch from current connector; auto-checkout | ✅ have | `snapshotdb create <name> --from <parent> [--print-url] [--format json]`; becomes current; idempotent. |
| `--print-url` (script-safe URL only) | ✅ have | Same flag, same contract (empty output = failure). |
| `--format json` (stable object, `schema_version`) | ✅ have | `--format json` with `schema_version`; JSON-wrapped errors; exit codes 0/1/2. |
| `--url-type direct\|pooled\|prisma` | ❌ missing | Single native URL only. Gap #3 — add pooled/prisma renderings (or document the one URL). |
| `-s/--service <type>` (branch service type) | ⚪ N/A-by-design | No service-type abstraction; engine is fixed by the parent. |
| Branch from source / point-in-time snapshot | ✅ have | And **more**: branch from *any* branch (git-like tree), not just the source. |
| `branch info` — URL/status/details | ✅ have | `snapshotdb info [name] [--print-url] [--format json]`; plus `snapshotdb url [name]`. |
| `branch list` (`*`/`●`/`○`, readiness, age, idle) | ✅ have | `snapshotdb list [--format json]`. State surfaced via `info`/`status`. |
| `branch switch` | ✅ have | `snapshotdb switch <name>`. |
| `branch delete` (queue + wait; re-run joins delete) | ✅ have | `snapshotdb rm <name>`; `--detach` + `job <id>` for async. |
| Auto-suspend after 5 min idle; instant resume; stable URL | ✅ have | Proxy suspends idle branches, resumes on next connection; URL never changes. `SNAPSHOTDB_IDLE_MINUTES` tunable. Synced roots never suspend. |
| Idempotent create / resumable long ops | ✅ have | Idempotent `create`; `--detach` returns a job ID; `job <id>` reconnects; lost client connection does not cancel the server op. |
| Branch readiness/statuses vocabulary | 🟡 partial | `info`/`status` expose running/suspended/snapshot + replication state; no rich `branching_engine_status` enum. |
| Prepared/ready branch pools | ✅ have (no Ardent equivalent) | `snapshotdb prepare <snapshot> --from <parent> --count <1-32>` freezes an immutable snapshot and prestarts a pool; `create --from <snapshot>` claims a ready, already-running branch (sub-second). snapshotdb **plus**. |
| Per-branch isolated credentials; leaked URL can't reach source | ✅ have | Every branch rotates to its own generated admin password on first start; Postgres trusts only its Unix socket; Mongo runs with a keyFile. |

## 6. Settings

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `settings list` — current connector settings | 🟡 partial | `snapshotdb settings <root>` manages keys; no dedicated formatted "list" subcommand documented. Add a plain-print/list. |
| `settings set default_db <db>` | ✅ have | `settings <root> set default_db <db>` (database name in branch URLs). |
| `settings set branch_sql @seed.sql --hook <name> [--database --order]` | ✅ have | `settings <root> set branch_sql @file --hook <name>`; multiple hooks run in name order; runs once per new branch, never on the source. No `--database`/`--order` flags (ordering via hook name). |
| `settings remove <key> [--hook <name>]` | ✅ have | `settings <root> remove <key> [--hook <name>]`. |
| Credential rotation (Ardent: via `connector update`) | ✅ have | `settings <root> set source <url>` rotates source credentials without re-syncing. |
| `settings supabase status\|link\|unlink` — point local Supabase at a branch | ❌ missing | No Supabase-specific local linking. Gap #10 (nice-to-have dev loop). |

## 7. Team / Org

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `invite <email>` / `invite list` / `invite delete` | ⚪ N/A-by-design | No users/orgs. Access = possession of the server token. |
| `org members` / `org set-role` / `org remove` | ⚪ N/A-by-design | No membership or roles. |
| `GET/PATCH/DELETE /v1/orgs/{id}`, `/members`, `/invites`, `/roles` | ⚪ N/A-by-design | No org resource. |
| Shared, team-visible branches ("just like git") | 🟡 partial | Branches are visible to anyone with the server token; there is no per-user view or ownership. |

## 8. API keys

| Ardent feature | snapshotdb | Notes / what to build |
|---|---|---|
| `Settings > API keys` in dashboard; `sk-ard_live_…`/`sk-ard_test_…`; secret shown once | 🟡 partial | One static `SNAPSHOTDB_TOKEN` set at `serve` time. No named keys, no create/revoke, no test/live split. Gap #4. |
| `POST /v1/orgs/{id}/api-keys` (create, roles + scopes) | ❌ missing | No key-management API; add multiple named tokens (+ optional read-only scope). |
| `GET /v1/orgs/{id}/api-keys` (list, newest first) | ❌ missing | — |
| `DELETE /v1/orgs/{id}/api-keys/{id}` (revoke) | ❌ missing | Revocation today = redeploy with a new token. |
| Roles/scopes (`role_org_member`, `connectors.read`, …) | ⚪ N/A-by-design | No RBAC; single admin authority. Optional read-only scope is the realistic subset. |

## 9. CLI commands (every Ardent command → snapshotdb equivalent)

| Ardent command | snapshotdb equivalent | Status | Notes |
|---|---|---|---|
| `ardent login` | — | ⚪ | GitHub OAuth; no accounts. |
| `ardent login --token <t>` | `SNAPSHOTDB_TOKEN` env | 🟡 | Env var, not a command. |
| `ardent logout` | — | ⚪ | No stored session. |
| `ardent status` | `snapshotdb` (health) | 🟡 | Health/version only; no identity. |
| `ardent project create/list/switch/delete` | — | ⚪ | No projects. |
| `ardent connector preflight` | `snapshotdb preflight` | ✅ | Multi-engine. |
| `ardent connector create` | `snapshotdb sync` / `clone` / `import` | ✅ | `sync` (live), `clone` (inferred engine), `import` (datadir/file/`--new`). |
| `ardent connector update` | `snapshotdb settings <root> set source` | 🟡 | Credential rotation; no TLS-cert flags. |
| `ardent connector list` | `snapshotdb list` | ✅ | |
| `ardent connector switch` | — | ⚪ | No current connector. |
| `ardent connector retry-setup` | `snapshotdb repair` / `reset` / `reconcile` | 🟡 | Same intent, different verbs. |
| `ardent connector quarantine` | `snapshotdb status` + `repair` | 🟡 | No named quarantine list/release. |
| `ardent connector lock` / `unlock` | `snapshotdb lock` / `unlock` | ✅ | |
| `ardent connector delete` | `snapshotdb rm` | ✅ | No `--force` wait toggle. |
| `ardent environment show [--allowlist]` | — | ⚪ | No environment resource. |
| `ardent branch create` | `snapshotdb create --from` | ✅ | Idempotent; auto-current. |
| `ardent branch list` | `snapshotdb list` | ✅ | |
| `ardent branch info` | `snapshotdb info` / `url` | ✅ | |
| `ardent branch delete` | `snapshotdb rm` | ✅ | |
| `ardent branch switch` | `snapshotdb switch` | ✅ | |
| `ardent settings list` | `snapshotdb settings <root>` | 🟡 | No dedicated list output. |
| `ardent settings set` | `snapshotdb settings <root> set` | ✅ | `default_db`, `branch_sql` (`--hook`), `source`. |
| `ardent settings remove` | `snapshotdb settings <root> remove` | ✅ | |
| `ardent settings supabase status/link/unlink` | — | ❌ | Gap #10. |
| `ardent invite [list/delete]` | — | ⚪ | No team. |
| `ardent org members/set-role/remove` | — | ⚪ | No org. |
| `ardent --help` | `snapshotdb` (USAGE) / `--help` | ✅ | Prints USAGE. |
| `ardent --version` | — | 🟡 | Version via `GET /v1/health`; no explicit CLI `--version` documented. |

**snapshotdb CLI commands with no Ardent equivalent** (net-additive): `serve` (deploy the
server), `clone` (inferred-engine sync), `job <id>` (reconnect to an async job), `import`
(root from datadir/file/`--new`), `prepare` (snapshot + ready-branch pool), `url` (print a
branch URL), `status` (replication state of a synced root), `repair` / `reconcile` / `reset`
(replication recovery), `start` / `stop` (engine lifecycle). Engines beyond Postgres: MySQL,
MongoDB, SQLite.

## 10. Web dashboard pages (Ardent dashboard → snapshotdb `web/console.html`)

Ardent's dashboard (`app.tryardent.com`) is referenced by the docs but not route-enumerated in
the inventory; the pages below are the resource-model views the docs imply (`Settings > API
keys`, connector setup, branches, team). Our console is a single-page app that connects with a
base URL + token and drives the server through `POST /v1/commands` + `GET /v1/jobs/{id}`.

| Ardent dashboard page (implied) | snapshotdb console equivalent | Status | Notes |
|---|---|---|---|
| Login / account | Connection modal (base URL + token) | 🟡 | Token entry, not an account. |
| Projects list | — | ⚪ | No projects. |
| Connectors (sources) list | "Sources" section (synced replicas & local roots) | ✅ | `console.html` renders the root list. |
| Connector detail — setup progress, `branching_engine_status`, replication health, WAL lag | Source meta/status (partial) | 🟡 | Shows status text; no health/WAL-lag/setup-timeline view. Gap #2. |
| Connector setup wizard — preflight → discover → select → replica-identity → engine-setup | New source form (name/url/schemas) | 🟡 | Single sync form; no in-UI preflight or per-table replica-identity. |
| Quarantine / paused CDC panel | — | ❌ | Surface paused replication + one-click resume. Gap #6. |
| Branches list | New-branch action + (implicit) root list | 🟡 | No dedicated branch list/tree view. Gap #2. |
| Branch detail — URL, status, copy, suspend/resume | Copy-URL + delete on a source | 🟡 | Branch-level detail page missing. |
| Settings — branch defaults, `branch_sql` | Settings (default database name) | 🟡 | Only `default_db`; no `branch_sql` editor. |
| Settings > API keys | Connection config (single token) | 🟡 | One token; no key management. Gap #4. |
| Team / members / invites / roles | — | ⚪ | No tenancy. |
| Environments (BYOC) | — | ⚪ | Self-hosted. |
| Billing / usage | — | ⚪ | OSS; no billing. |
| Operation/job progress | Toasts on command completion | 🟡 | No live stage/progress view. Gap #12. |

## 11. Docs pages (Ardent docs → snapshotdb equivalent)

snapshotdb docs today: `README.md`, `CHANGELOG.md`, `docs/server.md`, `docs/prepared-branches.md`,
`docs/ardent-comparison.md`, benchmark files. There is no structured docs site. Build plan in
`docs/site-plan.md`.

| Ardent docs page | snapshotdb equivalent | Status | Notes |
|---|---|---|---|
| `quickstart` | README top section | 🟡 | Content exists; no standalone page. |
| `cli/overview` | README "Commands" | 🟡 | |
| `cli/auth` | — | ❌ | No accounts; document token setup instead. |
| `cli/projects` | — | ⚪ | No projects. |
| `cli/connectors` | README engines/sync section | 🟡 | |
| `cli/branches` | README "Branches" | 🟡 | |
| `cli/settings` | README settings block | 🟡 | |
| `cli/org` | — | ⚪ | No team. |
| `cli/configuration` (env, exit codes, JSON) | Scattered in README | 🟡 | Consolidate: env vars, exit codes 0/1/2, `--format json`, `--detach`. |
| `connectors/self-hosted` | README Postgres requirements | 🟡 | We cover it inline; needs its own page. |
| `connectors/rds` | — | ❌ | Add: RDS param group, `rds_replication`, writer endpoint. |
| `connectors/supabase` | — | ❌ | Add: direct (non-pooler) connection, IPv4. |
| `connectors/planetscale` | — | ❌ | PlanetScale is MySQL — for us a MySQL-connector page. |
| (no Ardent page) MySQL / MongoDB / SQLite connectors | — | ❌ | snapshotdb-only; we must document these ourselves. |
| `architecture` | `docs/server.md` (partial) | 🟡 | Add a split of client vs server, proxy/suspend, CoW, per-engine replication. |
| `security` | README credentials/limits + `docs/server.md` | 🟡 | Consolidate: per-branch creds, socket trust, no telemetry, source footprint. |
| `faq` | — | ❌ | Add. |
| `api/overview` | README remote section | 🟡 | Document `/v1/health`, `/v1/commands`, `/v1/jobs/{id}`. |
| `api/branches`, `api/connectors`, `api/operations` | — | 🟡 | Our REST is the generic command RPC; document the job model. |
| `api/authentication`, `api/errors`, `api/api-keys` | — | ❌ | Bearer token, JSON errors, exit codes; API keys still single-token. |
| `workflows/ai-agents` | `skills/snapshotdb/SKILL.md` | 🟡 | Skill exists; no workflow page. |
| `workflows/local-dev` | — | ❌ | Add. |
| `workflows/ci-cd` | GitHub Actions e2e (internal) | ❌ | Add a user-facing recipe + reusable Action. Gap #9. |
| `workflows/regression-testing` | — | ❌ | Add. |

## 12. REST API endpoints (Ardent `openapi.json` v1 → snapshotdb server)

snapshotdb's server exposes exactly three endpoints (Bearer `SNAPSHOTDB_TOKEN`):
`GET /v1/health` (→ `{version}`), `POST /v1/commands` (submit a CLI-verb job → `202 {id,state}`),
`GET /v1/jobs/{id}` (poll → job state/exit/stderr). Plus per-branch SQLite `/v1/query?token=…`.
Every Ardent branching/connector operation maps onto `POST /v1/commands` with the matching CLI
verb; the tenancy endpoints have no counterpart. "🟡 via `/v1/commands`" means reachable but not
a REST-native resource route (Gap #7).

| Ardent endpoint | snapshotdb | Status | Notes |
|---|---|---|---|
| `POST /v1/branch/create` | `POST /v1/commands ["create","<name>","--from","<parent>"]` | 🟡 | Async job; poll `/v1/jobs/{id}`. |
| `GET /v1/branches/{connector_id}` | `POST /v1/commands ["list"]` | 🟡 | No REST list route. |
| `GET /v1/connectors` | `POST /v1/commands ["list"]` | 🟡 | |
| `POST /v1/connectors` | `POST /v1/commands ["sync",…]` / `["clone",…]` | 🟡 | |
| `POST /v1/connectors/preflight` | `POST /v1/commands ["preflight",…]` | 🟡 | |
| `GET /v1/connectors/{id}` | `POST /v1/commands ["status","<name>"]` | 🟡 | |
| `PUT /v1/connectors/{id}` | `POST /v1/commands ["settings",…]` | 🟡 | |
| `DELETE /v1/connectors/{id}` | `POST /v1/commands ["rm","<name>"]` | 🟡 | |
| `POST /v1/connectors/{id}/deletion-lock` | `POST /v1/commands ["lock","<name>"]` | 🟡 | |
| `DELETE /v1/connectors/{id}/deletion-lock` | `POST /v1/commands ["unlock","<name>"]` | 🟡 | |
| `POST /v1/connectors/{id}/discover` | — | 🟡 | Discovery folded into `sync --schemas`. |
| `POST /v1/connectors/{id}/selection` | `["sync",…,"--schemas","a,b"]` | 🟡 | Selection at sync time. |
| `PUT /v1/connectors/{id}/replica-identity-decisions` | `["sync",…,"--fix-replica-identity"]` | 🟡 | Blanket, not per-table. |
| `POST /v1/connectors/{id}/engine-setup` | — | ⚪ | No separate branch target. |
| `GET /v1/connectors/{id}/quarantine` | `["status","<name>"]` | 🟡 | Shows paused state. |
| `POST /v1/connectors/{id}/quarantine/{qid}/release` | `["repair","<name>"]` | 🟡 | Resume. |
| `GET /v1/operations/{operation_id}` | `GET /v1/jobs/{id}` | 🟡 | Our job = their operation. `?wait` long-poll not implemented. |
| `GET /v1/orgs/{id}` | — | ⚪ | No orgs. |
| `PATCH /v1/orgs/{id}` (rename) | — | ⚪ | |
| `DELETE /v1/orgs/{id}` | — | ⚪ | |
| `GET /v1/orgs/{id}/api-keys` | — | ❌ | Gap #4. |
| `POST /v1/orgs/{id}/api-keys` | — | ❌ | Gap #4. |
| `DELETE /v1/orgs/{id}/api-keys/{id}` | — | ❌ | Gap #4. |
| `POST /v1/orgs/{id}/invite` | — | ⚪ | |
| `GET /v1/orgs/{id}/invites` | — | ⚪ | |
| `DELETE /v1/orgs/{id}/invites/{id}` | — | ⚪ | |
| `GET /v1/orgs/{id}/members` | — | ⚪ | |
| `PATCH /v1/orgs/{id}/members/{id}` | — | ⚪ | |
| `DELETE /v1/orgs/{id}/members/{id}` | — | ⚪ | |
| `GET /v1/orgs/{id}/roles` | — | ⚪ | |
| `GET /v1/projects` | — | ⚪ | No projects. |
| `POST /v1/projects` | — | ⚪ | |
| `GET /v1/projects/{id}` | — | ⚪ | |
| `PATCH /v1/projects/{id}` | — | ⚪ | |
| `DELETE /v1/projects/{id}` | — | ⚪ | |

**snapshotdb endpoints with no Ardent equivalent**: `GET /v1/health`, `POST /v1/commands`
(generic authenticated job RPC), `GET /v1/jobs/{id}`, and the per-branch SQLite SQL-over-HTTP
route `/v1/query?token=<branch-secret>` (see `docs/prepared-branches.md`).
