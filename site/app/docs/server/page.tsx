export const metadata = { title: "Server deployment — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Getting started</span>
      <h1>Server deployment</h1>
      <p className="lead">The server holds the replica, the branches, and the engine processes. The CLI talks to it over an authenticated HTTP API.</p>

      <h2>Run the server</h2>
      <pre className="code"><code><span className="c"># generate an admin token and start the server</span>{"\n"}<span className="p">$</span> export ANYBRANCH_TOKEN=$(openssl rand -hex 32){"\n"}<span className="p">$</span> anybranch serve --bind 0.0.0.0:8080 --public-host branches.internal</code></pre>
      <ul>
        <li><code>--bind</code> — where the control API listens.</li>
        <li><code>--public-host</code> — the hostname branch connection URLs are handed out with (reachable by whoever will use the branches).</li>
        <li><code>ANYBRANCH_HOME</code> — where the server stores databases (default <code>~/.anybranch</code>). Put it on a reflink-capable volume.</li>
        <li><code>ANYBRANCH_TOKEN</code> — the single admin token every request must present as <code>Authorization: Bearer</code>.</li>
      </ul>

      <div className="note"><b>One server, one token.</b> anybranch has no accounts or tenancy. Anyone with the token is an admin. Keep it in a secret store and put the server behind your own network boundary or TLS terminator.</div>

      <h2>Point clients at it</h2>
      <pre className="code"><code>export ANYBRANCH_SERVER=https://anybranch.example.com{"\n"}export ANYBRANCH_TOKEN=&apos;…&apos;</code></pre>
      <p>Every command becomes an authenticated server job. The CLI submits it, waits, and prints the result; <code>--detach</code> returns a job id immediately and <code>anybranch job &lt;id&gt;</code> reconnects. A dropped client connection does not cancel the work — a long initial copy keeps running server-side, and <code>status</code> monitors it.</p>

      <h2>Reboots</h2>
      <p>After the machine restarts, run <code>anybranch up</code> once to bring every branch’s proxy back and restart synced roots, or install the login service (<a href="/docs/install">Install</a>) to do it automatically.</p>

      <div className="np"><a href="/docs/install">← Install</a><a className="n" href="/docs/connectors">Connect a database →</a></div>
    </>
  );
}
