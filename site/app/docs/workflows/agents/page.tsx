export const metadata = { title: "AI agents — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Workflows</span>
      <h1>AI agents</h1>
      <p className="lead">Give a coding agent a real, production-shaped database it can break — with no path to production.</p>

      <p>A branch is a sealed copy with its own generated credentials. Hand the branch URL to Claude Code, Cursor, or your own agent: it gets real data to test migrations, backfills, and queries against, and the URL cannot reach production or leak a real credential.</p>

      <h2>Script-driven</h2>
      <pre className="code"><code>DATABASE_URL=&quot;$(snapshotdb create agent-task --from prod --print-url)&quot;{"\n"}[ -n &quot;$DATABASE_URL&quot; ] || {"{"} echo &apos;branch failed&apos; &gt;&amp;2; exit 1; {"}"}{"\n"}export DATABASE_URL{"\n"}<span className="c"># ... run the agent against $DATABASE_URL ...</span>{"\n"}snapshotdb rm agent-task</code></pre>

      <h2>Agent-driven with a skill</h2>
      <p>The repo ships a skill file that teaches Claude Code or Cursor to always work on a branch. Install it and add one line to your <code>CLAUDE.md</code>:</p>
      <pre className="code"><code>mkdir -p .claude/skills/snapshotdb{"\n"}curl -fsSL https://raw.githubusercontent.com/snapshotdb/snapshotdb/main/skills/snapshotdb/SKILL.md \{"\n"}  -o .claude/skills/snapshotdb/SKILL.md</code></pre>
      <p>The skill’s rules: never connect to production; create a branch and use its URL; if <code>--print-url</code> is empty, stop; delete the branch when done. An agent following it can do database work safely with no supervision of the connection string.</p>

      <h2>Prepared pools for zero-wait claims</h2>
      <p>For fleets of agents, pre-warm a pool of ready snapshots so each agent claims a branch in well under a second instead of waiting for a clone. Preparation and initial replication happen before a branch is claimed.</p>

      <pre className="code"><code>snapshotdb prepare agent-pool --from prod --count 3{"\n"}snapshotdb create agent-task --from agent-pool --print-url</code></pre>

      <div className="np"><a href="/docs/faq">← FAQ</a><a className="n" href="/docs/workflows/ci-cd">CI / CD →</a></div>
    </>
  );
}
