# Agent branch latency — 13 September 2026

All four supported engines beat one second for prepared branch allocation, a new
authenticated connection, seeded-data read, committed write, and read-back. Database
copies and processes stayed on the Mumbai EC2 server. The working tree is based on
`b485c5e` with the unreleased prepared-pool and remote SQLite changes.

| Engine | Server-local p50 | Server-local maximum | Laptop → Mumbai maximum |
|---|---:|---:|---:|
| PostgreSQL | 103 ms | 124 ms | 356 ms |
| MySQL | 165 ms | 179 ms | 437 ms |
| MongoDB | 94 ms | 117 ms | 356 ms |
| SQLite over HTTP | 56 ms | 62 ms | 258 ms |

The server-local run used 12 claims per engine, in three bursts of four concurrent agents.
The laptop run used four concurrent claims per engine, over an SSH tunnel with port
forwarding configured during preparation. All 64 timed claims were below one second.
These small samples establish observed performance, not a latency SLA or an unlimited
capacity guarantee. The machine was a 16-vCPU, 32-GiB `c6i.4xlarge` in `ap-south-1`, with
Btrfs storage and the separate 1 TB source-generation benchmark running concurrently.

Fixtures contained 1,000 records. PostgreSQL, MySQL, and MongoDB were cloned from source
connection strings before preparation; SQLite started from an imported server-side root.
The network-engine measurements used psycopg, PyMySQL, and PyMongo directly. SQLite used
its authenticated HTTP endpoint. CLI process startup and network operations are inside
the timer. Creating the snapshot, warming engines, and configuring network routes are outside.

Preparing the first four available branches took 1.68 s for PostgreSQL, 7.63 s for MySQL,
12.04 s for MongoDB, and 0.29 s for SQLite. Refills reused the same immutable snapshot.
Preparation shares the command queue, so it should finish before the agent burst begins.
With the polling delay already removed, cold MySQL branches still took 2.59–3.19 s and
cold MongoDB branches 3.64–3.75 s. Preparing running capacity removes those engine startup
costs from agent allocation.

The run passed 172 correctness checks, including source and sibling isolation, distinct
URLs, idempotent claims, preserved snapshot contents despite later source writes, exhausted
pool errors, protected snapshot deletion, and absent client storage. A separate 33-check
run restarted the whole candidate server, waited 70 seconds with a one-minute idle policy,
then verified stable URLs, committed data, warmed capacity, and rejected sibling credentials.
Its slowest new claim and read/write took 202 ms. HTTP integration tests also verified
SQLite transaction rollback and refusal of `ATTACH`, `VACUUM INTO`, and unsafe pragmas.

The existing engine E2E suite passed all 94 assertions. The candidate also passed eight
Rust tests and seven Python benchmark tests. A real MySQL `/app` cloning failure found
during this work was fixed: administrative credential setup no longer selects an application
database before that database has been restored.

## 1 TB results

The retained 1 TB benchmark and follow-up checks passed. Four concurrent prepared
PostgreSQL agents completed allocation, URL retrieval, a new authenticated connection,
read, committed write, and read-back in **64.13, 65.79, 103.62, and 105.35 ms** on EC2.
The prepared baseline contained **1,000,061,067,264 table/index bytes**. Preparing four
running branches took 4.913 seconds, excluded from allocation timing. These are prepared
capacity results; MySQL, MongoDB, and SQLite were tested on the smaller fixtures above.

Twenty fresh PostgreSQL branches from the synced 1 TB replica, with no prepared pool,
completed the same read/write endpoint test in **997 ms median, 1,219 ms p95, and
1,302 ms maximum** on EC2. Each request used a new branch name, and each branch included
a newly replicated source marker. All eight large tables were checked. The first trials
were included. These fresh branches did not consistently beat one second.

Twenty additional fresh 1 TB trials from the laptop completed in **1,413 ms median,
1,801 ms p95, and 1,830 ms maximum**. The timer includes local CLI startup, remote branch
creation, URL retrieval, new SSH forwarding, authentication, data reads, a committed write,
and read-back. All database storage remained remote. Source and replica isolation passed
for every trial. Each percentile uses all 20 successful trials; p50 is the median and p95
uses nearest rank.

The shutdown regression found during these tests was fixed by stopping replication and
database processes before their proxies, handling SIGTERM, and using systemd
`KillMode=mixed`. The retained 1 TB suite passed 45 checks, followed by eight Rust tests,
seven Python benchmark tests, and 94 engine E2E assertions. Original failed reports were
preserved on the server, along with the 1 TB data and extra disk requested by the user.

See the [prepared 1 TB evidence](benchmarks/2026-09-13-prepared-1tb.json),
[fresh server-local evidence](benchmarks/2026-09-13-fresh-1tb.json),
[fresh laptop evidence](benchmarks/2026-09-13-fresh-1tb-laptop.json), and
[Ardent comparison status](ardent-comparison.md). The live retained service uses
`/usr/local/bin/snapshotdb-fair`; the original executable is preserved.

See [raw timing and correctness evidence](benchmarks/2026-09-13-latency.json),
[usage and limitations](prepared-branches.md), [server-local runner](../scripts/latency.py),
[remote-client runner](../scripts/latency_remote_client.py), and
[restart runner](../scripts/latency_lifecycle.py).
