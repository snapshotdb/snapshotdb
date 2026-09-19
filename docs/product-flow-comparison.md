# SnapshotDB versus Ardent: product flow audit

**Historical baseline:** these measurements and containment findings precede the
security fixes. See [the remediation report](security-hardening.md) for the new
behavior, upgrade requirements and protected 1 TB timings.

Measured and inspected 13 September 2026. **Direct competitors for the core use
case: sync a source once, create writable point-in-time branches, hand agents URLs.**
SnapshotDB is faster in the completed deployment tests, but the current deployment
does not provide equivalent agent security. It is not yet a demonstrated replacement
for the complete managed product. This is a scoped audit, not an exhaustive security
assessment or a claim that every advertised Ardent feature works.

## What the speed comparison proves

See [timings and raw evidence](head-to-head-results.md). Matched API tests alternate
providers and measure request through URL, fresh connection, payload read, committed
write and read-back. There are five trials per provider per size, with no SnapshotDB
prepared pool. Separately, 20 CLI trials per provider per size passed: 80 branches,
566 assertions. CLI medians were 1.033 s versus 12.293 s at 100 MB and 1.025 s versus
14.763 s at 10 GB. Do not mix CLI and API samples.

| API measurement, median | SnapshotDB | Ardent |
|---|---:|---:|
| 100 MB: URL | 0.882 s | 6.488 s |
| 100 MB: usable read/write | 1.149 s | 9.271 s |
| 10 GB: URL | 0.873 s | 7.943 s |
| 10 GB: usable read/write | 1.167 s | 11.228 s |

