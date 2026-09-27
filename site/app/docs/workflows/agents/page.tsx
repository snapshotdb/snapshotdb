export const metadata = { title: "AI agents — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Workflows</span>
      <h1>AI agents</h1>
      <p className="lead">Give each coding agent a writable database branch with its own URL and credentials.</p>

      <p>Hand a branch URL to Claude Code, Codex, OpenCode, or your own agent to test migrations, backfills, and queries. Branch writes do not replicate back to the source. The URL grants access to copied data, so keep it private, apply masking when needed, and enforce the <a href="/docs/security">network and transport boundaries</a>.</p>

      <h2>Script-driven</h2>
      <pre className="code"><code>DATABASE_URL=&quot;$(snapshotdb create agent-task --from prod --print-url)&quot;{"\n"}[ -n &quot;$DATABASE_URL&quot; ] || {"{"} echo &apos;branch failed&apos; &gt;&amp;2; exit 1; {"}"}{"\n"}export DATABASE_URL{"\n"}<span className="c"># ... run the agent against $DATABASE_URL ...</span>{"\n"}snapshotdb rm agent-task</code></pre>

      <h2>Agent-driven with a skill</h2>
      <p>The repo ships a skill describing branch creation, connection rules, and cleanup. For Claude Code, install it in your project:</p>
      <pre className="code"><code>mkdir -p .claude/skills/snapshotdb{"\n"}curl -fsSL https://raw.githubusercontent.com/snapshotdb/snapshotdb/main/skills/snapshotdb/SKILL.md \{"\n"}  -o .claude/skills/snapshotdb/SKILL.md</code></pre>
      <p>The skill’s rules: never connect to production; create a branch and use its URL; if <code>--print-url</code> is empty, stop; delete the branch when done. Use an explicit branch name when retrieving a URL in a workspace shared by multiple agents. These instructions complement the server’s access controls; they do not replace them.</p>

      <h2>Prepared pools for fast claims</h2>
      <p>Prepare an immutable snapshot and running branches before an agent burst. Recorded prepared claims completed in under a second; preparation, initial replication, and cold resume are separate. This is a measured result, not a latency guarantee. See the <a href="https://github.com/snapshotdb/snapshotdb/blob/main/docs/latency-results.md">benchmark conditions and raw evidence</a>.</p>

      <pre className="code"><code>snapshotdb prepare agent-pool --from prod --count 3{"\n"}snapshotdb create agent-task --from agent-pool --print-url</code></pre>
      <p>Claims consume finite warm capacity. Repeat <code>prepare</code> to refill the same snapshot; use a new snapshot name for newer source data. An exhausted pool returns an error instead of falling back to a cold clone.</p>

      <div className="np"><a href="/docs/faq">← FAQ</a><a className="n" href="/docs/workflows/ci-cd">CI / CD →</a></div>
    </>
  );
}
