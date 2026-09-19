"use client";

import { useEffect, useRef, useState } from "react";

const FIGS = [
  { to: 4, dec: 0, suf: "", label: "engines" },
  { to: 0, dec: 0, suf: "", label: "prod creds leaked" },
  { to: 100, dec: 0, suf: "%", label: "self-hosted" },
];

// Big stat band; numbers count up when it scrolls into view.
export default function Stats() {
  const [p, setP] = useState(0);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = root.current;
    if (!el) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) { setP(1); return; }
    let anim = 0;
    let started = false;
    const start = () => {
      if (started) return;
      started = true;
      window.removeEventListener("scroll", check);
      window.removeEventListener("resize", check);
      let i = 0;
      const N = 26;
      anim = window.setInterval(() => {
        i += 1;
        const pr = i / N;
        setP(1 - Math.pow(1 - pr, 3));
        if (i >= N) { setP(1); window.clearInterval(anim); }
      }, 38);
    };
    const check = () => {
      const r = el.getBoundingClientRect();
      if (r.top < window.innerHeight * 0.85 && r.bottom > 0) start();
    };
    window.addEventListener("scroll", check, { passive: true });
    window.addEventListener("resize", check);
    check();
    return () => {
      window.removeEventListener("scroll", check);
      window.removeEventListener("resize", check);
      window.clearInterval(anim);
    };
  }, []);

  return (
    <section className="statband" ref={root}>
      <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />
      <div className="statband-lead">
        <span className="lbl">the number that matters</span>
        <h2>A terabyte,<br />branched in <span className="hot">~{(1.0 * p).toFixed(1)}s</span>.</h2>
        <p className="foot">*a copy-on-write clone — branch time is independent of database size.</p>
      </div>
      <div className="statrow">
        {FIGS.map((f) => (
          <div key={f.label}>
            <b>{Math.round(f.to * p)}{f.suf}</b>
            <span>{f.label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
