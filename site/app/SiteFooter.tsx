"use client";

import { useEffect, useRef } from "react";
import Logo from "./Logo";

const GH = "https://github.com/snapshotdb/snapshotdb";

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
    const draw = (time: number, ax: number, ay: number) => {
      if (!W || !H) return;
      ctx.clearRect(0, 0, W, H);
      const scale = Math.min(W * 0.22, H * 0.23), cd = 5.4;
      const twist = 0.14 * Math.sin(time * 0.42);
      const P = V.map((p, i) => {
        // Opposing top/bottom rotation gently skews the original wireframe.
        const warped = rotY(p, p[1] * twist + (i >= 8 ? Math.sin(time * 0.3) * 0.18 : 0));
        const q = rotX(rotY(warped, ay), ax);
        const f = cd / (cd - q[2]);
        return [W / 2 + q[0] * scale * f, H / 2 + q[1] * scale * f, q[2]];
      });
      ctx.lineWidth = 1.1;
      ctx.setLineDash([3, 5]);
      for (const [i, j] of E) {
        const a = P[i], b = P[j];
        const depth = Math.max(0, Math.min(1, ((a[2] + b[2]) / 2 + 2.1) / 4.2));
        ctx.strokeStyle = `rgba(239,233,223,${0.16 + 0.54 * depth})`;
        ctx.beginPath();
        ctx.moveTo(a[0], a[1]);
        ctx.lineTo(b[0], b[1]);
        ctx.stroke();
      }
    };

    let mx = 0, my = 0, visible = false, raf = 0, last = 0, elapsed = 0;
    let tiltX = -0.36, tiltY = 0;
    const onMove = (e: PointerEvent) => {
      const bounds = cv.getBoundingClientRect();
      mx = Math.max(-0.5, Math.min(0.5, (e.clientX - bounds.left) / bounds.width - 0.5));
      my = Math.max(-0.5, Math.min(0.5, (e.clientY - bounds.top) / bounds.height - 0.5));
    };
    cv.parentElement?.addEventListener("pointermove", onMove, { passive: true });
    const loop = (now: number) => {
      const dt = last ? Math.min((now - last) / 1000, 0.05) : 0;
      last = now;
      elapsed += dt;
      const ease = 1 - Math.exp(-dt * 5);
      tiltX += ((-0.36 + my * 0.22) - tiltX) * ease;
      tiltY += ((mx * 0.25) - tiltY) * ease;
      draw(elapsed, tiltX, 0.65 + elapsed * 0.12 + tiltY);
      raf = requestAnimationFrame(loop);
    };
    const ro = new ResizeObserver(() => {
      size();
      draw(reduce ? 5 : elapsed, tiltX, 0.65 + elapsed * 0.12 + tiltY);
    });
    ro.observe(cv);
    const io = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      cancelAnimationFrame(raf);
      last = 0;
      if (visible && !reduce) raf = requestAnimationFrame(loop);
    });
    io.observe(cv);
    size();
    draw(reduce ? 5 : 0, tiltX, 0.65);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      cv.parentElement?.removeEventListener("pointermove", onMove);
    };
  }, []);

  return (
    <footer className="sfoot">
      <span className="sf-reg tl" /><span className="sf-reg tr" />
      <span className="sf-reg bl" /><span className="sf-reg br" />

      <div className="sf-stage">
        <canvas ref={ref} aria-hidden="true" />
        <div className="sf-word">
          <div className="sf-lockup">
            <Logo fill="var(--white)" />
            <span className="w">snapshot<em>db</em></span>
          </div>
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
