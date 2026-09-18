"use client";

import { useEffect, useRef } from "react";

const GH = "https://github.com/GitHoobar/anybranch";

export default function SiteFooter() {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const ctx = cv.getContext("2d");
    if (!ctx) return;

    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const rotY = (p: number[], a: number): number[] => {
      const c = Math.cos(a), s = Math.sin(a);
      return [p[0] * c - p[2] * s, p[1], p[0] * s + p[2] * c];
    };
    const rotX = (p: number[], a: number): number[] => {
      const c = Math.cos(a), s = Math.sin(a);
      return [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c];
    };
    // A twisted nested cube (hypercube projection) — an outer box with a
    // rotated inner box, joined corner-to-corner. Richer than a single cuboid.
    const HX = 1.55, HY = 1.14, HZ = 1.14;
    const outer: number[][] = [
      [-HX, -HY, -HZ], [HX, -HY, -HZ], [HX, HY, -HZ], [-HX, HY, -HZ],
      [-HX, -HY, HZ], [HX, -HY, HZ], [HX, HY, HZ], [-HX, HY, HZ],
    ];
    const s = 0.58;
    const inner: number[][] = [
      [-s, -s, -s], [s, -s, -s], [s, s, -s], [-s, s, -s],
      [-s, -s, s], [s, -s, s], [s, s, s], [-s, s, s],
    ].map((p) => rotX(rotY(p, 0.52), 0.34));
    const V: number[][] = outer.concat(inner);
    const face = (o: number): number[][] => [
      [o, o + 1], [o + 1, o + 2], [o + 2, o + 3], [o + 3, o],
      [o + 4, o + 5], [o + 5, o + 6], [o + 6, o + 7], [o + 7, o + 4],
      [o, o + 4], [o + 1, o + 5], [o + 2, o + 6], [o + 3, o + 7],
    ];
    const E: number[][] = face(0).concat(
      face(8),
      [0, 1, 2, 3, 4, 5, 6, 7].map((i): number[] => [i, i + 8]),
    );

    let W = 0, H = 0;
    const DPR = Math.min(window.devicePixelRatio || 1, 2);
    const size = () => {
      const r = cv.getBoundingClientRect();
      W = r.width; H = r.height;
      cv.width = Math.max(1, Math.round(W * DPR));
      cv.height = Math.max(1, Math.round(H * DPR));
      ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
    };
    const draw = (ax: number, ay: number) => {
      if (!W || !H) return;
      ctx.clearRect(0, 0, W, H);
      const scale = Math.min(W, H) * 0.36, cd = 5.4;
      const P = V.map((p) => {
        const q = rotX(rotY(p, ay), ax);
        const f = cd / (cd - q[2]);
        return [W / 2 + q[0] * scale * f, H / 2 + q[1] * scale * f, q[2]];
      });
      ctx.lineWidth = 1.1;
      ctx.setLineDash([3, 5]);
      for (const [i, j] of E) {
        const a = P[i], b = P[j];
        let t = ((a[2] + b[2]) / 2 + 2.1) / 4.2;
        t = t < 0 ? 0 : t > 1 ? 1 : t;
        ctx.strokeStyle = `rgba(239,233,223,${(0.16 + 0.64 * t).toFixed(3)})`;
        ctx.beginPath();
        ctx.moveTo(a[0], a[1]);
        ctx.lineTo(b[0], b[1]);
        ctx.stroke();
      }
    };

    let mx = 0, my = 0;
    const onMove = (e: PointerEvent) => {
      mx = e.clientX / window.innerWidth - 0.5;
      my = e.clientY / window.innerHeight - 0.5;
    };
    window.addEventListener("pointermove", onMove, { passive: true });

    const ro = new ResizeObserver(() => {
      size();
      if (reduce) draw(-0.3, 0.7);
    });
    ro.observe(cv);
    size();

    let raf = 0, spin = 0.7, tiltX = -0.3, tiltY = 0;
    if (reduce) {
      draw(-0.3, 0.7);
    } else {
      const loop = () => {
        spin += 0.002;
        tiltX += ((-0.3 + my * 0.5) - tiltX) * 0.06;
        tiltY += ((mx * 0.7) - tiltY) * 0.06;
        draw(tiltX, spin + tiltY);
        raf = requestAnimationFrame(loop);
      };
      raf = requestAnimationFrame(loop);
    }

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      window.removeEventListener("pointermove", onMove);
    };
  }, []);

  return (
    <footer className="sfoot">
      <span className="sf-reg tl" /><span className="sf-reg tr" />
      <span className="sf-reg bl" /><span className="sf-reg br" />

      <div className="sf-stage">
        <canvas ref={ref} aria-hidden="true" />
        <div className="sf-word">
          <svg viewBox="0 0 32 32" fill="none" stroke="var(--white)" strokeWidth="2.2" strokeLinecap="round" aria-hidden="true">
            <circle cx="9" cy="7" r="3" /><circle cx="9" cy="25" r="3" /><circle cx="23" cy="16" r="3" />
            <path d="M9 10v12M9 16h4a6 6 0 0 0 6-6" />
          </svg>
          <span className="w">snapshot<em>db</em></span>
        </div>
      </div>

      <div className="sf-bar">
        <span className="c">snapshotdb © 2026</span>
        <span className="mid"><a href="/docs">Docs</a><a href={`${GH}/blob/main/LICENSE`}>Apache-2.0</a></span>
        <span className="right"><a href={GH}>GitHub</a></span>
      </div>
    </footer>
  );
}
