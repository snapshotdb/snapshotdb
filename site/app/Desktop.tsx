"use client";

import { useEffect, useRef, useState } from "react";

let zTop = 10;

function LiveClock() {
  const [t, setT] = useState("--:--:--");
  const [d, setD] = useState("—");
  useEffect(() => {
    const upd = () => {
      const n = new Date();
      setT(n.toLocaleTimeString([], { hour12: false }));
      setD(n.toLocaleDateString([], { weekday: "short", month: "short", day: "2-digit", year: "numeric" }));
    };
    upd();
    const iv = window.setInterval(upd, 1000);
    return () => window.clearInterval(iv);
  }, []);
  return (
    <>
      <div className="ck-d">{d}</div>
      <div className="ck-t">{t}</div>
    </>
  );
}

type WinProps = {
  title: string;
  className?: string;
  children: React.ReactNode;
};

function Win({ title, className, children }: WinProps) {
  const ref = useRef<HTMLDivElement>(null);
  const pos = useRef({ x: 0, y: 0 });
  const drag = useRef<{ dx: number; dy: number } | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el?.parentElement) return;
    const resize = new ResizeObserver(() => {
      pos.current = { x: 0, y: 0 };
      drag.current = null;
      el.style.transform = "";
      el.classList.remove("grab");
    });
    resize.observe(el.parentElement);
    return () => resize.disconnect();
  }, []);

  const onDown = (e: React.PointerEvent) => {
    const el = ref.current;
    if (!el || !window.matchMedia("(min-width: 761px)").matches) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    el.style.zIndex = String(++zTop);
    el.classList.add("grab");
    drag.current = { dx: e.clientX - pos.current.x, dy: e.clientY - pos.current.y };
  };
  const onMove = (e: React.PointerEvent) => {
    const el = ref.current;
    if (!el || !drag.current) return;
    const stage = el.parentElement;
    if (!stage) return;
    const gutter = parseFloat(getComputedStyle(stage).paddingLeft);
    let nx = e.clientX - drag.current.dx;
    let ny = e.clientY - drag.current.dy;
    nx = Math.max(gutter - el.offsetLeft, Math.min(nx, stage.clientWidth - gutter - el.offsetLeft - el.offsetWidth));
    ny = Math.max(gutter - el.offsetTop, Math.min(ny, stage.clientHeight - gutter - el.offsetTop - el.offsetHeight));
    pos.current = { x: nx, y: ny };
    el.style.transform = `translate(${nx}px, ${ny}px)`;
  };
  const onUp = () => {
    drag.current = null;
    ref.current?.classList.remove("grab");
  };

  return (
    <div className={"win" + (className ? " " + className : "")} ref={ref}>
      <div
        className="win-bar"
        onPointerDown={onDown}
        onPointerMove={onMove}
        onPointerUp={onUp}
        onPointerCancel={onUp}
        onLostPointerCapture={onUp}
      >
        <span className="win-x" />
        <span className="win-t">{title}</span>
        <span className="win-x" />
      </div>
      <div className="win-body">{children}</div>
    </div>
  );
}

export default function Desktop() {
  return (
    <section className="desk" id="desktop">
      <div className="desk-h">
        <span className="lbl">snapshotdb.os</span>
        <h2>Your whole database, on one server you run.</h2>
        <p>A live replica of production plus every branch off it — each a real database with its own port and generated credentials. Drag the windows.</p>
      </div>
      <div className="desk-stage">
        <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />

        <Win title="prod — source">
          <div className="chips"><span className="chip pg">postgres</span><span className="chip st">● in sync</span></div>
          <dl className="kv">
            <div><dt>stream</dt><dd>0 bytes behind</dd></div>
            <div><dt>wal slot</dt><dd>reserved</dd></div>
          </dl>
        </Win>

        <Win title="branches (3)">
          <ul className="brs">
            <li><span className="led run" /> dev<span className="port">:55036</span></li>
            <li><span className="led run" /> staging<span className="port">:55051</span></li>
            <li><span className="led run" /> agent-1<span className="port">:55084</span></li>
          </ul>
        </Win>

        <Win title="clock" className="win-clock">
          <LiveClock />
        </Win>

        <Win title="engines">
          <div className="chips">
            <span className="chip">postgres</span><span className="chip">mysql</span>
            <span className="chip">mongodb</span><span className="chip">sqlite</span>
          </div>
          <div className="engnote">branched the same way</div>
        </Win>

        <Win title="new branch" className="win-new">
          <div className="cmd">$ snapshotdb create fix-orders --from prod</div>
          <div className="ok">✓ ready in 0.5s · its own url</div>
        </Win>
      </div>
    </section>
  );
}
