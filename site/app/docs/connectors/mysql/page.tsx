export const metadata = { title: "MySQL — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>MySQL</h1>
      <p className="lead">MySQL 8.0+ with GTID replication. Nothing is created on the source — snapshotdb dumps once, then follows the binlog.</p>

      <h2>Requirements</h2>
      <ul>
        <li><code>gtid_mode = ON</code> and <code>enforce_gtid_consistency = ON</code>.</li>
        <li>Binary logging on, with <code>binlog_format = ROW</code>.</li>
        <li>A user with <code>REPLICATION SLAVE</code> plus read access.</li>
      </ul>
      <pre className="code"><code><span className="p">$</span> snapshotdb preflight mysql &apos;mysql://user:pass@host:3306/&apos;</code></pre>

      <h2>How sync works</h2>
      <p>User databases are dumped once with <code>mysqldump --single-transaction --set-gtid-purged=ON</code>, then the replica follows the source’s binlog by GTID auto-position. System schemas (<code>mysql</code>, <code>sys</code>, <code>information_schema</code>, <code>performance_schema</code>) are never mirrored. DDL replicates natively through the binlog, so migrations on production appear on branches with no extra setup.</p>

      <h2>When replication breaks</h2>
      <p>A transaction the replica cannot apply — for example a row you inserted on the replica that production later inserts too — pauses the SQL thread instead of retrying forever. <code>status</code> shows the error; <code>repair</code> skips that one transaction with an empty commit under its GTID and resumes.</p>

      <div className="np"><a href="/docs/connectors/supabase">← Supabase</a><a className="n" href="/docs/connectors/mongodb">MongoDB →</a></div>
    </>
  );
}
