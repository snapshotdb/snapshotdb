export const metadata = { title: "Security — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Concepts</span>
      <h1>Security</h1>
      <p className="lead">Branches are sealed copies with their own credentials, on infrastructure you own. Your data never leaves your machines.</p>

      <h2>Minimal production footprint</h2>
      <p>snapshotdb reads from production over the engine’s replication and never writes to your application tables. On Postgres it adds only its own publication, slot, metadata schema, and DDL event trigger, all named <code>snapshotdb_&lt;name&gt;</code>, and <code>rm</code> removes them. On MySQL and MongoDB it creates nothing on the source.</p>

      <h2>Per-branch credentials</h2>
      <p>Roots that snapshotdb creates get a generated admin password, and every clone rotates to its own password on first start — so a branch URL never opens its parent. Postgres accepts the password over TCP and trusts only its Unix socket (which snapshotdb itself uses); MongoDB runs with a keyFile and a generated <code>root</code> user. A leaked branch URL has no path to production and no shared credential. This is what makes it safe to hand a branch to a coding agent.</p>

      <h2>Server token</h2>
      <p>The control API is guarded by a single admin token (<code>SNAPSHOTDB_TOKEN</code>) presented as <code>Authorization: Bearer</code>. There is no tenancy: anyone with the token is an admin. Keep it in a secret store, put the server behind your network boundary or a TLS terminator, and rotate by restarting with a new value.</p>

      <h2>Anonymization</h2>
      <p>Use a per-root <code>branch_sql</code> hook to redact or synthesize sensitive columns on every new branch. It runs once per branch, right after creation, and never touches the source. See <a href="/docs/cli">Settings</a>.</p>

      <h2>WAL retention</h2>
      <p>A stopped Postgres synced root keeps its replication slot, which retains WAL on the source until the root is started again or removed. <code>status</code> shows how much WAL the slot is holding. Preflight warns when <code>max_slot_wal_keep_size</code> is unbounded — set a bound so a forgotten root cannot fill the source’s disk.</p>

      <div className="np"><a href="/docs/architecture">← Architecture</a><a className="n" href="/docs/cli">CLI reference →</a></div>
    </>
  );
}
