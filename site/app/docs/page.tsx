export const metadata = { title: "snapshotdb docs — Overview" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Documentation</span>
      <h1>snapshotdb</h1>
      <p className="lead">Branch your database like code. An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own. Open source, self-hosted, Apache-2.0.</p>

      <p>snapshotdb keeps a live replica of your production database on a server you run, then cuts copy-on-write branches off it. Each branch is a real, writable database with its own port and its own generated credentials. A 50 GiB branch costs the same as a 50 MiB one, because nothing is copied until a page changes.</p>

      <div className="note"><b>New here?</b> Start with the <a href="/docs/quickstart">Quickstart</a> — preflight a source, keep a replica, and cut your first branch in three commands.</div>

      <h2>What it does</h2>
      <ul>
        <li><b>Replica once, branch forever.</b> One initial copy of production; every branch after that is instant.</li>
        <li><b>Any engine.</b> Postgres, MySQL, MongoDB, and SQLite, branched the same way.</li>
        <li><b>Zero blast radius.</b> Branches are sealed copies with their own credentials — a leaked branch URL can’t reach production.</li>
        <li><b>Branch from any branch.</b> Fork a branch of a branch, like git.</li>
        <li><b>Prepared pools.</b> Pre-warm snapshots so an agent can claim a ready branch in well under a second.</li>
      </ul>

      <h2>How it compares to hosted branching</h2>
      <p>Managed branching services sell the same idea — a replica of production plus instant branches — as a Postgres-only, multi-tenant cloud product. snapshotdb is the open-source, self-hosted shape of that: you deploy the server, it uses each engine’s own replication instead of a proprietary pipeline, it covers four engines, and your data never leaves your machines. It has no accounts, orgs, billing, or bring-your-own-cloud — it is one server with one admin token.</p>

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
