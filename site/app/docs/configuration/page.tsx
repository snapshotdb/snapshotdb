export const metadata = { title: "Configuration — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Reference</span>
      <h1>Configuration</h1>
      <p className="lead">Environment variables, exit codes, and JSON output — everything you need to wire snapshotdb into a script, CI job, or agent.</p>

      <h2>Environment variables</h2>
      <table>
        <thead><tr><th>Variable</th><th>Meaning</th></tr></thead>
        <tbody>
          <tr><td><code>SNAPSHOTDB_SERVER</code></td><td>Server base URL the client submits jobs to. Required — the client fails without it.</td></tr>
          <tr><td><code>SNAPSHOTDB_TOKEN</code></td><td>Admin token, sent as <code>Authorization: Bearer</code>.</td></tr>
          <tr><td><code>SNAPSHOTDB_HOME</code></td><td><b>Server</b> storage directory (default <code>~/.snapshotdb</code>). Put it on a reflink-capable volume.</td></tr>
          <tr><td><code>SNAPSHOTDB_IDLE_MINUTES</code></td><td>Idle timeout before a branch suspends (default 5; 0 never suspends).</td></tr>
        </tbody>
      </table>

      <h2>Exit codes</h2>
      <table>
        <thead><tr><th>Code</th><th>Meaning</th></tr></thead>
        <tbody>
          <tr><td><code>0</code></td><td>Success.</td></tr>
          <tr><td><code>1</code></td><td>Something failed while running — network, engine, timeout.</td></tr>
          <tr><td><code>2</code></td><td>The command was wrong, or <code>preflight</code> did not pass.</td></tr>
        </tbody>
      </table>

      <h2>JSON and scripting</h2>
      <ul>
        <li><code>--print-url</code> prints only the connection URL, nothing else. Treat an empty result as a hard failure — do not fall back to another database.</li>
        <li><code>--format json</code> prints a stable object with a <code>schema_version</code>; errors come back as <code>{"{"}&quot;error&quot;: &quot;…&quot;{"}"}</code>.</li>
        <li><code>--detach</code> returns a job id immediately; <code>snapshotdb job &lt;id&gt;</code> reconnects. A long clone keeps running server-side after the client disconnects.</li>
      </ul>

      <div className="np"><a href="/docs/cli">← CLI reference</a><a className="n" href="/docs/api">REST API →</a></div>
    </>
  );
}
