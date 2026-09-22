export const metadata = { title: "FAQ — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Reference</span>
      <h1>FAQ</h1>
      <p className="lead">Common questions about branching, replication, and what snapshotdb does and does not do.</p>

      <h2>How fast is a branch, really?</h2>
      <p>In the retained September 13, 2026 PostgreSQL benchmark, a prepared claim of 1,000,060,870,656 bytes took a median 94.1 ms through a fresh connection, read, committed write, and read-back (three samples). Fresh creation took a median 2.554 s (five samples). Pool preparation and initial replication are excluded from prepared-claim timing. These local-client measurements are not a latency guarantee. <a href="https://github.com/snapshotdb/snapshotdb/blob/main/docs/security-hardening.md">Benchmark method and raw samples</a>.</p>

      <h2>Does a branch get new production changes after it is created?</h2>
      <p>No. A branch is a point-in-time copy of the replica at the moment you branched. After that it is isolated. The <b>replica</b> keeps syncing; run <code>reset</code> to re-clone a branch from the current replica.</p>

      <h2>Do schema changes on production reach branches?</h2>
      <p>Yes for the replica. Postgres replays DDL through an event trigger (or <code>reconcile</code> without superuser); MySQL replicates DDL natively; MongoDB replicates index and collection changes. New branches made after a migration include it.</p>

      <h2>Which engines are supported?</h2>
      <p>Postgres, MySQL, MongoDB, and SQLite. Network-engine branches require Linux with the filesystem sandbox; SQLite is also supported on macOS/APFS.</p>

      <h2>Is my data sent anywhere?</h2>
      <p>In hosted mode, database copies live on SnapshotDB infrastructure. In BYOC mode, the replica and branches live on your server; connection settings are available only after selecting BYOC.</p>

      <h2>What does the hosted plan include?</h2>
      <p>Free includes one source database and two total trial branch-hours. Pro is $150/month for unlimited source database count and 300 branch-hours per paid billing month, with 50 GiB shared storage and 50 GiB outbound data. Running source replicas count too. Idle copies keep their data; resuming needs remaining allowance. BYOC does not use hosted allowances.</p>

      <h2>What are the known limits?</h2>
      <ul>
        <li>Postgres schema replay can pause on unsupported DDL or missing prerequisites. Inspect status and repair the replica before branching.</li>
        <li>Linux requires reflink-capable storage and refuses unsupported clones. macOS needs APFS for copy-on-write.</li>
        <li>Branch URLs are on the server’s <code>--public-host</code>; there is no managed routing layer.</li>
      </ul>

      <div className="np"><a href="/docs/api">← REST API</a><a className="n" href="/docs/workflows/agents">AI agents →</a></div>
    </>
  );
}
