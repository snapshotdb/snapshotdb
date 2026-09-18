# End-to-end test results

The later retained 1 TB run passed 45 checks, followed by eight Rust tests, seven
Python tests, and 94 engine E2E checks. See [1 TB results](latency-results.md) and
[Ardent comparison status](ardent-comparison.md). The September 12 run below is
preserved as the earlier small-server validation.

Completed 2026-09-12T22:16:51+00:00 against EC2 `i-0c0d826d940910b44` in Mumbai, AWS account `686255969284`.
The deployed source and release binary matched the checked-out source/build.

| Test group | Result |
| --- | --- |
| Rust unit/integration tests, including SQLite | 8 passed |
| Python benchmark tests | 5 passed |
| PostgreSQL, MySQL, MongoDB engine end-to-end suite | 94 passed |
| Fresh laptop → SSH tunnel → deployed API → PostgreSQL workflow | 35 passed |
| 1 GB PostgreSQL scale benchmark | 28 passed |

The laptop workflow created a new source with 10,000 rows, submitted an asynchronous
clone job, compared all replicated values, replicated a schema change, and created two
isolated branches. It verified insert/update/delete isolation, later source updates,
suspend/resume, deletion locks, safe reset refusal while replication was paused, and a
successful reset. It restarted the actual systemd service, verified persisted data and
continued replication, then removed the test databases and confirmed replication-slot
cleanup. No client database directory was created. Existing demo databases remain.

The scale benchmark measured **1,001,914,368 bytes** of table/index data.
Initial replication took 40.5 seconds; the full
benchmark took 106.3 seconds. Both isolated branches, live
updates, schema changes, resume, and cleanup passed. These are single-run measurements.
**This earlier run did not test 1 TB.**

The first laptop test attempt stopped because the test script used a hyphenated
subscription name instead of SnapshotDB's underscore-normalized name. Its test databases
were cleaned up; after correcting the script, the complete fresh run passed. No product
code changes were needed during this test run.

The service remains active. Temporary test databases and engine-suite directories were
removed; reports and job metadata remain. Private logs and detailed JSON reports are in
`.local/aws-mumbai/`: `full-e2e-latest.log`, `fresh-e2e-report.json`, and
`benchmark-latest-report.json`. Logs may contain synthetic test credentials and should
not be published. The remote scale report is `/srv/snapshotdb-data/bench/scale-i37w2vuo/report.json`.
