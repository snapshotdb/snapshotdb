export const metadata = { title: "Connect a database — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>Connect a database</h1>
      <p className="lead">A source is a production database snapshotdb keeps a live replica of. Run <code>preflight</code>, then <code>clone</code>/<code>sync</code>. Both are branched the same way.</p>

      <h2>Two ways to make a root</h2>
      <ul>
        <li><b><code>sync</code> / <code>clone</code></b> — a root that continuously replicates from a live database, using the engine’s own replication.</li>
        <li><b><code>import</code></b> — a root from a stopped data directory, a SQLite file, or <code>--new</code> (an empty database).</li>
      </ul>

      <h2>Preflight first</h2>
      <p>Preflight is read-only. It prints a pass / warn / fail checklist and, when grants are the problem, a ready-to-run grant script. It creates nothing and stores no credentials. <code>sync</code> runs the same checks and refuses on any failure.</p>

      <h2>Replica identity</h2>
      <p>Logical replication needs a way to identify rows for UPDATE and DELETE. Tables without a PRIMARY KEY are skipped, and the exact <code>ALTER TABLE … REPLICA IDENTITY FULL</code> to include them is printed. Pass <code>--fix-replica-identity</code> to apply those on the source for you.</p>

      <h2>Engines</h2>
      <table>
        <thead><tr><th>Engine</th><th>Sync mechanism</th><th>Schema changes</th></tr></thead>
        <tbody>
          <tr><td><a href="/docs/connectors/postgres">Postgres</a></td><td>logical replication</td><td>event-trigger replay, or <code>reconcile</code></td></tr>
          <tr><td><a href="/docs/connectors/mysql">MySQL</a></td><td>GTID replication</td><td>native (binlog)</td></tr>
          <tr><td><a href="/docs/connectors/mongodb">MongoDB</a></td><td>dump + change streams</td><td>indexes, collections, collMod</td></tr>
          <tr><td>SQLite</td><td>n/a (file import)</td><td>n/a</td></tr>
        </tbody>
      </table>
      <p>Hosted guides: <a href="/docs/connectors/rds">AWS RDS</a> · <a href="/docs/connectors/supabase">Supabase</a>.</p>

      <div className="np"><a href="/docs/server">← Server deployment</a><a className="n" href="/docs/connectors/postgres">Self-hosted Postgres →</a></div>
    </>
  );
}
