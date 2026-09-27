export const metadata = { title: "snapshotdb docs — Overview" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Documentation</span>
      <h1>snapshotdb</h1>
      <p className="lead">Branch your database like code. An isolated, writable copy of production in seconds — across four engines, on infrastructure you own. Open source, self-hosted, Apache-2.0.</p>

      <p>snapshotdb keeps a live replica of your production database on a server you run, then cuts copy-on-write branches off it. Each branch is a real, writable database with its own port and its own generated credentials. Unchanged pages share storage; database startup and preparation can still take time.</p>

      <div className="note"><b>New here?</b> Start with the <a href="/docs/quickstart">Quickstart</a> — preflight a source, wait for its replica to become ready, and create a branch.</div>

      <h2>What it does</h2>
      <ul>
        <li><b>Replica once, branch forever.</b> One initial copy of production; use copy-on-write clones and prepared pools for fast branches.</li>
        <li><b>Any engine.</b> Postgres, MySQL, MongoDB, and SQLite, branched the same way.</li>
        <li><b>Separate branch credentials.</b> Branch writes do not replicate back to production. Protect copied data and enforce <a href="/docs/security">network isolation</a> separately.</li>
        <li><b>Branch from any branch.</b> Fork a branch of a branch, like git.</li>
        <li><b>Prepared pools.</b> Prepare running capacity before allocation; see the <a href="/docs/workflows/agents">measured claim times and limitations</a>.</li>
      </ul>

      <h2>How it compares to hosted branching</h2>
      <p>The console defaults to SnapshotDB Cloud and can connect to your own server in BYOC mode. The CLI requires your own server URL and admin token; GitHub CLI sign-in does not connect it to a hosted workspace. PostgreSQL, MySQL, and MongoDB use replication; SQLite starts from a server-side file or an empty database. Hosted console sessions use separate workspaces with plan limits and Dodo Payments billing.</p>

      <h2>Explore</h2>
      <ul>
        <li><a href="/docs/quickstart">Quickstart</a> and <a href="/docs/install">Install</a> / <a href="/docs/server">Server deployment</a></li>
        <li>Connect a database: <a href="/docs/connectors">overview</a>, <a href="/docs/connectors/postgres">Postgres</a>, <a href="/docs/connectors/rds">RDS</a>, <a href="/docs/connectors/supabase">Supabase</a>, <a href="/docs/connectors/mysql">MySQL</a>, <a href="/docs/connectors/mongodb">MongoDB</a></li>
        <li>Concepts: <a href="/docs/architecture">Architecture</a>, <a href="/docs/security">Security</a></li>
        <li>Reference: <a href="/docs/cli">CLI</a>, <a href="/docs/configuration">Configuration</a>, <a href="/docs/api">REST API</a>, <a href="/docs/faq">FAQ</a></li>
        <li>Workflows: <a href="/docs/workflows/agents">AI agents</a>, <a href="/docs/workflows/ci-cd">CI/CD</a>, <a href="/docs/workflows/local-dev">Local development</a></li>
      </ul>

      <div className="np"><span /><a className="n" href="/docs/quickstart">Quickstart →</a></div>
    </>
  );
}
