export const metadata = { title: "Security — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Concepts</span>
      <h1>Security</h1>
      <p className="lead">Branches have their own credentials. BYOC stores replicas on infrastructure you operate; SnapshotDB Cloud copies source data to hosted infrastructure.</p>

      <h2>Minimal production footprint</h2>
      <p>snapshotdb reads from production over the engine’s replication and never writes to your application tables. On Postgres it adds only its own publication, slot, metadata schema, and DDL event trigger, all named <code>snapshotdb_&lt;name&gt;</code>, and <code>rm</code> removes them. On MySQL and MongoDB it creates nothing on the source.</p>

      <h2>Per-branch credentials</h2>
      <p>Roots that snapshotdb creates get a generated admin password, and every clone rotates to its own password on first start. Postgres accepts the password over TCP and trusts only its Unix socket (which snapshotdb itself uses); MongoDB runs with a keyFile and a generated <code>root</code> user. Treat branch URLs as secrets: they grant access to the copied data. Separate credentials do not replace network isolation or data redaction.</p>

      <h2>Server token</h2>
      <p>The BYOC control API uses an admin token (<code>SNAPSHOTDB_TOKEN</code>) presented as <code>Authorization: Bearer</code>; anyone with that token is an admin. The hosted gateway uses a separate server-side secret and derives each workspace from the signed-in GitHub account. Keep server tokens in a secret store, put the API behind a TLS terminator, and rotate by restarting with a new value.</p>

      <h2>Network boundaries</h2>
      <p>The filesystem sandbox does not isolate networking. Hosted source validation rejects private initial addresses and SRV discovery, but database clients can resolve DNS again or discover additional replica members. Operators must enforce egress restrictions that block metadata services, private infrastructure, and other tenants at connection time. Do not expose a shared hosted service on the strength of URL validation alone.</p>
      <p>The native database proxies do not add TLS; SQLite query URLs use HTTP. An HTTPS control API does not encrypt those database connections. Keep them on a private network or behind an authenticated encrypted tunnel or compatible TLS gateway before transmitting credentials or source data.</p>

      <h2>Anonymization</h2>
      <p>Use a per-root <code>branch_sql</code> hook to redact or synthesize sensitive columns on every new branch. It runs once per branch, right after creation, and never touches the source. See <a href="/docs/cli">Settings</a>.</p>

      <h2>WAL retention</h2>
      <p>A stopped Postgres synced root keeps its replication slot, which retains WAL on the source until the root is started again or removed. <code>status</code> shows how much WAL the slot is holding. Preflight warns when <code>max_slot_wal_keep_size</code> is unbounded — set a bound so a forgotten root cannot fill the source’s disk.</p>

      <div className="np"><a href="/docs/architecture">← Architecture</a><a className="n" href="/docs/cli">CLI reference →</a></div>
    </>
  );
}
