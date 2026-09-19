export const metadata = { title: "Architecture — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Concepts</span>
      <h1>Architecture</h1>
      <p className="lead">A control API, a copy-on-write clone per branch, and one engine process per branch — all on a server you run.</p>

      <h2>Client and server</h2>
      <p>The CLI is a thin client. It submits a command to the server as an authenticated job over HTTP, waits, and prints the result. Database files and engine processes live entirely on the server. Without <code>SNAPSHOTDB_SERVER</code> the client fails; it never falls back to a local copy.</p>

      <h2>On-disk layout</h2>
      <p>Everything lives under <code>SNAPSHOTDB_HOME/&lt;name&gt;/</code> on the server:</p>
      <pre className="code"><code>engine       postgres | mysql | sqlite | mongodb{"\n"}parent       optional — the branch this was cloned from{"\n"}source       optional production URL (mode 0600) — makes this a synced root{"\n"}data/        the engine data directory — this is what gets cloned{"\n"}run/         ports, pids, sockets, logs, tailer state — never cloned</code></pre>

      <h2>Copy-on-write branching</h2>
      <p>A branch is a clone of the parent’s <code>data/</code> directory made with the platform’s file-clone primitive — <code>clonefile</code> on macOS/APFS, reflink on Linux (Btrfs, XFS, bcachefs). No bytes are copied until a page changes, so branch time and initial disk are independent of database size. Branching a running parent holds it, stops it, clones, and restarts it, because a per-file clone is only a consistent snapshot while nothing is writing.</p>

      <h2>Replica and branches</h2>
      <p>A synced root replicates from production using the engine’s own replication (Postgres logical replication, MySQL GTID, MongoDB change streams). Branches are always cut from the replica, never from production, so branch activity never touches your primary. A fresh branch inherits the parent’s replication config and is detached on first start — the subscription is removed and, for Postgres, sequences are advanced past the copied rows so inserts do not collide.</p>

      <h2>Ports and suspend</h2>
      <p>Every branch has a stable public port owned by a small in-process proxy, plus an engine port behind it. Idle branches stop their engine after <code>SNAPSHOTDB_IDLE_MINUTES</code> (default 5) and resume on the next connection — the URL never changes. Synced roots never suspend, because they must keep replicating.</p>

      <div className="np"><a href="/docs/connectors/mongodb">← MongoDB</a><a className="n" href="/docs/security">Security →</a></div>
    </>
  );
}
