"use client";

import { useEffect, useState } from "react";

const STEPS = ["mounting replica", "probing engines", "warming branch pool", "ready"];

// Retro boot loader — plays once per tab, then fades to reveal the page.
export default function Boot() {
  const [show, setShow] = useState(false);
  const [pct, setPct] = useState(0);
  const [out, setOut] = useState(false);

  useEffect(() => {
    let played = false;
    try { played = sessionStorage.getItem("sdb_booted") === "1"; } catch {}
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (played || reduce) return;
    const frame = window.requestAnimationFrame(() => setShow(true));
    const previousOverflow = document.documentElement.style.overflow;
    const timers: number[] = [];

    document.documentElement.style.overflow = "hidden";
    let p = 0;
    const iv = window.setInterval(() => {
      p = Math.min(100, p + Math.random() * 17 + 7);
      setPct(Math.round(p));
      if (p >= 100) {
        window.clearInterval(iv);
        try { sessionStorage.setItem("sdb_booted", "1"); } catch {}
        timers.push(window.setTimeout(() => setOut(true), 350));
        timers.push(window.setTimeout(() => { document.documentElement.style.overflow = previousOverflow; setShow(false); }, 1000));
      }
    }, 190);
    return () => { window.cancelAnimationFrame(frame); window.clearInterval(iv); timers.forEach(window.clearTimeout); document.documentElement.style.overflow = previousOverflow; };
  }, []);

  if (!show) return null;
  const step = STEPS[Math.min(STEPS.length - 1, Math.floor((pct / 100) * STEPS.length))];

  return (
    <div className={"boot" + (out ? " out" : "")} aria-hidden="true">
      <div className="boot-win">
        <div className="boot-bar"><span>snapshotdb</span><span>v0.4.0</span></div>
        <div className="boot-body">
          <div className="boot-line">booting<span className="boot-dots" /></div>
          <div className="boot-step">{step}</div>
          <div className="boot-prog"><i style={{ width: pct + "%" }} /></div>
          <div className="boot-pct">{pct}%</div>
        </div>
      </div>
    </div>
  );
}
