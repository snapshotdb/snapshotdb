# Committed updates → agent branch: Mumbai diagnostic

This diagnostic does **not** establish a general performance winner. It exposes a freshness gap that frozen-snapshot branching measurements do not cover. One 100 MB and one 1 GB transaction were applied to the same 10.77 GB PostgreSQL source. Both APIs were called concurrently from Mumbai. Anybranch ran in Mumbai; Ardent's managed data plane remained in Virginia. Source execution time is excluded from the post-commit timer.

| Updated payload | Anybranch: commit → verified agent read/write | Ardent: commit → verified agent read/write |
| --- | --- | --- |
| 100,000,800 bytes | 78.17 s; one stale branch before success | 78.40 s; two stale branches before success |
| 1,000,008,000 bytes | Failed: ten stale branches; trial ended at 692.39 s | 589.26 s; eighth branch succeeded |

The first Anybranch URL after the 100 MB commit arrived in 1.57 s but contained none of the updates. Ardent's first URL at 8.36 s was also stale. The first subsequently verified current URLs were observed at 42.70 s and 58.86 s respectively. These are observed workflow timings, not minimum replication lag: expensive full-row checks delayed the next request.

Every updated row in all eight tables was checked against its expected payload. Successful candidates also passed an agent write, readback through a fresh connection, non-superuser check, and source isolation. Full-data validation happens inside the reported timer in this diagnostic; it took 35.41 s on the successful 100 MB Anybranch candidate, 11.52 s on Ardent, and 68.12 s on the successful 1 GB Ardent candidate. The proposed faster-probe harness was not executed and has no reported results.

Anybranch's create_cold path stops and restarts the parent. Logs confirm replication-worker termination at these stops. Its readiness check verifies initial-copy completion and an enabled subscription, but does not enforce a source-commit watermark. A fast returned URL therefore does not establish read-your-writes freshness. Repeated branching can interrupt an in-flight apply transaction; further controlled testing is required to quantify that contribution.

The Mumbai host also reached roughly 500 MiB/s aggregate disk traffic and 39–45% I/O wait during the 1 GB test. Existing subscribers shared the source and disk. Compute, storage, and transport were not matched; no claim of strict infrastructure parity or intrinsic engine latency follows. Ardent achieved the required data freshness in this run while Anybranch did not.

All temporary Anybranch branches were deleted. Ardent initially reported two cleanup HTTP errors; both deletions succeeded on reconciliation. All temporary branches from this diagnostic have been deleted. Source updates and test databases are retained. See the adjacent JSON for every attempt, verification counts, timings, and the tested Anybranch binary hash. No latest-write guarantee has been implemented by this diagnostic.
