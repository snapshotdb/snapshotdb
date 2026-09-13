export const metadata = { title: "Self-hosted Postgres — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>Self-hosted Postgres</h1>
      <p className="lead">Postgres 13+ with logical replication. anybranch reads the schema and streams changes; your application tables are never written to.</p>

      <h2>Requirements</h2>
      <ul>
        <li><code>wal_level = logical</code> (needs a restart to change).</li>
        <li>A free replication slot and WAL sender.</li>
        <li>A role with the <code>REPLICATION</code> attribute that can read the tables and owns them — a publication needs the table owner.</li>
        <li>A PRIMARY KEY on each replicated table (or <code>--fix-replica-identity</code> to set <code>REPLICA IDENTITY FULL</code>).</li>
      </ul>
      <pre className="code"><code><span className="p">$</span> anybranch preflight postgres &apos;postgresql://user:pass@host:5432/app&apos; --schemas public</code></pre>
      <p>Preflight prints a grant script when reads or ownership are missing. Run it on the source, then re-run preflight until it passes.</p>

      <h2>What anybranch creates on the source</h2>
      <p>All named <code>anybranch_&lt;name&gt;</code>: a publication, a replication slot, a schema holding a <code>ddl</code> log table, and an event trigger that records each DDL statement. The DDL table is itself replicated and replayed on the replica, so migrations on production appear on branches, and new keyed tables join replication automatically. The event trigger swallows its own errors, so it can never fail your DDL. <code>rm</code> removes everything it created.</p>
      <div className="note"><b>No superuser?</b> The event trigger needs superuser to create. Without it, rows still replicate and <code>status</code> says schema changes are not tracked; run <code>anybranch reconcile &lt;name&gt;</code> after a migration to add the new columns and tables.</div>

      <h2>Sync</h2>
      <pre className="code"><code><span className="p">$</span> anybranch clone prod &apos;postgresql://user:pass@host:5432/app&apos;{"\n"}<span className="p">$</span> anybranch status prod   <span className="c"># initial-copy progress, WAL lag, schema tracking</span></code></pre>

      <h2>Client TLS</h2>
      <p>Pass TLS options in the connection string itself, e.g. <code>?sslmode=verify-full&amp;sslrootcert=…&amp;sslcert=…&amp;sslkey=…</code>.</p>

      <div className="np"><a href="/docs/connectors">← Connectors</a><a className="n" href="/docs/connectors/rds">AWS RDS →</a></div>
    </>
  );
}
