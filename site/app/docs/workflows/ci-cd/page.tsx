export const metadata = { title: "CI / CD — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Workflows</span>
      <h1>CI / CD</h1>
      <p className="lead">A fresh, production-shaped database for every pull request. Isolated, real data, torn down after the run.</p>

      <h2>Per-PR branch</h2>
      <pre className="code"><code>DATABASE_URL=&quot;$(anybranch create &quot;pr-$PR_NUMBER&quot; --from prod --print-url)&quot;{"\n"}[ -n &quot;$DATABASE_URL&quot; ] || exit 1{"\n"}export DATABASE_URL{"\n"}<span className="c"># run migrations and the test suite against $DATABASE_URL</span>{"\n"}anybranch rm &quot;pr-$PR_NUMBER&quot;</code></pre>

      <h2>GitHub Actions</h2>
      <pre className="code"><code>env:{"\n"}  ANYBRANCH_SERVER: https://anybranch.internal{"\n"}  ANYBRANCH_TOKEN: $&#123;&#123; secrets.ANYBRANCH_TOKEN &#125;&#125;{"\n"}steps:{"\n"}  - run: |{"\n"}      DATABASE_URL=&quot;$(anybranch create pr-$&#123;&#123; github.event.number &#125;&#125; --from prod --print-url)&quot;{"\n"}      export DATABASE_URL{"\n"}      ./run-migrations &amp;&amp; ./test{"\n"}  - if: always(){"\n"}    run: anybranch rm pr-$&#123;&#123; github.event.number &#125;&#125;</code></pre>
      <p>Use <code>if: always()</code> (or your CI’s equivalent) so the branch is deleted even when tests fail. Set <code>ANYBRANCH_TOKEN</code> as a secret; nothing else is needed.</p>

      <div className="note"><b>Tip:</b> because a branch is copy-on-write, a per-PR database of any size is instant and costs disk only for what the tests change. Delete it after the run to release that.</div>

      <div className="np"><a href="/docs/workflows/agents">← AI agents</a><a className="n" href="/docs/workflows/local-dev">Local development →</a></div>
    </>
  );
}
