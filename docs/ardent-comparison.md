# Anybranch and Ardent comparison — 13 September 2026

**Update:** [Completed 100 MB and 10 GB head-to-head results](head-to-head-results.md) are now available. The setup-pending statements below describe earlier observations.

**A direct latency winner is not established.** Anybranch completed 20 fresh 1 TB
branch trials from the laptop. Ardent's earlier setup attempts were blocked by a
source PostgreSQL plugin configuration issue, subsequently fixed. Its corrected
setup is running, with an active replication slot and source data streaming observed.
No successful Ardent branch latency sample has been obtained yet.

| PostgreSQL test | Trials | Median | p95 | Maximum |
|---|---:|---:|---:|---:|
| Anybranch, fresh 1 TB, laptop to Mumbai | 20 | 1.413 s | 1.801 s | 1.830 s |
| Anybranch, fresh 1 TB, EC2 client | 20 | 0.997 s | 1.219 s | 1.302 s |
| Anybranch, prepared 1 TB, EC2 client | 4 concurrent | 0.085 s | 0.105 s | 0.105 s |
| Ardent, same source, laptop client | 0 successful | unavailable | unavailable | unavailable |

Ardent's [published claim](https://www.tryardent.com/) is database copies in under
six seconds, including at terabyte scale. Its [quickstart](https://docs.tryardent.com/quickstart)
separates the initial data copy from subsequent branch creation and connection.
Every measured Anybranch fresh branch was below six seconds. This meets that threshold;
it does not prove Anybranch is faster than Ardent, whose successful timings remain unknown.

## Measurement boundary and comparability

The original Anybranch initial-copy phase took **10,161.477 seconds (2 h 49 m 21 s)**,
excluding synthetic source generation. This is recorded by the core scale benchmark;
it is separate from the optimized fresh-branch measurements below. No successful
Ardent initial setup duration has been measured yet. Source-to-target placement differs:
Anybranch's source and target are on the Mumbai host, whereas Ardent's managed target
is in us-east-1. Initial-copy times would therefore include different network paths.

For the 20 successful laptop trials, CLI launch through URL output alone measured
**1.136 s median, 1.403 s p95, and 1.545 s maximum**. Through the first committed
read/write, the corresponding numbers were **1.413 s, 1.801 s, and 1.830 s**.

The fresh tests begin with a synced replica, create a new branch name without a
prepared pool, and time CLI startup through URL retrieval, a new authenticated database
connection, a source-marker read, a large-table payload read, a committed write, and
read-back. The laptop tests include setting up forwarding for each new branch URL.
Source and replica isolation and the presence of all eight large tables are checked
separately. Initial source generation and synchronization are excluded.

Before every request, the runner writes a new source nonce and waits for Anybranch
replication, then allows two seconds of catch-up grace. The returned branch must contain
that exact nonce. Ardent's internal replica is not directly queryable, so a future run
must report stale-marker failures and cannot silently discard them. Prepared-snapshot
allocation is a separate test and does not establish fresh-branch performance.

Anybranch runs on a c6i.4xlarge (16 vCPU, 32 GiB) in Mumbai with Btrfs on a 3,072 GiB
gp3 volume provisioned at 6,000 IOPS and 500 MiB/s. Ardent assigned its managed environment
in us-east-1, with m6i.xlarge workers. Both CLI commands run on the same laptop, but
regions, hardware, scheduling, and routing differ. This is a deployment experience
comparison, not an identical-hardware engine benchmark. Anybranch uses SSH forwarding;
Ardent would use its returned native TLS connection string. No local database copy is made.

## Actual Ardent attempts

The official `ardent-cli@0.0.104` used the existing local signed-in session. The connector
`anybranch-1tb-comparison` targets the same synthetic 1 TB source, exposed over PostgreSQL
TLS with client authentication and AWS ingress restricted to Ardent's reported egress IP.
Source reachability, authentication, grants, and schema discovery completed.

Initial setup failed at `deploying-pgstream:postgres` after approximately 10 minutes.
The official `connector retry-setup` command also failed at that stage, from
12:46:59 to 12:57:15 UTC. Both operation records report:

> Engine setup failed unexpectedly. Contact Ardent support.

Both a direct `branch create` invocation and the comparison runner's CLI invocation
received HTTP 422 because the branching engine was still reported as setup pending.
The connector status display continued to show 45% and unavailable copy metrics even
after its operation failed; the operation record is retained as the terminal evidence.
A small database diagnostic on the same host was rejected as a duplicate source, so
it provides no evidence about size dependence.

**Subsequent diagnosis corrected the attribution:** at about 13:08 UTC, source logs
showed repeated refusals of `wal2json` during all three setup attempts, including the
user-started operation `op_c2f938c2-d62b-43cf-8606-9d22db5bdc22` at 12:58:42 UTC.
The plugin package was installed, but PostgreSQL 16.15's `output_plugin_libraries`
setting still allowed only `pgoutput, test_decoding`. The source was updated to
`pgoutput, test_decoding, wal2json`, reloaded without a restart, and a real temporary
wal2json replication slot was successfully created and automatically removed.
This is a source prerequisite failure; earlier generic Ardent errors must not be
interpreted as evidence of an Ardent infrastructure defect or poor copy performance.
The setting is documented in the [PostgreSQL replication configuration reference](https://www.postgresql.org/docs/16/runtime-config-replication.html#GUC-OUTPUT-PLUGIN-LIBRARIES).

The user-started attempt ultimately failed at 13:08:58 UTC. After the source fix,
the official CLI started operation `op_7e179294-c7a6-4a1d-8c14-f73ce3bc777a` at
**13:09:29 UTC (18:39:29 IST)**. The source then showed an active Ardent wal2json
slot and 32 sessions streaming data. Ardent currently labels the stage as validation
or schema discovery, without byte-copy metrics; those percentages are not data-copy
percentages and cannot be used to calculate an ETA.

A local monitor records setup state every 30 seconds and will run 20 new trials
per provider after Ardent reports ready. It records both URL-output time and committed
read/write time. The updated runner checks at least 1 TB of table/index storage in
every branch, as well as data and isolation. Two real Anybranch trials validated
the updated runner, each measuring 1,000,060,870,656 branch table/index bytes.
Progress is in `.local/aws-mumbai/ardent/comparison-progress.json`; the future result
is `.local/aws-mumbai/ardent/head-to-head.json`. These are ongoing work, not completed
comparison evidence. The monitor uses the laptop's existing CLI authentication and
requires the laptop to remain running and connected. Its corrected-attempt setup time
will exclude previous failed attempts and must be labeled as a retry, not a clean
first-ever deployment.

The source, 1 TB disk, retained Anybranch test branches, and Ardent connector are kept
for further testing. Ardent setup must succeed before the 20-trial head-to-head run can
complete. Credentials and full connector details remain in ignored private local files.

## Reproducible evidence

- [Fresh laptop timings and isolation checks](benchmarks/2026-09-13-fresh-1tb-laptop.json)
- [Fresh EC2 timings and checks](benchmarks/2026-09-13-fresh-1tb.json)
- [Prepared 1 TB timings and checks](benchmarks/2026-09-13-prepared-1tb.json)
- [Sanitized Ardent failures and operation IDs](benchmarks/2026-09-13-ardent-attempt.json)
- [Same-client CLI comparison runner](../scripts/compare_ardent.py)

The published median is the conventional median (mean of the middle two for even
sample counts); p95 uses nearest rank. No successful trials were omitted. An initial
runner launch inherited the remote PATH and could not locate local Node; it was fixed
before the recorded Ardent CLI attempt and is not counted as an Ardent service failure.
