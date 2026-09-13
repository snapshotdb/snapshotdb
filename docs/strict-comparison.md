# Strict comparison protocol

Status: **not completed**. Historical results in `head-to-head-results.md` do not
establish an infrastructure-controlled win. The security changes must be deployed
on the measured Anybranch server, not only the client.

Two different questions require separate reports:

1. **Customer experience:** same client, source, dataset, freshness requirement,
   encrypted connection requirement, and test workload. Compare the services as
   delivered, including API queueing, routing and connection establishment.
2. **Infrastructure efficiency:** additionally control region/AZ, instance count
   and type, CPU/memory limits, storage type/IOPS/throughput, database version,
   durability settings, competing workloads and provisioned capacity. Managed
   Ardent internals are currently unverified. Unknown values fail this comparison
   gate; choosing the same nominal instance class alone does not satisfy it.

Ardent documents a [BYOC data plane](https://docs.tryardent.com/architecture).
Use it if the account exposes deployment controls; do not infer resource settings
from branch latency or advertise a hardware-controlled result without evidence.

## Required runs

Use the same immutable source content for 100 MB, 10 GB and 1 TB runs. Record actual
logical table bytes, row counts, schema manifest and deterministic sampled content
hashes. Storage allocation is a separate metric: identical logical content can
have different physical sizes. Keep the original source out of branch benchmarks.

| Run | Clock starts | Clock stops |
| --- | --- | --- |
| Full onboarding | First connector/sync request, prerequisites already satisfied | First branch passes content and committed-write probes |
| Initial replication | Setup request submitted | Copy complete and a source marker is demonstrably present |
| Fresh branch | First create request byte sent | URL delivered; separately first connection, content read, committed write/read-back |
| Prepared branch | First prepared claim request | Same readiness boundaries as fresh; preparation cost and pool size reported separately |
| Resume | First connection to suspended branch | Persisted data read and new write committed |
| Reset | Reset request | Original data restored and write probe succeeds |
| Delete | Delete request | Operation complete and both new and existing sessions cannot use the branch |

Ardent setup UI percentages and worker-stage durations are diagnostic information,
not substitutes for these end-to-end clocks. A pending 1 TB setup is censored at its
observation time, never converted into a completed latency or speedup.

For latency, run at least 30 trials per provider, size and mode, alternating the
provider order with a recorded random seed. Use the same polling interval and
timeouts. Report every failure, timeout, cleanup failure and retry separately;
never remove unsuccessful attempts from the denominator. Report p50, p95, p99,
minimum, maximum and sample count. With 30 trials, p99 is effectively the maximum.

Run concurrency 1, 4 and 16 where account quotas permit. Mark a quota refusal as
such. Measure CPU, memory, disk throughput/IOPS, network and source replication
lag during each run. Test quiet conditions separately from simultaneous ingest.
Report the client-observed clock as the primary result; server traces are a
separate explanation and must use equivalent start/end events on both systems.

## Correctness gates

- Returned URL authenticates with an agent role; TLS/private tunnel is verified.
- Sampled source content and schema match, including indexes, constraints,
  sequences, defaults, functions, extensions, permissions and RLS behavior.
- INSERT/UPDATE/DELETE, explicit transactions, rollback and DDL work as expected.
- Writes are isolated from source, replica and another simultaneously live branch.
- A new connection sees committed data; suspended/resumed branches retain it.
- Latest-write tests verify the exact source marker. Frozen-snapshot tests verify
  the agreed snapshot marker. These are separate modes.
- Failed masking hooks never publish usable URLs. Schema/replication errors are
  visible and recovery neither loses nor duplicates committed source events.
- Branch credentials cannot administer other branches, read host secrets or act
  as the source administrator. Compare privilege semantics, not only role names.
- CLI and API flows both work. Website flows require a deployed website URL and
  browser verification; this repository currently contains no website application.

If a feature is unavailable on either product, show **unsupported/unverified**.
An Anybranch-only test cannot establish comparative parity. PostgreSQL results
cannot establish MongoDB or MySQL performance against a PostgreSQL-only endpoint.

## Publication gate

Attach commit and binary hashes, sanitized raw samples, configuration evidence,
source manifest, failure counts, test commands and observed limitations. Report
latency wins only for the exact passing mode and size. Never compare prepared
Anybranch capacity with an unprepared Ardent branch under one headline.
