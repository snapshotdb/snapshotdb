import HeroFig from "./HeroFig";
import SiteFooter from "./SiteFooter";
import Boot from "./Boot";
import Stats from "./Stats";
import Desktop from "./Desktop";
import Install from "./Install";
import Term from "./Term";
import Reveal from "./Reveal";
import Logo from "./Logo";
import GithubStars from "./GithubStars";
import LifecycleFlow from "./LifecycleFlow";
const GH = "https://github.com/snapshotdb/snapshotdb";

export default function Home() {
  return (
    <div className="frame">
      <Boot />
      <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />

      <nav><div className="row">
        <a className="logo" href="#top"><Logo /><span className="wm">snapshot<em>db</em></span></a>
        <div className="links"><a href="#how">How it works</a><a href="#pricing">Pricing</a><a href="/docs">Docs</a></div>
        <a className="btn" href="/console">Sign in</a>
        <a className="btn solid" href="/docs/quickstart">Get started</a>
        <GithubStars />
      </div></nav>

      <section className="hero" id="top"><div className="grid">
        <div className="left">
          <div className="eyebrow"><b>OSS</b><span>GIT FOR YOUR DATABASE</span></div>
          <h1 className="title"><span><i>Branch your</i></span><span><i>database</i></span><span><i>like code.</i></span></h1>
          <p className="sub">An isolated, writable copy of production in seconds — PostgreSQL, MySQL, MongoDB, and SQLite. Hosted for you, or deployed in your own cloud.</p>
          <div className="statusline"><span className="live" />replica <b>in sync</b> · branch <b id="phase">opens</b></div>
          <div className="actions"><a className="btn solid" href="#start">Start branching <span className="arw">→</span></a><a className="btn" href="/console">Open console</a></div>
          <Install />
          <div className="terms">
            <div><b>&lt;1s</b>prepared 1 TB</div><div><b>0</b>prod creds</div><div><b>4</b>engines</div><div><b>CoW</b>copy-on-write</div>
          </div>
        </div>
        <HeroFig />
      </div></section>

      <div className="hatch" />

      <Stats />

      <Desktop />

      <section className="sec" id="how-anchor">
        <div className="sec-h" id="how"><span className="no">01</span><h2>The life of a branch</h2>
          <p>Click through what happens from open to cleanup.</p></div>
        <LifecycleFlow />
      </section>

      <section className="sec" id="why">
        <div className="sec-h"><span className="no">02</span><h2>Why it holds up</h2>
          <p>The guarantees that make branches safe to hand to anyone — a teammate, CI, or an agent.</p></div>
        <div className="vals">
          <div className="val reveal"><div className="n">/ ISOLATION</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><path d="M12 2l8 4v6c0 5-3.5 8-8 10-4.5-2-8-5-8-10V6z" /><path d="M9 12l2 2 4-4" /></svg>
            <h3><b>0</b> prod creds leaked</h3><p>Every branch is a sealed copy with its own credentials. A leaked branch URL can’t touch production. Agents get real data, never real access.</p></div>
          <div className="val reveal"><div className="n">/ SCALE</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><path d="M13 2L3 14h7l-1 8 10-12h-7z" /></svg>
            <h3><b>&lt;1s</b> for 1 TB, prepared</h3><p>Copy-on-write means branch time doesn’t grow with your data. Unchanged pages share storage; you pay disk only for pages you change.</p></div>
          <div className="val reveal"><div className="n">/ CONTROL</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><rect x="3" y="4" width="18" height="6" rx="1.5" /><rect x="3" y="14" width="18" height="6" rx="1.5" /><path d="M6.5 7h.01M6.5 17h.01" /></svg>
            <h3><b>4</b> engines, one workflow</h3><p>Postgres, MySQL, MongoDB, SQLite — branched the same way. Start on SnapshotDB Cloud, or choose BYOC to keep the replica and branches in your infrastructure.</p></div>
        </div>
      </section>

      <section className="sec" id="pricing">
        <div className="sec-h"><span className="no">/ PRICING</span><h2>Start small. Branch when you need it.</h2><p>Hosted by default. No server setup required.</p></div>
        <div className="vals">
          <div className="val"><div className="n">FREE TRIAL</div><h3>$0</h3><p>1 source database. 2 total branch-hours. 1 GiB storage and 1 GiB outbound data. Up to 2 running databases.</p><a className="btn" href="/console">Start free</a></div>
          <div className="val"><div className="n">PRO</div><h3>$150 / month</h3><p>Unlimited source database count. 300 branch-hours per billing month. 50 GiB shared storage and 50 GiB outbound data. Up to 4 running databases.</p><a className="btn solid" href="/console">Choose Pro</a></div>
          <div className="val"><div className="n">BRING YOUR OWN CLOUD</div><h3>Your infrastructure</h3><p>Run the open-source server in your cloud. Configure its address and access token in BYOC mode. Hosted allowances do not apply; you pay your cloud provider.</p><a className="btn" href="/docs/server">Deploy BYOC</a></div>
        </div>
        <p>One branch-hour is one database running for one hour, including source replicas. Concurrent databases add together. Each hosted database has a 1 vCPU / 2 GiB limit. Idle native engines pause after five minutes. Compute pauses at your allowance; no automatic overage charges. Storage includes the logical size of each copy. Taxes may apply.</p>
      </section>

      <section className="sec how" id="start">
        <div className="cap"><span className="lbl">03 / how it works</span><h2>Replica once. Branch forever.</h2></div>
        <div className="pipe">
          <div className="cell"><div className="op">source</div><b>Production</b><span>your live database</span></div>
          <div className="cell"><div className="op">─ replicate →</div><b>Replica</b><span>in sync, on your server</span></div>
          <div className="cell"><div className="op">─ copy-on-write →</div><b>Branches</b><span>instant · isolated · writable</span></div>
        </div>
        <Term />
      </section>

      <section className="final">
        <div className="inner">
          <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />
          <span className="lbl">open source · apache-2.0</span>
          <h2 style={{ marginTop: 16 }}>Give every change<br />its own database.</h2>
          <p>Branch production like a git branch — for migrations, tests, and coding agents. Self-hosted and free.</p>
          <div className="actions"><a className="btn solid" href={GH}>★ Star on GitHub</a><a className="btn" href="/console">Open the console</a></div>
          <div className="metrics"><div><b>&lt;1s</b><span>PREPARED 1 TB</span></div><div><b>0</b><span>PROD CREDS LEAKED</span></div><div><b>4</b><span>ENGINES</span></div></div>
        </div>
      </section>

      <SiteFooter />

      <Reveal />
    </div>
  );
}
