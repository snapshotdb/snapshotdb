# SnapshotDB versus Ardent — measured 13 September 2026

**Historical baseline:** these measurements and containment findings precede the
security fixes. See [the remediation report](security-hardening.md) for the new
behavior, upgrade requirements and protected 1 TB timings.

**SnapshotDB won the completed 100 MB and 10 GB deployment-latency tests.** Every
matched API trial was faster for SnapshotDB. This is an observed result on these
deployments, not an identical-hardware comparison or a latency guarantee. The 1 TB
Ardent setup is still running, so there is no 1 TB head-to-head winner yet.

## Same API-to-database test

Five trials per provider and size, alternating providers, on the same laptop. Each
trial requests a fresh branch through the provider API, polls at a nominal 100 ms
interval, obtains its URL, opens a new authenticated PostgreSQL connection, reads
a 1,700-byte payload from the source data, creates a table, commits an insert, and
reads it back. Initial copy and branch cleanup are outside the timer. SnapshotDB's
new SSH forwarding is inside it. There is no prepared SnapshotDB pool. Ardent's
internal capacity preparation is managed by Ardent and was not controlled by us.

| Dataset | SnapshotDB URL median | Ardent URL median | SnapshotDB read/write median | Ardent read/write median |
|---|---:|---:|---:|---:|
| 116 MB source | 0.882 s | 6.488 s | **1.149 s** | 9.271 s |
| 10.77 GB source | 0.873 s | 7.943 s | **1.167 s** | 11.228 s |

SnapshotDB's median end-to-end result was approximately **8.1x** faster at 100 MB
and **9.6x** faster at 10 GB in this run. Observed read/write ranges:

| Dataset | SnapshotDB minimum–maximum | Ardent minimum–maximum |
|---|---:|---:|
| 116 MB | 1.144–1.183 s | 9.091–9.839 s |
| 10.77 GB | 1.131–1.192 s | 10.706–12.274 s |

All 20 trials passed native read/write validation. Each branch contained all eight
large tables and met the requested minimum physical table/index size. The identical
probe table name could be independently created in every branch, and neither source
nor SnapshotDB replica contained the branch-created probe tables. No database copy
was created on the laptop. Cleanup requests were submitted outside measured latency.

## Server-side evidence — different timing boundaries

SnapshotDB measurements run on the database's EC2 host and include local CLI launch,
server request processing, URL, authentication, and committed read/write. Ardent
measurements use its operation `started_at` to `completed_at`, excluding queue and
all client networking/read/write time. These are deliberately labeled separately:
they cannot provide an exact engine-only speedup ratio.

| Dataset | SnapshotDB local end-to-end, min / median (20 trials) | Ardent worker execution, min / median (5 trials) |
|---|---:|---:|
| 116 MB | **0.733 / 0.736 s** | 3.308 / 3.543 s |
| 10.77 GB | **0.733 / 0.736 s** | 4.672 / 4.844 s |

Ardent operation time including its queue had medians of 4.071 s and 5.493 s,
respectively. Every Ardent backend sample was followed by successful native reads,
committed writes, and minimum-size verification. SnapshotDB maximums were 1.200 s
at both sizes, so fresh branches did not consistently meet the sub-second target
under this concurrent load. Prepared-branch results are a separate benchmark.

Do not subtract Ardent branch `created_at` from `write_ready_at`: observed branch
records can have creation timestamps later than readiness. Operation timestamps
were used instead, with operation IDs retained in the raw evidence.

## Initial setup

| Dataset | SnapshotDB initial copy | Ardent worker setup | Ardent setup including queue |
|---|---:|---:|---:|
| 116 MB | **5.827 s** | 4m 15.592s | 11m 40.099s |
| 10.77 GB | **1m 27.644s** | 29m 43.443s | 29m 44.540s |
| 1 TB | **2h 49m 21.477s** | Still running | Still running |

Synthetic data generation is excluded. Ardent times begin with the setup operation,
so connector creation and earlier schema discovery are not included. These are
not identical initialization boundaries: Ardent includes managed-engine provisioning
and validation; SnapshotDB times clone submission through subscribed-table readiness
on the already deployed server. SnapshotDB copied source-to-target on the same Mumbai
host; Ardent's target is in Virginia. These topology and provisioning differences
prevent attributing the copy-time difference solely to replication software.

The 100 MB worker waited approximately 7m 25s before starting. Its start immediately
followed the 10 GB setup's completion, which is consistent with a shared worker queue,
but does not establish Ardent's scheduling policy. Earlier 1 TB attempts hit a source
wal2json configuration issue that was fixed; those failed attempts are excluded from
successful setup durations. The 1 TB corrected attempt remains a retry, not a clean
first-ever deployment measurement.

## Limitations and remaining work

SnapshotDB uses a 16-vCPU c6i.4xlarge in Mumbai. Ardent assigned managed m6i.xlarge
workers in us-east-1. The laptop has different network paths to those regions.
The 1 TB sync and other CLI comparisons continued during these samples. We did not
equalize worker count, resource contention, region, cost, or caching. Results establish
that the tested SnapshotDB deployment returned usable branches faster; they do not
establish universal product superiority or performance at arbitrary concurrency.

The 20-trial-per-provider CLI batches are complete: all 80 branches passed,
with 566 check assertions across the two sizes. CLI process launch through usable
read/write had the following median / p95 results:

| Dataset | SnapshotDB | Ardent |
|---|---:|---:|
| 116 MB | 1.033 / 1.102 s | 12.293 / 13.976 s |
| 10.77 GB | 1.025 / 1.073 s | 14.763 / 20.075 s |

Their CLI startup, polling, and deletion behavior differs from the matched API test;
do not merge these samples into the API statistics. The 1 TB comparison remains pending Ardent readiness.
Both monitors and desktop notifications remain active.

## Evidence

- [Matched API trials](benchmarks/2026-09-13-api-head-to-head.json)
- [Ardent worker and queue measurements](benchmarks/2026-09-13-ardent-backend.json)
- [SnapshotDB 100 MB server-local trials](benchmarks/2026-09-13-100mb-server.json)
- [SnapshotDB 10 GB server-local trials](benchmarks/2026-09-13-10gb-server.json)
- [Ardent setup timestamps](benchmarks/2026-09-13-ardent-initial-setup.json)
- [SnapshotDB 100 MB setup](benchmarks/2026-09-13-100mb-setup.json)
- [SnapshotDB 10 GB setup](benchmarks/2026-09-13-10gb-setup.json)

The first server-local runner invocation incorrectly required the historical 1 TB
padding table on the smaller fixtures. It produced no timed samples. The corrected
runner explicitly skips that fixture-specific check while retaining measured-size
verification; those failures remain in private logs and are not provider failures.

## Product and security qualification

These are latency and ordinary SQL correctness results, not a security-isolation
guarantee. A subsequent controlled test confirmed that an SnapshotDB branch's returned
admin credentials can read a private canary file in a sibling branch directory.
Ardent's tested returned role was non-superuser and lacked `pg_read_server_files`.
SnapshotDB also returned a successful URL despite a failing branch SQL hook. See the
[full flow audit](product-flow-comparison.md) before interpreting this as product parity.

- [100 MB completed CLI comparison](benchmarks/2026-09-13-100mb-cli-head-to-head.json)
- [10 GB completed CLI comparison](benchmarks/2026-09-13-10gb-cli-head-to-head.json)
