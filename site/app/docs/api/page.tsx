export const metadata = { title: "REST API — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Reference</span>
      <h1>REST API</h1>
      <p className="lead">A small job API. The CLI is a client for it; anything that speaks HTTP can drive snapshotdb the same way.</p>

      <p>Every request carries <code>Authorization: Bearer $SNAPSHOTDB_TOKEN</code>. The API is a command executor: you submit a CLI command as a JSON array, poll the job, and read its output.</p>

      <h2>Endpoints</h2>
      <table>
        <thead><tr><th>Method &amp; path</th><th>Purpose</th></tr></thead>
        <tbody>
          <tr><td><code>GET /v1/health</code></td><td>Liveness; returns <code>{"{"} &quot;version&quot;: &quot;…&quot; {"}"}</code>.</td></tr>
          <tr><td><code>POST /v1/commands</code></td><td>Submit a command (JSON array of args). Returns <code>202 {"{"} &quot;id&quot;, &quot;state&quot;:&quot;queued&quot; {"}"}</code>.</td></tr>
          <tr><td><code>GET /v1/jobs/&#123;id&#125;</code></td><td>Poll a job: <code>{"{"} state, exit_code, stdout, stderr {"}"}</code>.</td></tr>
        </tbody>
      </table>

      <h2>Example</h2>
      <pre className="code"><code><span className="c"># submit `list --format json`</span>{"\n"}<span className="p">$</span> curl -sX POST $SNAPSHOTDB_SERVER/v1/commands \{"\n"}    -H &quot;Authorization: Bearer $SNAPSHOTDB_TOKEN&quot; -H &apos;Content-Type: application/json&apos; \{"\n"}    -d &apos;[&quot;list&quot;,&quot;--format&quot;,&quot;json&quot;]&apos;{"\n"}{"{"} &quot;id&quot;: &quot;&lt;id&gt;&quot;, &quot;state&quot;: &quot;queued&quot; {"}"}{"\n\n"}<span className="p">$</span> curl -s $SNAPSHOTDB_SERVER/v1/jobs/&lt;id&gt; -H &quot;Authorization: Bearer $SNAPSHOTDB_TOKEN&quot;</code></pre>

      <p>Allowed commands over the API are the branch and source lifecycle: <code>preflight, clone, sync, import, create, prepare, info, url, switch, list, status, repair, reconcile, reset, settings, lock, unlock, start, stop, rm</code>.</p>

      <div className="note"><b>Why one endpoint, not dozens:</b> a multi-tenant managed service needs many resource routes (projects, connectors, branches, operations, api-keys, orgs). snapshotdb is a single self-hosted server with no tenancy, so it exposes one generic job endpoint instead — every capability, without the multi-tenant surface.</div>

      <div className="np"><a href="/docs/configuration">← Configuration</a><a className="n" href="/docs/faq">FAQ →</a></div>
    </>
  );
}
