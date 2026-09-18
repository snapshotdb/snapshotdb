export const metadata = { title: "FAQ — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Reference</span>
      <h1>FAQ</h1>
      <p className="lead">Common questions about branching, replication, and what snapshotdb does and does not do.</p>

      <h2>How fast is a branch, really?</h2>
      <p>Branch time does not grow with data size. Measured on an M5 Pro laptop (APFS): a Postgres branch of an 88&nbsp;GB / 600M-row database takes about 0.3&nbsp;s from a stopped parent and 0.6&nbsp;s from a running one. Two clones of that database consumed no measurable extra disk. Prepared snapshots claim in well under a second.</p>

      <h2>Does a branch get new production changes after it is created?</h2>
      <p>No. A branch is a point-in-time copy of the replica at the moment you branched. After that it is isolated. The <b>replica</b> keeps syncing; run <code>reset</code> to re-clone a branch from the current replica.</p>

      <h2>Do schema changes on production reach branches?</h2>
      <p>Yes for the replica. Postgres replays DDL through an event trigger (or <code>reconcile</code> without superuser); MySQL replicates DDL natively; MongoDB replicates index and collection changes. New branches made after a migration include it.</p>

      <h2>Which engines are supported?</h2>
      <p>Postgres, MySQL, MongoDB, and SQLite. Hosted branching products are Postgres-only.</p>

      <h2>Is my data sent anywhere?</h2>
      <p>No. snapshotdb is self-hosted. The replica and every branch live on a server you run; nothing leaves your infrastructure and there is no telemetry.</p>

      <h2>What is intentionally not here?</h2>
      <p>No accounts, organizations, teams, roles, billing, or bring-your-own-cloud. snapshotdb is one server with one admin token. Those are multi-tenant SaaS concerns; if you need shared access, run the server where your team can reach it.</p>

      <h2>What are the known limits?</h2>
      <ul>
        <li>Postgres DDL replay uses the whole client query string; migration tools that send one statement per query are exact, but a single query mixing DDL and DML replays the DML too.</li>
        <li>On a filesystem without reflink, branches fall back to full copies (branch storage is no longer free).</li>
        <li>Branch URLs are on the server’s <code>--public-host</code>; there is no managed routing layer.</li>
      </ul>

      <div className="np"><a href="/docs/api">← REST API</a><a className="n" href="/docs/workflows/agents">AI agents →</a></div>
    </>
  );
}
