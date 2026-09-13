export const metadata = { title: "Local development — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Workflows</span>
      <h1>Local development</h1>
      <p className="lead">Branch before you touch the database — every time. Develop against real, production-shaped data instead of a stale dump or a shared staging box.</p>

      <h2>A branch per task</h2>
      <pre className="code"><code><span className="p">$</span> anybranch create feature-x --from prod --print-url{"\n"}postgresql://you:••••@branches.internal:57380/app</code></pre>
      <p>Point your app’s <code>DATABASE_URL</code> at it. Run migrations, try the destructive change, seed test rows — it is yours. Nothing you do reaches production or a teammate’s branch.</p>

      <h2>Reset instead of re-seed</h2>
      <p>When a branch gets messy, <code>anybranch reset feature-x</code> throws away your changes and re-clones a fresh copy from the current replica — faster and truer than rebuilding a seed script.</p>

      <h2>Leave it running</h2>
      <p>Idle branches suspend on their own after five minutes and resume instantly on the next connection, so there is no cleanup pressure on your laptop. Delete a branch with <code>anybranch rm</code> when the task is done.</p>

      <h2>Seed or anonymize every branch</h2>
      <p>Set a <code>branch_sql</code> hook on the root so every new branch comes up with fixtures applied or sensitive columns redacted, without each developer keeping their own script:</p>
      <pre className="code"><code><span className="p">$</span> anybranch settings prod set branch_sql @seed.sql --hook 20-seed</code></pre>

      <div className="np"><a href="/docs/workflows/ci-cd">← CI / CD</a><span /></div>
    </>
  );
}
