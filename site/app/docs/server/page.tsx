export const metadata = { title: "Server deployment — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Getting started</span>
      <h1>Server deployment</h1>
      <p className="lead">The server holds the replica, the branches, and the engine processes. The CLI talks to it over an authenticated HTTP API.</p>

      <h2>Run the server</h2>
      <pre className="code"><code><span className="c"># generate an admin token and start the server</span>{"\n"}<span className="p">$</span> export SNAPSHOTDB_TOKEN=$(openssl rand -hex 32){"\n"}<span className="p">$</span> snapshotdb serve --bind 127.0.0.1:8080 --db-bind 0.0.0.0 --public-host branches.internal</code></pre>
      <ul>
        <li><code>--bind</code> — where the control API listens.</li>
        <li><code>--db-bind</code> — interface used for branch database connections. Use a private interface and restrict access in your firewall.</li>
        <li><code>--public-host</code> — the hostname branch connection URLs are handed out with (reachable by whoever will use the branches).</li>
        <li><code>SNAPSHOTDB_HOME</code> — where the server stores databases (default <code>~/.snapshotdb</code>). Put it on a reflink-capable volume.</li>
        <li><code>SNAPSHOTDB_TOKEN</code> — the single admin token every request must present as <code>Authorization: Bearer</code>.</li>
      </ul>

      <div className="note"><b>One server, one token.</b> snapshotdb has no accounts or tenancy. Anyone with the token is an admin. Keep it in a secret store and put the server behind your own network boundary or TLS terminator.</div>

      <h2>Point clients at it</h2>
      <pre className="code"><code>export SNAPSHOTDB_SERVER=https://snapshotdb.example.com{"\n"}export SNAPSHOTDB_TOKEN=&apos;…&apos;</code></pre>
      <p>Every command becomes an authenticated server job. The CLI submits it, waits, and prints the result; <code>--detach</code> returns a job id immediately and <code>snapshotdb job &lt;id&gt;</code> reconnects. A dropped client connection does not cancel the work — a long initial copy keeps running server-side, and <code>status</code> monitors it.</p>

      <h2>Connect the browser console</h2>
      <p>Serve the API over HTTPS, then set the exact console origins on the server before starting it:</p>
      <pre className="code"><code>export SNAPSHOTDB_CONSOLE_ORIGINS=https://snapshotdb.io,https://www.snapshotdb.io</code></pre>
      <p>In Console → Connection, enter your HTTPS API address and server token. Cross-origin access is disabled unless explicitly allowed. For local development, allow <code>http://localhost:3000</code>.</p>
      <h2>Reboots</h2>
      <p>Restart <code>snapshotdb serve</code> to restore branch proxies and restart synced roots. Use the systemd deployment unit to start it automatically after reboot.</p>

      <div className="np"><a href="/docs/install">← Install</a><a className="n" href="/docs/connectors">Connect a database →</a></div>
    </>
  );
}
