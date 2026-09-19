export const metadata = { title: "AWS RDS — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>AWS RDS</h1>
      <p className="lead">RDS and Aurora Postgres or MySQL, connected over a network path you control.</p>

      <h2>Postgres on RDS</h2>
      <ul>
        <li>Set <code>rds.logical_replication = 1</code> in the parameter group and reboot the instance.</li>
        <li>Grant the connecting role <code>rds_replication</code> — preflight checks this as <code>can replicate</code>.</li>
        <li>The schema-tracking event trigger needs superuser; on RDS that is <code>rds_superuser</code>. Without it, use <code>reconcile</code> after migrations.</li>
        <li>The roles dump (<code>pg_dumpall --roles-only</code>) is not permitted on RDS and is skipped automatically.</li>
      </ul>

      <h2>MySQL on RDS</h2>
      <ul>
        <li>Set <code>gtid-mode = ON</code>, <code>enforce_gtid_consistency = ON</code>, and <code>binlog_format = ROW</code> in the parameter group, then reboot.</li>
        <li>Enable automated backups so binary logging is on.</li>
        <li>Grant the user <code>REPLICATION SLAVE</code> plus read access.</li>
      </ul>

      <h2>Network</h2>
      <p>RDS instances are usually not publicly reachable. Run the snapshotdb server where it can reach the instance (same VPC, or via a bastion / tunnel), and point <code>preflight</code>/<code>clone</code> at the instance endpoint. Preflight is read-only, so it is safe to run first to confirm the path and settings.</p>

      <div className="note"><b>Changing a parameter group requires a reboot,</b> which is disruptive on a production instance. Do it in a maintenance window, or connect a read-restricted replica the same way.</div>

      <div className="np"><a href="/docs/connectors/postgres">← Self-hosted Postgres</a><a className="n" href="/docs/connectors/supabase">Supabase →</a></div>
    </>
  );
}
