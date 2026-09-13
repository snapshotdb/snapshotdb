# PostgreSQL branch freshness

A new branch from a continuously synced PostgreSQL root waits for source changes committed before its freshness marker. Initial table-copy completion alone is not a freshness guarantee.

During sync, Anybranch creates a one-row `_freshness_v1` table in that root's existing `anybranch_<root>` bookkeeping schema and includes it in the publication. Before creating a branch, resetting from a synced root, or preparing a snapshot, it updates a random token on the source and waits until the replica has applied it. Only then may it stop the parent and clone its files. This adds one small source write per freshness check; it does not change application tables. The connector role must retain access to its bookkeeping schema/table. Older replicas install this table lazily and wait for its initial table copy before writing the token.

Subscription publication refreshes now run only when table membership differs. Repeated branch/status requests no longer refresh an unchanged subscription, avoiding unnecessary apply-worker restarts and lock waits.

Set `ANYBRANCH_FRESHNESS_TIMEOUT_SECONDS` on the deployed server to an integer from 1 to 3600 (default 600). If the replica cannot apply the token, the operation returns an error before allocating branch storage or stopping the parent. Individual SQL/network operations also have their own timeout, so this is not a strict whole-request deadline. A disabled subscription, initial copy, or recorded DDL error is refused rather than represented as a fresh branch.

Prepared snapshot claims still return the explicitly prepared snapshot; they do not include later source changes. Reusing an existing branch name remains idempotent and returns that branch, without updating its contents. Use a new name for a new source snapshot. Writes committed after the marker are not guaranteed to appear.

This change applies to PostgreSQL. It does not establish equivalent latest-write guarantees for MySQL or MongoDB. Large updates still need time to transmit and apply; the change prevents returning an old snapshot as though it satisfied freshness. It does not promise subsecond replication of a 1 GB transaction.

Regression coverage deliberately blocks the PostgreSQL apply worker while committing a source update, then checks that a fresh branch contains it. A second blocked-apply test verifies that a timeout creates no branch directory and does not restart the parent.

## Mumbai verification, 14 September 2026

The fixed binary passed 8 Rust tests and 127 Linux end-to-end checks across PostgreSQL, MySQL, MongoDB, and SQLite. The new freshness regressions are PostgreSQL-specific.

On the retained PostgreSQL dataset, each update was followed by exactly one fresh-branch request:

| Updated payload | Commit → URL | Commit → agent read/write | Commit → full verification complete |
| --- | --- | --- | --- |
| 100 MB / 58,824 rows | 3.70 s | 3.80 s | 5.29 s |
| 1 GB / 588,240 rows | 34.38 s | 34.55 s | 134.16 s |

Both returned branches contained every updated row across all eight tables. Timed agent read/write includes indexed probes and a committed write read through a fresh connection; a candidate counts only after the subsequent full-row verification passes. All temporary branches were deleted and the source data retained. The source UPDATE transactions themselves took approximately 3.97 s and 105.29 s respectively, **before** the post-commit timers. See the JSON for exact values.

These are one-off fix-verification runs on the shared Mumbai host, not a fresh Ardent head-to-head or a percentile benchmark. The earlier diagnostic put expensive full-row verification before the agent write and between retries; its end-to-end numbers cannot be used directly to calculate a speedup against this harness. The important regression result is that the 1 GB case now returned a correct branch on its first request, where the earlier run returned ten stale branches.

[Raw fixed-run evidence](benchmarks/2026-09-14-postgres-freshness-fixed.json) · [Earlier diagnostic](benchmarks/2026-09-14-mumbai-update-diagnostic.md)