The server-local SnapshotDB read/write median was 0.736 s for both sizes. Ardent's
worker-only medians were 3.543 s and 4.844 s; these exclude client SQL and networking.
That worker result is consistent with its published under-six-second branching
claim for these sizes. Its advertised boundary is not defined precisely enough to
equate it to our laptop CLI timer. Initial setup is explicitly excluded from that
claim in [Ardent's FAQ](https://docs.tryardent.com/faq).

SnapshotDB initial copy: 5.827 s / 87.644 s / 2h49m21.477s for 100 MB / 10 GB / 1 TB.
Ardent completed setup: 4m15.592s worker / 11m40.099s including queue at 100 MB;
29m43.443s worker / 29m44.540s including queue at 10 GB. The corrected 1 TB operation
was still running at this audit. There is no completed 1 TB head-to-head result.

SnapshotDB uses a 16-vCPU Mumbai host; Ardent assigned m6i.xlarge workers in Virginia.
SnapshotDB's source and target share a host; Ardent copies across regions. Managed
provisioning and validation also differ. These results establish a deployment win,
not a normalized cost, hardware, topology, or universal product win.

## Flow-by-flow coverage

“Documented” below means vendor documentation or implementation inspection, not a
successful matched live test. A blank verification is not a provider failure.
Both providers passed the additional committed-write, rollback, identity, constraint
and branch-migration checks in the [SQL flow evidence](benchmarks/2026-09-13-sql-flow-audit.json).

| Flow | SnapshotDB evidence | Ardent evidence | Comparison status |
|---|---|---|---|
| Remote initial clone | 100 MB, 10 GB and 1 TB completed; files on EC2 | 100 MB and 10 GB completed; 1 TB pending | Two completed size comparisons |
| Source writes reach replica/next fresh branch | Fresh nonce in every CLI trial | Same fresh nonce checked | Passed for the tested source workload |
| Return URL and read actual copied payload | Native PostgreSQL connection, eight tables, minimum size, payload checksum | Same checks | Passed; size checks do not checksum every row |
| Commit branch writes without changing source | Source and replica marker unchanged | Source marker unchanged | Passed ordinary SQL isolation |
| Transactions, constraints and migrations inside a branch | Additional identical SQL flow on retained 100 MB branch | Same SQL flow | See separate SQL evidence; not source metadata fidelity |
| Untrusted agent containment | **Sibling private file readable with returned credentials** | Returned role is not superuser and lacks server-file-read role | SnapshotDB fails tested containment boundary; Ardent OS isolation not independently audited |
| Anonymization/initialization hook | **Failed SQL still returns success and URL** | Per-branch SQL documented; failure behavior not tested | SnapshotDB fail-open confirmed |
| Point-in-time branch | Detached replica configuration; later source changes should not stream in | Point-in-time behavior documented | Existing checks cover writes; long-duration paired detachment test still needed |
| Prepare snapshot before request | Explicit ready pool, finite capacity; snapshot freshness frozen | Internal preparation not controlled | Compare prepared SnapshotDB separately |
| Idle suspend and wake | Proxy implementation and prior SnapshotDB tests | Five-minute auto-suspend documented | No matched idle/wake latency benchmark yet |
| Reset, deletion, URL revocation | Reset/rm implemented; benchmark cleanup submitted | Branch lifecycle documented; cleanup submitted | No complete paired active-session/revocation timing test |
| Full source fidelity | Known DDL replay limitations; no complete grants/RLS/extensions matrix | Schema, grants, RLS, functions, triggers and supported extensions documented | No demonstrated full parity |
| Access policy and routing | Single deployment admin token; SSH used for benchmark URLs | Account/branch policy and managed TLS routing documented | Material product difference |
| Resource isolation and capacity | Shared host/user; serialized engine jobs; finite explicit pools | Managed autoscaling documented | No matched saturation, resource fairness or failure test |
| Recovery under failure | Interrupted jobs require inspection/resubmission; repair commands exist | Replication recovery documented | No matched failover, crash-consistency or RPO/RTO result |
| Other databases | PostgreSQL, MySQL, MongoDB, SQLite support; prior SnapshotDB tests | Public FAQ currently lists PostgreSQL | Direct cross-provider results apply only to PostgreSQL |

Architecture/lifecycle claims are from [Ardent architecture](https://docs.tryardent.com/architecture).
Credential, TLS and branch SQL claims are from [Ardent security](https://docs.tryardent.com/security).
Database support, idle behavior and fidelity claims are from [Ardent FAQ](https://docs.tryardent.com/faq).

## Confirmed gaps that matter before giving URLs to agents

### 1. Branch database credentials cross the filesystem boundary

Tested two retained 100 MB SnapshotDB branches on EC2. Created a random, harmless
mode-0600 canary in branch B's directory. Connected over TCP to branch A with its
normal generated credentials. The role was superuser, and PostgreSQL `pg_read_file`
returned the sibling canary. Only the equality result was printed; the canary was
removed. No actual secrets or unrelated files were read, and no shell execution was
attempted. All engines run as the same OS user on the shared filesystem.

On a retained Ardent 100 MB branch, the returned role had `rolsuper=false` and
`pg_has_role(current_user, 'pg_read_server_files', 'USAGE')=false`. This is a narrower
live check than proving its complete infrastructure isolation.

Separate database passwords prevent ordinary cross-connection authentication; they
do not contain a SQL superuser sharing the server's filesystem. SnapshotDB needs a
restricted agent database role and an enforced per-branch OS/filesystem boundary.
Merely changing TCP passwords does not fix this. See `src/main.rs` `admin_user`,
`initdb`, and credential creation.

### 2. A failed branch SQL hook does not block readiness

On a disposable SnapshotDB root, configured a call to a nonexistent SQL function as
the branch hook. Creating a child returned exit code 0 and a PostgreSQL URL, emitted
a warning, and wrote `run/branch_sql.done`. Both disposable databases were removed.
This reproduces the fail-open path in `src/main.rs` around lines 693–699.

If that hook masks production data, its failure can expose the original data to an
agent. Creation must fail closed, withhold readiness and URL access, and preserve
the failed state for repair. Marking all hooks complete after a warning is unsafe.
Ardent's equivalent failure behavior was not tested.

### 3. Schema fidelity and operational semantics still need parity tests

The README already documents a DDL replay limitation: a single query string mixing
DDL and DML can replay DML twice and pause replication. Concurrent index creation
is replayed as ordinary index creation. The fallback reconciliation adds missing
columns/tables rather than reproducing every migration. Validate grants, ownership,
RLS, functions, extensions, sequences and difficult migrations from source through
replica into branch; branch-local SQL success alone does not prove copied fidelity.

Also test branch deletion with active sessions, idle wake under load, pool exhaustion,
server restarts, WAL pressure and source disconnects. These are unmeasured boundaries,
not evidence that Ardent passes or SnapshotDB fails them.

## Why it is fast, and the next fair comparison

The normal branch operation uses copy-on-write storage rather than copying all source
bytes again. A running server, local source/replica topology, simple process startup,
and lightweight control path help. Ardent also documents copy-on-write, so that
technique alone is not a unique advantage. Prepared SnapshotDB capacity moves engine
startup before the request; it must be labeled separately from a fresh branch.

We have not measured how much of the speed difference is caused by each factor, nor
the latency cost of fixing security. Preserve these measurements, fix containment
and hook readiness first, then rerun exactly the same API flow. Finally equalize
region/resources as far as the managed service permits and add paired concurrency,
lifecycle, fidelity and recovery tests. Until then the accurate claim is: faster
usable PostgreSQL branches in these tested deployments, with important remaining
security and managed-service differences.
