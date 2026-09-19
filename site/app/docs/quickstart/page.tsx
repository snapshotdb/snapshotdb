export const metadata = { title: "Quickstart — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Getting started</span>
      <h1>Quickstart</h1>
      <p className="lead">Preflight a source, keep a live replica, and cut your first branch — three commands.</p>

      <div className="note"><b>Prerequisite:</b> a running snapshotdb server. The CLI is a thin client that submits jobs; the databases live on the server. See <a href="/docs/server">Server deployment</a>. Without <code>SNAPSHOTDB_SERVER</code> the client fails and never falls back to a local copy.</div>

      <h2>1. Point the client at your server</h2>
      <pre className="code"><code>export SNAPSHOTDB_SERVER=https://snapshotdb.example.com{"\n"}export SNAPSHOTDB_TOKEN=&apos;&lt;your server access token&gt;&apos;</code></pre>

      <h2>2. Preflight the source</h2>
      <p>Preflight checks the database read-only and prints exactly what to fix. It creates nothing and stores nothing.</p>
      <pre className="code"><code><span className="p">$</span> snapshotdb preflight postgres &apos;postgresql://user:pass@db:5432/app&apos;{"\n"}  ✓ connection (PostgreSQL 16.4)   ✓ is writer   ✓ wal level (actual: logical){"\n"}  ✓ can replicate   ✓ can read tables   ! replica identity (1 table skipped){"\n"}  ✓ Preflight passed.</code></pre>
      <p>Exit code is <code>2</code> if it does not pass, so it drops straight into a script.</p>

      <h2>3. Keep a replica, then branch it</h2>
      <pre className="code"><code><span className="c"># one continuously-synced replica of production</span>{"\n"}<span className="p">$</span> snapshotdb clone prod &apos;postgresql://user:pass@db:5432/app&apos;{"\n\n"}<span className="c"># branch it — ready in seconds, its own URL and credentials</span>{"\n"}<span className="p">$</span> snapshotdb create fix-orders --from prod --print-url{"\n"}postgresql://you:••••@branches.internal:57375/app</code></pre>

      <p>Connect to that URL with any client and work freely. When you are done:</p>
      <pre className="code"><code><span className="p">$</span> snapshotdb rm fix-orders</code></pre>

      <h2>The same shape, any engine</h2>
      <p>Swap <code>postgres</code> for <code>mysql</code> or <code>mongodb</code> in <code>preflight</code>; <code>clone</code> infers the engine from the connection string. See the per-engine guides under <a href="/docs/connectors">Connect a database</a>.</p>

      <div className="np"><a href="/docs">← Overview</a><a className="n" href="/docs/install">Install →</a></div>
    </>
  );
}
