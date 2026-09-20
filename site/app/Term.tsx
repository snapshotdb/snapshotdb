"use client";

import { useEffect, useRef, useState } from "react";

type Seg = { t: string; c?: string };

const LINES: Seg[][] = [
  [{ t: "# check the source, then keep a live replica", c: "c" }],
  [{ t: "$ ", c: "p" }, { t: "snapshotdb preflight postgres", c: "k" }, { t: " 'postgresql://…/app'   " }, { t: "✓ passed", c: "c" }],
  [{ t: "$ ", c: "p" }, { t: "snapshotdb clone prod", c: "k" }, { t: " 'postgresql://…/app'" }],
  [],
  [{ t: "# branch it — ready in seconds, its own URL", c: "c" }],
  [{ t: "$ ", c: "p" }, { t: "snapshotdb create fix-orders --from prod --print-url", c: "k" }],
  [{ t: "postgresql://you:••••@branches.internal:57375/app" }],
];

const CHARS: { ch: string; c?: string }[] = [];
LINES.forEach((line, li) => {
  line.forEach((seg) => { for (const ch of seg.t) CHARS.push({ ch, c: seg.c }); });
  if (li < LINES.length - 1) CHARS.push({ ch: "\n" });
});

export default function Term() {
  const [n, setN] = useState(0);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) { const frame = requestAnimationFrame(() => setN(CHARS.length)); return () => cancelAnimationFrame(frame); }
    let iv = 0;
    let started = false;
    const begin = () => {
      if (started) return;
      started = true;
      window.removeEventListener("scroll", check);
      iv = window.setInterval(() => {
        setN((v) => {
          if (v >= CHARS.length) { window.clearInterval(iv); return v; }
          return v + 1;
        });
      }, 16);
    };
    const check = () => {
      const r = el.getBoundingClientRect();
      if (r.top < window.innerHeight * 0.82 && r.bottom > 0) begin();
    };
    window.addEventListener("scroll", check, { passive: true });
    check();
    return () => { window.removeEventListener("scroll", check); window.clearInterval(iv); };
  }, []);

  const shown = CHARS.slice(0, n);
  const nodes: React.ReactNode[] = [];
  let buf = "";
  let bufC: string | undefined;
  let key = 0;
  const flush = () => {
    if (!buf) return;
    nodes.push(bufC ? <span key={key++} className={bufC}>{buf}</span> : <span key={key++}>{buf}</span>);
    buf = "";
  };
  for (const c of shown) {
    if (c.ch === "\n") { flush(); nodes.push(<br key={key++} />); continue; }
    if (c.c !== bufC) { flush(); bufC = c.c; }
    buf += c.ch;
  }
  flush();
  const done = n >= CHARS.length;

  return (
    <div className="term typer" ref={ref}>
      <div className="bar"><i /><i /><i /><span>snapshotdb — zsh</span></div>
      <pre>{nodes}<span className={"caret" + (done ? " idle" : "")} /></pre>
    </div>
  );
}
