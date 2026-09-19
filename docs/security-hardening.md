# Agent branch security and flow fixes

This change addresses the concrete containment, hook-readiness, replication and
lifecycle problems found during the Ardent comparison. Historical latency results
in the comparison reports predate these protections and should not be presented as
measurements of the updated implementation.

## Changes

- Child database URLs use a restricted `snapshotdb_agent` account. PostgreSQL removes
  superuser, replication, role-administration and inherited membership privileges;
  MySQL grants application-database privileges without server administration or file
  export; MongoDB grants data/database administration without user administration.
  Separate maintenance credentials remain in private server metadata. PostgreSQL
  object ownership is assigned for agent migrations, not exact source RLS semantics.
- Linux branch engines run in bubblewrap mount namespaces. Only their data/runtime
  directories are writable; sibling directories, sibling Unix sockets, server
  credentials and host process files are absent. System binaries, libraries and
  selected CPU/memory information are read-only. MySQL file import/export is disabled.
  Missing sandbox support fails startup rather than falling back to an exposed process.
- Proxy connections and URLs are gated on completed initialization. A failed SQL/JS
  hook fails branch creation, and successful hooks are recorded privately for retries.
  SQLite uses the same rule. Prepared children inherit initialized snapshot data;
  they no longer run the snapshot's hooks a second time.
- PostgreSQL replay extracts top-level DDL from mixed DDL/DML batches, preserving
  quoted strings, dollar-quoted bodies and nested comments. DML is left to logical
  replication. Recorded schema errors block new branches; repair retries them once
  missing prerequisites have been corrected. Procedural/dynamic DDL, `CREATE TABLE AS`
  and other unsupported forms fail closed for explicit reconciliation.
- MySQL refuses branches while replication is paused/disconnected. MongoDB stops on
  an application failure without advancing its resume token, rejects new branches,
  and retries the event after repair. Command-level MongoDB errors are checked too.
- Deletion prevents proxy wakeups, waits for the proxy to exit, then stops the engine
  and removes data. Failed startup keeps a private diagnostic log before cleanup.
  Parent-restart failures also clean up incomplete child directories.

## Verification

The Linux end-to-end suite exercises source setup, live replication, schema changes,
repair, native branch writes, source isolation, suspend/resume, reset and teardown.
Additional checks exercise restricted accounts, attempted file/program access,
sandboxed-superuser canary reads, failed hooks, inherited PostgreSQL memberships,
and a deliberately failed MongoDB event followed by a successful retry.

Rust client/server integration covers SQLite isolation, transactional rollback,
prepared capacity, hooks running once, failed-hook URL denial and refusal of
unverifiable legacy hook completion. Python benchmark-runner tests also run.
The full engine run passed 115 checks. Subsequent PostgreSQL migration coverage passed
76 checks, and the focused MySQL/MongoDB run passed 44 checks (these suites overlap;
the counts are not additive). Local verification passed 8 Rust tests and 7 Python tests.
[Engine verification evidence](benchmarks/2026-09-13-security-e2e.json).

## Updated 1 TB latency

The test uses a stopped retained snapshot with over 1 TB of PostgreSQL tables, cloned
with Btrfs reflinks into a separate test server's storage. The original benchmark data
is not modified. The timer covers local CLI launch, API creation, URL, a fresh native
connection, reading copied data, a committed write and read-back. It also verifies
the returned role is non-superuser and cannot read server files. This is a protected
SnapshotDB test, not a new simultaneous Ardent comparison.

Measured table size: **1,000,060,870,656 bytes**.

| Mode | Samples | URL median | Read/write median | Read/write range |
|---|---:|---:|---:|---:|
| Fresh branch | 5 | 2.509 s | 2.554 s | 1.569–2.656 s |
| Prepared claim | 3 | 42.7 ms | 94.1 ms | 93.9–94.8 ms |

[Raw protected 1 TB samples](benchmarks/2026-09-13-secure-1tb.json).
The first attempted fixture was an older snapshot below the strict 1 TB table-size
threshold; it was rejected and excluded. These samples use the later verified snapshot.
Prepared claims remain a separate operation from fresh creation: pool preparation
is outside their timer. These measurements are not a latency guarantee under arbitrary
load or from a remote client.

## Upgrade

Validation used separate EC2 test server processes. The retained comparison services
were not replaced by those test processes; apply the following upgrade before treating
an existing deployment's URLs as protected by these changes.

1. Install bubblewrap on the Linux server. On Ubuntu run
   `sudo bash deploy/install-sandbox.sh`. The AppArmor rule permits the distro-owned
   bubblewrap binary to create namespaces without disabling the global restriction.
2. Build and install the updated binary, then restart the service using graceful
   shutdown (`KillMode=mixed` in the supplied unit). Existing unsandboxed branch
   processes must restart; replacing the file alone cannot contain a running process.
3. Retrieve new child URLs. Maintenance passwords rotate when restricted credentials
   are installed, so old administrator URLs are not the agent interface anymore.
4. Legacy hook markers cannot establish whether old hooks succeeded. Branches with
   configured hooks and unverifiable completion refuse readiness: reset/recreate
   them, or prepare a new snapshot. Do not treat an old warning as successful masking.
5. Check replica status before creating new branches. Correct failed schema/event
   prerequisites, then run repair. Failed-start diagnostics are private files under
   the server's `.failed` directory and should be retained only as needed.

Linux is required for PostgreSQL, MySQL and MongoDB agent branches. Clients can run
elsewhere, and SQLite is still supported without a Linux engine sandbox. Unmanaged
imported data directories cannot produce agent branches; use a managed root or sync.

## Remaining product boundaries

This remains a single-administrator deployment. It does not add per-user API policy,
compute quotas, managed failover, automatic public TLS routing or complete source
role/RLS/extension fidelity. Engine namespaces share host networking and compute;
they are not VM isolation or protection against kernel exploits. Database URLs belong
on a private network or encrypted tunnel. These are explicit product boundaries,
not claims that the complete Ardent managed product has been reproduced.
