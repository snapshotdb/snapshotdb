export const metadata = { title: "MongoDB — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>MongoDB</h1>
      <p className="lead">MongoDB 6.0+ replica sets. snapshotdb dumps once, then applies change-stream events. Nothing is created on the source.</p>

      <h2>Requirements</h2>
      <ul>
        <li>A replica set — change streams need one (Atlas always is; a single-node <code>rs.initiate()</code> is fine).</li>
        <li>A user that can read all databases and open a change stream (Atlas: <code>readAnyDatabase</code>).</li>
      </ul>
      <pre className="code"><code><span className="p">$</span> snapshotdb preflight mongodb &apos;mongodb://user:pass@host:27017/?replicaSet=rs0&apos;</code></pre>
      <p>SnapshotDB Cloud requires a standard <code>mongodb://</code> seed-list URL with public hosts. Hosted <code>mongodb+srv://</code> discovery is disabled because its additional destinations cannot yet be safely validated. Ask your provider for the standard connection string, retain its replica-set and authentication options, and set <code>tls=true</code> for a TLS source. BYOC supports SRV discovery inside your own network boundary.</p>

      <h2>How sync works</h2>
      <p>snapshotdb takes a change-stream resume token, copies the data with <code>mongodump | mongorestore</code>, then a tailer applies every change as an upsert or delete keyed by <code>_id</code>, so the overlap with the dump is harmless. It replicates index creation and drops, collection creation with options, and <code>collMod</code>. Every branch mongod runs as its own single-node replica set, so transactions and change streams work on branches too.</p>

      <h2>Failed events</h2>
      <p>An event the replica cannot apply is logged to <code>run/tail.errors</code> and skipped, never fatal. <code>status</code> shows the tailer state and how many events were skipped; <code>repair</code> restarts the tailer from its last resume token.</p>

      <div className="np"><a href="/docs/connectors/mysql">← MySQL</a><a className="n" href="/docs/architecture">Architecture →</a></div>
    </>
  );
}
