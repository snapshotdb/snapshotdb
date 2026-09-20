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
    const faces = [[0, 1, 2, 3], [4, 5, 6, 7], [0, 1, 5, 4],
      [3, 2, 6, 7], [0, 3, 7, 4], [1, 2, 6, 5]];
    const cells = Array.from({ length: 27 }, (_, i) => [i % 3 - 1, Math.floor(i / 3) % 3 - 1, Math.floor(i / 9) - 1]);

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
      const scale = Math.min(W * 0.19, H * 0.25);
      const project = (p: number[]) => {
        const f = 6 / (6 - p[2]);
        return [W / 2 + p[0] * scale * f, H / 2 + p[1] * scale * f, p[2]];
      };
      const stroke = (a: number[], b: number[], color: string, width = 1) => {
        ctx.strokeStyle = color;
        ctx.lineWidth = width;
        ctx.beginPath(); ctx.moveTo(a[0], a[1]); ctx.lineTo(b[0], b[1]); ctx.stroke();
      };
      const split = (1 - Math.cos(time * Math.PI / 6)) / 2;
      const spread = 0.2 + split * 0.8;
      const source = [0, -0.82, 0];
      const destinations = [[-1.72, 0.82, 0], [0, 1.04, 0.45], [1.72, 0.82, 0]];
      const core = project(source);
      const aura = ctx.createRadialGradient(core[0], core[1], 0, core[0], core[1], scale * 1.9);
      aura.addColorStop(0, "rgba(229,83,63,0.12)");
      aura.addColorStop(1, "rgba(229,83,63,0)");
      ctx.fillStyle = aura;
      ctx.fillRect(0, 0, W, H);
      const clusters = [{ center: source, size: 1, alpha: 1, branch: false, index: 0 },
        ...destinations.map((target, i) => ({
          center: source.map((v, j) => v + (target[j] - v) * spread),
          size: 0.54, alpha: 0.18 + split * 0.72, branch: true, index: i + 1,
        }))];
      // Packets follow the same curve as the tether; no detached decoration.
      for (const cluster of clusters.slice(1)) {
        const end = project(cluster.center);
        const control = [end[0], core[1] + (end[1] - core[1]) * 0.25];
        ctx.strokeStyle = `rgba(229,83,63,${cluster.alpha * 0.28})`;
        ctx.lineWidth = 1;
        ctx.beginPath(); ctx.moveTo(core[0], core[1]);
        ctx.quadraticCurveTo(control[0], control[1], end[0], end[1]); ctx.stroke();
        for (let packet = 0; packet < 2; packet++) {
          const t = (time * 0.32 + cluster.index * 0.2 + packet * 0.5) % 1;
          const x = (1 - t) ** 2 * core[0] + 2 * (1 - t) * t * control[0] + t * t * end[0];
          const y = (1 - t) ** 2 * core[1] + 2 * (1 - t) * t * control[1] + t * t * end[1];
          ctx.shadowBlur = 10; ctx.shadowColor = "#e5533f";
          ctx.fillStyle = `rgba(255,142,104,${cluster.alpha})`;
          ctx.fillRect(x - 1.5, y - 1.5, 3, 3);
          ctx.shadowBlur = 0;
        }
      }
      for (const cluster of clusters) {
        const turn = ay + cluster.index * 0.65;
        const gap = 0.37 + Math.sin(time * 0.52) ** 2 * 0.075;
        const transform = (p: number[]) => {
          const q = rotX(rotY(p, turn), ax);
          return project(q.map((v, j) => v * cluster.size + cluster.center[j]));
        };
        const voxels = cells.map((cell) => {
          const center = cell.map(v => v * gap);
          const h = 0.145;
          const points = [[-h,-h,-h],[h,-h,-h],[h,h,-h],[-h,h,-h],
            [-h,-h,h],[h,-h,h],[h,h,h],[-h,h,h]].map(p => transform(p.map((v,j) => v + center[j])));
          return { points, depth: transform(center)[2], hot: Math.exp(-18 * (center[1] - Math.sin(time * 0.9) * 0.57) ** 2) };
        }).sort((a, b) => a.depth - b.depth);
        for (const { points, hot } of voxels) {
          const alpha = cluster.alpha;
          const heat = cluster.branch ? 0.8 : hot;
          const tone = [239 - heat * 10, 233 - heat * 133, 223 - heat * 151];
          const sortedFaces = faces.map(face => ({ face,
            depth: face.reduce((sum, i) => sum + points[i][2], 0) / 4,
          })).sort((a, b) => a.depth - b.depth);
          for (const { face } of sortedFaces) {
            ctx.beginPath();
            face.forEach((idx, j) => j ? ctx.lineTo(points[idx][0], points[idx][1]) : ctx.moveTo(points[idx][0], points[idx][1]));
            ctx.closePath();
            ctx.fillStyle = `rgba(${16 + heat * 24},${14 + heat * 5},12,${alpha * 0.95})`;
            ctx.fill();
            ctx.strokeStyle = `rgba(${tone.join(",")},${alpha * (0.28 + heat * 0.22)})`;
            ctx.lineWidth = 0.8;
            ctx.stroke();
          }
        }
        // A luminous perimeter scans the source continuously from top to bottom.
        if (!cluster.branch) {
          const scanY = Math.sin(time * 0.9) * 0.57;
          const ring = [[-0.64,scanY,-0.64],[0.64,scanY,-0.64],[0.64,scanY,0.64],[-0.64,scanY,0.64]].map(transform);
          ctx.shadowBlur = 14; ctx.shadowColor = "#e5533f";
          ring.forEach((p, i) => stroke(p, ring[(i + 1) % 4], "rgba(255,128,86,0.85)", 1.5));
          ctx.shadowBlur = 0;
        }
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
      draw(elapsed, tiltX, 0.65 + elapsed * 0.16 + tiltY);
      raf = requestAnimationFrame(loop);
    };
    const ro = new ResizeObserver(() => {
      size();
      draw(reduce ? 5 : elapsed, tiltX, 0.65 + elapsed * 0.16 + tiltY);
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
