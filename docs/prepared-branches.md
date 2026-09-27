# Prepared agent branches

After the initial server-side database sync, prepare an immutable snapshot and a pool
of running branches. An agent claim assigns one of those private databases and returns
its usable URL without starting an engine or copying database files on the request path.

```sh
snapshotdb clone prod 'postgresql://user:password@source.internal/app'
snapshotdb status prod
# Wait for initial replication to finish, then prepare capacity before allocating agents.
snapshotdb prepare release-1 --from prod --count 4
snapshotdb create agent-1 --from release-1 --print-url
snapshotdb create agent-2 --from release-1 --print-url
```

The same `prepare` and `create` commands work with PostgreSQL, MySQL, MongoDB, and SQLite
roots. Each branch has separate credentials and writable copy-on-write storage on the
server. The client only submits commands and connects to the resulting endpoint.

Preparation is explicit and may take seconds or longer depending on data size and engine.
`--count` sets the desired number of **available** branches, from 1 to 32. Calling
`prepare release-1 --from prod --count 4` again fills that snapshot's remaining capacity.
It does not refresh its data. To include newer source changes, prepare a new snapshot name.
The preparation response records the snapshot's creation time and available count.

The snapshot contains the already-synced replica's data at preparation time, not a promise
that replication had caught up to every source write. Branch initialization hooks run while
preparing the snapshot and are not reapplied to each prepared child. A snapshot cannot be
started or reset into a writable database. Removing one is refused while agent branches
reference it. Remove agents first; deleting the snapshot then also removes unused capacity.
Prepared children cannot be reset either. Claim a new branch from available capacity before
removing the old one; prepare a new snapshot if you need newer source data.

Claims are serialized and consume distinct slots. Retrying the same name and parent returns
the same branch and URL. An empty pool or dead ready engine returns an error without falling
back to a slow clone. Refill the pool to repair capacity. Ready engines stay awake, and server
startup restores them before accepting commands. Claimed branches follow the ordinary idle
policy, with a full idle grace period after allocation. Cold resume is a separate operation.

The latency target applies to healthy, prepared capacity. Preparation and other engine jobs
currently share the command queue with claims; schedule preparation ahead of the agent burst.
Network round trips, resource contention, pool size, and database driver handshakes affect
observed latency. Finite warm capacity consumes memory and some additional disk blocks.
Ordinary `create --from prod` continues to make a fresh cold branch.

## SQLite requests

SQLite files stay on the server. Its returned URL has this shape:
`http://branches.internal:PORT/v1/query?token=BRANCH_SECRET`. Treat the whole URL as a secret
and use a private network or encrypted tunnel, as with the native database ports.

```python
import json
import urllib.request

body = {"statements": [
    {"sql": "INSERT INTO notes(id, body) VALUES(?, ?)", "params": [1, "hello"]},
    {"sql": "SELECT * FROM notes WHERE id=?", "params": [1]},
]}
request = urllib.request.Request(branch_url, json.dumps(body).encode(),
                                 {"Content-Type": "application/json"})
with urllib.request.urlopen(request) as response:
    print(json.load(response))
```

Each request commits all its statements atomically; an error rolls back the entire request.
Each result contains `columns` and `rows`; BLOB values are returned as `{"bytes": [...]}`.
Parameters accept JSON scalars. Cross-request transactions are not supported. Requests are
limited to 64 KiB and 100 statements, results to 10,000 rows per statement and 4 MB overall,
individual SQLite values to 1 MB, and query execution to approximately two seconds.
`ATTACH`, `DETACH`, transaction-control SQL, and pragmas are refused. The server coordinates
query transactions and filesystem cloning so an HTTP write cannot race a SQLite snapshot.

## Reproduce the test

On an isolated deployed server with all engine binaries and Python native drivers:

```sh
pip install 'psycopg[binary]' 'PyMySQL[rsa]' pymongo
python scripts/latency.py --binary /path/to/snapshotdb --report /private/latency.json
```

The runner creates disposable 1,000-row sources, syncs each supported network engine,
measures cold branches, prepares pools, and runs three bursts of four concurrent claims
per engine. Its timer includes CLI launch, URL retrieval, a new authenticated connection,
seeded-data read, committed update, and read-back. Preparation is reported separately.
It checks unique endpoints, sibling/source isolation, snapshot immutability, idempotent
claims, pool exhaustion, protected deletion, and absence of client-side database files.
This small-fixture test does not establish 1 TB preparation or claim latency.
