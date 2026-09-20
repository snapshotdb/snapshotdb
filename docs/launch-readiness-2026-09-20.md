# Launch readiness check — 2026-09-20

This is a bounded functional audit, not a guarantee that every deployment or workload is safe. Tests used isolated synthetic databases; existing customer databases were not modified.

## Verified

- PostgreSQL 16, MySQL 8, and MongoDB 8 on Ubuntu 24.04 with Btrfs: **127 end-to-end assertions passed**, zero failed. Coverage includes initial copy, live replication, DDL, conflict repair, freshness checks, independent branch credentials, filesystem/role restrictions, branch isolation, suspend/resume, settings, reset, and cleanup.
- Rust unit/integration suite: **9 tests passed** on macOS/APFS and Linux/Btrfs. Integration covers authenticated remote jobs, CORS allow/deny behavior, SQLite transactions and isolation, prepared pools, initialization hooks, SQLite stop/start, and server restart with data preservation.
- Python benchmark/support suite: **10 tests passed**.
- Web session tests: **3 passed**, covering tampering, malformed signatures/identities, expiration, and missing secret. Web lint and production build passed.
- All **21 landing, console, and documentation routes** returned HTTP 200 locally, with no old personal-repository links in their rendered HTML.
- Real browser checks: GitHub login, authenticated cross-origin connection, failed PostgreSQL preflight checklist with clone disabled, SQLite source creation, child and grandchild creation/display, SQLite stop/start, and console error log inspection.
- Responsive checks at 390px: landing stats use equal columns and gutters; landing, console, and CLI documentation fit without horizontal page overflow. Desktop stats use four equal columns.
- Installer downloaded the currently hosted macOS arm64 binary, verified its SHA256, and executed version 0.4.0 in a disposable prefix. Deployment shell scripts passed syntax checks.

## Corrections

- Added the missing apex-domain OAuth callback to the existing SnapshotDB OAuth app. Login on `snapshotdb.io` and `www.snapshotdb.io` was verified. A new organization-owned app is not required for repository transfer.
- Added explicit console-origin CORS configuration without bypassing API authentication.
- Preserved preflight failure details and selected PostgreSQL schemas; editing source fields invalidates prior preflight approval.
- Fixed nested branch visibility, missing restart controls, inappropriate immutable-snapshot controls, stale connection state, cancellation on unmount, and mobile layout.
- Fixed SQLite stop reporting success while leaving the endpoint running; restart preserves the URL and data.
- Added signed session expiration and fail-closed secret validation; hardened CLI callback validation and credential writes.
- Corrected GitHub links, old deployment binary/unit names, unsupported documentation commands, and installer checksum validation.
- Qualified performance claims using the existing September 13 benchmark evidence: prepared PostgreSQL claims had a 94.1ms read/write median; preparation and initial replication were excluded. This audit did not rerun the 1TB benchmark.
- Refreshed CI package indexes before installation and added web lint/test/build checks.

## Deployment requirements

Deploy the site and rebuild/redeploy the SnapshotDB server from this commit. On the API server set:

```sh
SNAPSHOTDB_CONSOLE_ORIGINS=https://snapshotdb.io,https://www.snapshotdb.io
```

Serve the remote control API over HTTPS. Keep `SNAPSHOTDB_TOKEN` configured. The site requires `AUTH_SECRET` with at least 32 characters, plus the existing GitHub OAuth client ID and secret. Older browser sessions will need to sign in again because they lack signed expiry.

## Still outside this verification

- No deployment was performed: the owner requested repository push and will deploy. Served production changes and current-head hosted CI must be checked after push/deployment.
- Static binaries under `site/public/dl` were not rebuilt by this audit. Their published 0.4.0 checksum was verified, but they do not include these server fixes. Rebuild release artifacts before offering the updated server through downloads.
- AWS RDS, Supabase, new BYOC appliances, and the Fly image were not provisioned or certified end to end. Deployment path/name fixes and shell syntax are not provider qualification.
- The historical EC2 benchmark instance was unavailable in the current AWS account. No new 1TB latency or load/recovery benchmark was run.
- Repository visibility remains private by the owner's pre-launch instruction; public GitHub installation/links must be verified when it is made public.
