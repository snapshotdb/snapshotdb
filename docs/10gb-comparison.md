# 10 GB head-to-head benchmark — in progress

**Update:** [Completed 100 MB and 10 GB head-to-head results](head-to-head-results.md) are now available. The setup-pending statements below describe earlier observations.

A separate PostgreSQL source on the Mumbai EC2 host contains **10,769,342,464 bytes**
of tables/indexes, with eight tables of 650,000 rows each. PLAIN payload storage
prevents TOAST compression. Generation took 72.16 seconds and is excluded from copy
timing. SnapshotDB's initial copy completed in **87.644 seconds**; all eight row counts
match, and the replica measures **10,770,161,664 bytes**.

Ardent connector `snapshotdb-10gb-comparison`
(`connector_2c6bf5ee-b512-4734-b572-f41fbe6e7b5d`) targets this separate source on
port 15451. TLS requires a client certificate; AWS ingress allows only Ardent's
reported egress IP. A real temporary wal2json slot was verified before setup.
Ardent accepted the connector and began initial setup. Successful Ardent copy and
branch timings are pending.

The local monitor will run 20 fresh branches per provider once both copies are ready.
It times CLI launch through URL output, then through a new authenticated connection,
source-marker and payload reads, committed write, and read-back. It checks source
isolation, all eight tables, and at least 10 GB of table/index storage per branch.
Connector selection and the shared CLI configuration lock are outside the timer.
The lock prevents the two benchmark runners from selecting each other's connectors.

Both 10 GB systems are tested while the 1 TB Ardent sync continues. CPU, disk, and
network contention may affect results. SnapshotDB's source and target are in Mumbai;
Ardent's managed target is in Virginia. This compares the deployed experiences, not
identical hardware and network placement. No prepared branch pool is used.

The source and SnapshotDB replica live at `/srv/snapshotdb-tb/comparison-10gb` and are
managed by `snapshotdb-10gb.service`. Server setup evidence is at
`/srv/snapshotdb-data/comparison-10gb/setup-report.json`. Private local progress and
eventual results are in `.local/aws-mumbai/ardent-10gb/`. The 20-minute Mac notification
job includes this run; its monitor also attempts a notification at completion.

The first 20 SnapshotDB fresh-branch trials all passed. Committed read/write latency
was **1.039 s median, 1.499 s p95, and 5.429 s maximum**. The slow first sample
is included. These are SnapshotDB-only results while Ardent setup is pending; they
do not establish a head-to-head winner. See [raw trials](benchmarks/2026-09-13-10gb-snapshotdb.json).
