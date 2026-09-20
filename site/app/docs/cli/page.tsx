export const metadata = { title: "CLI reference — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Reference</span>
      <h1>CLI reference</h1>
      <p className="lead">Every command submits an authenticated job to the server. <code>create</code> and <code>sync</code> make the new branch current, so <code>info --print-url</code> needs no name.</p>

      <h2>Sources</h2>
      <table>
        <thead><tr><th>Command</th><th>What it does</th></tr></thead>
        <tbody>
          <tr><td><code>preflight &lt;engine&gt; &lt;url&gt; [--schemas a,b] [--format json]</code></td><td>Check a source read-only; exit 2 on failure. Creates nothing.</td></tr>
          <tr><td><code>clone &lt;name&gt; &lt;url&gt;</code></td><td>Infer the engine and create a continuously-synced replica.</td></tr>
          <tr><td><code>sync &lt;engine&gt; &lt;name&gt; &lt;url&gt; [--schemas] [--fix-replica-identity]</code></td><td>Same, engine explicit; can fix replica identity on the source.</td></tr>
          <tr><td><code>import &lt;engine&gt; &lt;name&gt; &lt;datadir|file|--new&gt;</code></td><td>Root from a stopped data dir, a SQLite file, or an empty database.</td></tr>
          <tr><td><code>status &lt;name&gt; [--format json]</code></td><td>Replication state: initial-copy progress, WAL lag, schema tracking.</td></tr>
          <tr><td><code>repair &lt;name&gt;</code></td><td>Resume a paused replica — skip a poison transaction or reconcile schema.</td></tr>
          <tr><td><code>reconcile &lt;name&gt;</code></td><td>Add columns/tables the source gained (for sources without the event trigger).</td></tr>
        </tbody>
      </table>

      <h2>Branches</h2>
      <table>
        <thead><tr><th>Command</th><th>What it does</th></tr></thead>
        <tbody>
          <tr><td><code>create &lt;name&gt; --from &lt;parent&gt; [--print-url] [--format json]</code></td><td>Copy-on-write branch of a root or another branch. Idempotent.</td></tr>
          <tr><td><code>prepare &lt;snapshot&gt; --from &lt;parent&gt; --count &lt;1-32&gt;</code></td><td>Prepare an immutable snapshot and ready branch pool. Claim with <code>create --from &lt;snapshot&gt;</code>; preparation happens ahead of the claim.</td></tr>
          <tr><td><code>info [name] [--print-url] [--format json]</code></td><td>Details of a branch (default: current).</td></tr>
          <tr><td><code>url [name]</code> · <code>switch &lt;name&gt;</code></td><td>Full connection URL · make a branch current.</td></tr>
          <tr><td><code>reset &lt;name&gt;</code></td><td>Throw away changes, re-clone from the parent.</td></tr>
          <tr><td><code>start</code> · <code>stop</code> · <code>rm &lt;name&gt;</code></td><td>Resume · suspend · delete.</td></tr>
          <tr><td><code>lock</code> · <code>unlock &lt;name&gt;</code></td><td>Protect a branch from <code>rm</code>.</td></tr>
          <tr><td><code>list [--format json]</code></td><td>All branches; passwords redacted.</td></tr>
        </tbody>
      </table>

      <h2>Settings</h2>
      <p>Per-root defaults applied to every new branch:</p>
      <pre className="code"><code>snapshotdb settings &lt;root&gt; set default_db app{"\n"}snapshotdb settings &lt;root&gt; set branch_sql @anonymize.sql --hook 10-anon{"\n"}snapshotdb settings &lt;root&gt; set source &apos;postgresql://…&apos;   <span className="c"># rotate credentials</span></code></pre>
      <p><code>branch_sql</code> hooks run once per new branch, in hook-name order; multiple hooks coexist via <code>--hook</code>.</p>

      <h2>Server</h2>
      <p><code>serve</code> starts the authenticated API and restores existing branches. Run it under a service manager for automatic startup. See <a href="/docs/server">Server deployment</a>.</p>

      <div className="np"><a href="/docs/security">← Security</a><a className="n" href="/docs/configuration">Configuration →</a></div>
    </>
  );
}
