"use client";

import { useEffect, useRef } from "react";

const NAMES = ["dev", "staging", "agent-1", "ci-8f2", "migrate-y", "hotfix", "review-42", "load-test", "preview", "sandbox"];
const PHASES = ["opens", "diverges", "works", "collects"];

// Hero figure: a live replica spine with a streaming pulse, and branches that
// fork off on a clean rhythm — alternating up/down, each growing to a labelled
// node, the newest in amber, the oldest rolling off so it never clutters.
export default function HeroFig() {
  const cvRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const cv = cvRef.current;
    if (!cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const monoVar = getComputedStyle(document.documentElement).getPropertyValue("--f-mono").trim();
    const MONO = (monoVar ? monoVar + "," : "") + "ui-monospace, monospace";

    let W = 0, H = 0;
    const DPR = Math.min(window.devicePixelRatio || 1, 2);
    const size = () => {
      const r = cv.getBoundingClientRect();
      W = r.width; H = r.height;
      cv.width = Math.max(1, Math.round(W * DPR));
      cv.height = Math.max(1, Math.round(H * DPR));
      ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
    };
    size();
    const ro = new ResizeObserver(size);
    ro.observe(cv);

    const bone = (a: number) => `rgba(239,233,223,${a})`;
    const amber = (a: number) => `rgba(229,83,63,${a})`;
    const wrap = (x: number) => ((x % 1) + 1) % 1;
    const TAU = Math.PI * 2;

    const N = 5;
    const CYCLE = 6600, GROW = 640, FADE = 520, GAP = 1000;
    const labelFor = (slot: number, cyc: number) => NAMES[(slot * 3 + cyc * 2) % NAMES.length];
    const latFor = (slot: number, cyc: number) => (0.14 + ((slot * 7 + cyc * 5) % 22) / 100);

    const t0 = performance.now();

    const draw = (now: number) => {
      const T = now - t0;
      ctx.clearRect(0, 0, W, H);

      const cy = H * 0.5;
      const padL = W * 0.17, padR = W * 0.9;
      const spineW = padR - padL;
      const stemH = Math.min(H * 0.26, 118);

      ctx.save();
      ctx.lineCap = "round";

      // spine + ticks
      ctx.strokeStyle = bone(0.22); ctx.lineWidth = 1; ctx.setLineDash([]);
      ctx.beginPath(); ctx.moveTo(padL, cy); ctx.lineTo(padR, cy); ctx.stroke();
      ctx.strokeStyle = bone(0.1);
      for (let x = padL; x <= padR + 0.5; x += spineW / 16) {
        ctx.beginPath(); ctx.moveTo(x, cy - 3.5); ctx.lineTo(x, cy + 3.5); ctx.stroke();
      }

      // branches
      for (let i = 0; i < N; i++) {
        const dir = i % 2 === 0 ? -1 : 1;
        const phase = (i / N) * CYCLE;
        const local = (T + phase) % CYCLE;
        const cyc = Math.floor((T + phase) / CYCLE);
        const aliveEnd = CYCLE - FADE - GAP;
        let prog = 0, op = 0, growing = false;
        if (local < GROW) { prog = local / GROW; op = Math.min(1, prog * 1.4); growing = true; }
        else if (local < aliveEnd) { prog = 1; op = 1; }
        else if (local < aliveEnd + FADE) { prog = 1; op = 1 - (local - aliveEnd) / FADE; }
        else continue;
        if (op <= 0.01) continue;

        const ease = 1 - Math.pow(1 - prog, 3);
        const sx = padL + spineW * ((i + 1) / (N + 1));
        const ny = cy + dir * stemH * ease;
        const col = growing ? amber : bone;

        // junction on the spine
        ctx.fillStyle = bone(0.55 * op);
        ctx.beginPath(); ctx.arc(sx, cy, 2, 0, TAU); ctx.fill();
        // stem
        ctx.strokeStyle = col(0.5 * op); ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(sx, cy); ctx.lineTo(sx, ny); ctx.stroke();

        if (ease > 0.55) {
          const na = op;
          // node
          ctx.strokeStyle = col(0.9 * op); ctx.lineWidth = 1.4;
          ctx.beginPath(); ctx.arc(sx, ny, 4.5, 0, TAU); ctx.stroke();
          ctx.fillStyle = col(0.9 * na); ctx.beginPath(); ctx.arc(sx, ny, 1.7, 0, TAU); ctx.fill();
          // label + latency
          ctx.font = "10px " + MONO;
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          const ty = ny + dir * 15;
          ctx.fillStyle = bone(0.5 * op);
          ctx.fillText(labelFor(i, cyc) + "  ·  " + latFor(i, cyc).toFixed(2) + "s", sx, ty);
        }
      }

      // replica core + expanding ring
      const ring = (T / 1600) % 1;
      ctx.strokeStyle = bone(0.3 * (1 - ring)); ctx.lineWidth = 1;
      ctx.beginPath(); ctx.arc(padL, cy, 6 + 12 * ring, 0, TAU); ctx.stroke();
      ctx.fillStyle = bone(0.95); ctx.beginPath(); ctx.arc(padL, cy, 5, 0, TAU); ctx.fill();
      ctx.fillStyle = "#0b0a09"; ctx.beginPath(); ctx.arc(padL, cy, 1.8, 0, TAU); ctx.fill();
      ctx.font = "10px " + MONO; ctx.textAlign = "left"; ctx.textBaseline = "middle";
      ctx.fillStyle = bone(0.42); ctx.fillText("replica", padL - 3, cy + 22);

      // streaming pulse + trail along the spine
      const head = wrap(T / 2600);
      for (let k = 6; k >= 0; k--) {
        const p = wrap((T - k * 46) / 2600);
        const a = k === 0 ? 0.95 : 0.12 * (1 - k / 7);
        ctx.fillStyle = amber(a);
        ctx.beginPath(); ctx.arc(padL + spineW * p, cy, k === 0 ? 2.6 : 2, 0, TAU); ctx.fill();
      }
      void head;

      ctx.restore();
    };

    let raf = 0;
    const loop = (now: number) => { draw(now); raf = requestAnimationFrame(loop); };
    if (reduce) draw(t0 + 2800);
    else raf = requestAnimationFrame(loop);

    // phase ticker in the left column status line
    const phaseEl = document.getElementById("phase");
    let pi = 0, ptimer = 0;
    if (phaseEl && !reduce) {
      ptimer = window.setInterval(() => {
        pi = (pi + 1) % PHASES.length;
        phaseEl.style.transition = ".3s";
        phaseEl.style.opacity = "0";
        window.setTimeout(() => { phaseEl.textContent = PHASES[pi]; phaseEl.style.opacity = "1"; }, 260);
      }, 2400);
    }

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      window.clearInterval(ptimer);
    };
  }, []);

  return (
    <div className="right">
      <div className="fig">fig.01 — copy-on-write branching</div>
      <div className="figtr"><span className="b" />streaming</div>
      <div className="figr">5 branches · 1 replica</div>
      <canvas className="figcanvas" ref={cvRef} aria-hidden="true" />
    </div>
  );
}
