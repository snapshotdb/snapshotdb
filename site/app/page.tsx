import HeroFig from "./HeroFig";
import SiteFooter from "./SiteFooter";
import Boot from "./Boot";
import Stats from "./Stats";
import Desktop from "./Desktop";
import Install from "./Install";
import Term from "./Term";
import Reveal from "./Reveal";

function Logo({ size = 23 }: { size?: number }) {
  return (
    <svg viewBox="0 0 32 32" width={size} height={size} fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
      <circle cx="9" cy="7" r="3" /><circle cx="9" cy="25" r="3" /><circle cx="23" cy="16" r="3" /><path d="M9 10v12M9 16h4a6 6 0 0 0 6-6" />
    </svg>
  );
}
const GH = "https://github.com/GitHoobar/anybranch";

export default function Home() {
  return (
    <div className="frame">
      <Boot />
      <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />

      <nav><div className="row">
        <a className="logo" href="#top"><Logo /><span className="wm">snapshot<em>db</em></span></a>
        <div className="links"><a href="#how">How it works</a><a href="#why">Why</a><a href="/docs">Docs</a><a href="/console">Console</a><a href={GH}>GitHub</a></div>
        <a className="btn solid" href="#start">Get started</a>
      </div></nav>

      <section className="hero" id="top"><div className="grid">
        <div className="left">
          <div className="eyebrow"><b>OSS</b><span>GIT FOR YOUR DATABASE</span></div>
          <h1 className="title"><span><i>Branch your</i></span><span><i>database</i></span><span><i>like code.</i></span></h1>
          <p className="sub">An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own.</p>
          <div className="statusline"><span className="live" />replica <b>in sync</b> · branch <b id="phase">opens</b></div>
          <div className="actions"><a className="btn solid" href="#start">Start branching <span className="arw">→</span></a><a className="btn" href="/console">Open console</a></div>
          <Install />
          <div className="terms">
            <div><b>~1s</b>1 TB branch</div><div><b>0</b>prod creds</div><div><b>4</b>engines</div><div><b>CoW</b>copy-on-write</div>
          </div>
        </div>
        <HeroFig />
      </div></section>

      <div className="hatch" />

      <Stats />

      <Desktop />

      <section className="sec" id="how-anchor">
        <div className="sec-h" id="how"><span className="no">01</span><h2>The life of a branch</h2>
          <p>Opens off the replica, diverges into isolation, does real work, collects nothing back.</p></div>
        <div className="beats">
          <div className="beat reveal"><div className="k">01 / OPEN</div><h3>It <em>opens</em></h3><p>A copy-on-write clone of the live replica. No data copies until a page changes — 1 TB opens as fast as 1 MB.</p>
            <svg viewBox="0 0 40 40" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><circle cx="10" cy="20" r="4" /><path d="M14 20h8" /><circle cx="28" cy="20" r="4" /></svg></div>
          <div className="beat reveal"><div className="k">02 / DIVERGE</div><h3>It <em>diverges</em></h3><p>Own server, own port, own generated credentials. Writes stay on the branch — never production, never a sibling.</p>
            <svg viewBox="0 0 40 40" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><circle cx="9" cy="20" r="4" /><path d="M13 20q9 0 12-7M13 20q9 0 12 7" /><circle cx="29" cy="12" r="3" /><circle cx="29" cy="28" r="3" /></svg></div>
          <div className="beat reveal"><div className="k">03 / WORK</div><h3>It does <em>work</em></h3><p>Run the migration, the backfill, the risky query, the agent. Break it entirely. Real data, zero blast radius.</p>
            <svg viewBox="0 0 40 40" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><circle cx="20" cy="20" r="4.5" /><path d="M20 8v5M20 27v5M8 20h5M27 20h5" /></svg></div>
          <div className="beat reveal"><div className="k">04 / COLLECT</div><h3>It <em>collects</em></h3><p>Keep what you learned, drop the branch, or reset to a fresh copy. Production never felt a thing. Idle branches suspend.</p>
            <svg viewBox="0 0 40 40" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round"><circle cx="28" cy="20" r="4" /><path d="M24 20H15M20 13l-7 7 7 7" /></svg></div>
        </div>
      </section>

      <section className="sec" id="why">
        <div className="sec-h"><span className="no">02</span><h2>Why it holds up</h2>
          <p>The guarantees that make branches safe to hand to anyone — a teammate, CI, or an agent.</p></div>
        <div className="vals">
          <div className="val reveal"><div className="n">/ ISOLATION</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><path d="M12 2l8 4v6c0 5-3.5 8-8 10-4.5-2-8-5-8-10V6z" /><path d="M9 12l2 2 4-4" /></svg>
            <h3>Zero blast radius</h3><p>Every branch is a sealed copy with its own credentials. A leaked branch URL can’t touch production. Agents get real data, never real access.</p></div>
          <div className="val reveal"><div className="n">/ SCALE</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><path d="M13 2L3 14h7l-1 8 10-12h-7z" /></svg>
            <h3>Seconds at any size</h3><p>Copy-on-write means branch time doesn’t grow with your data. A terabyte forks in about a second; you pay disk only for pages you change.</p></div>
          <div className="val reveal"><div className="n">/ CONTROL</div>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeLinecap="round"><rect x="3" y="4" width="18" height="6" rx="1.5" /><rect x="3" y="14" width="18" height="6" rx="1.5" /><path d="M6.5 7h.01M6.5 17h.01" /></svg>
            <h3>Any engine, your infra</h3><p>Postgres, MySQL, MongoDB, SQLite — branched the same way, on a server you run. Open source. Your data never leaves your machines.</p></div>
        </div>
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
          <div className="metrics"><div><b>~1s</b><span>1 TB BRANCH</span></div><div><b>0</b><span>PROD CREDS LEAKED</span></div><div><b>4</b><span>ENGINES</span></div></div>
        </div>
      </section>

      <SiteFooter />

      <Reveal />
    </div>
  );
}
