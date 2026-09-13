"use client";
import { useEffect } from "react";

export default function Effects() {
  useEffect(() => {
    const NS = "http://www.w3.org/2000/svg";
    const XL = "http://www.w3.org/1999/xlink";
    const svg = document.getElementById("organism");
    if (!svg) return;
    const ruler = document.getElementById("ruler") as unknown as SVGGElement;
    const branchLayer = document.getElementById("branchlayer") as unknown as SVGGElement;
    const particles = document.getElementById("particles") as unknown as SVGGElement;
    const sprouts = document.getElementById("sprouts") as unknown as SVGGElement;
    [ruler, branchLayer, particles, sprouts].forEach((g) => g && (g.innerHTML = ""));

    const line = (x1: number, y1: number, x2: number, y2: number, cls = "rl", op = "1") => {
      const l = document.createElementNS(NS, "line");
      l.setAttribute("x1", String(x1)); l.setAttribute("y1", String(y1)); l.setAttribute("x2", String(x2)); l.setAttribute("y2", String(y2));
      l.setAttribute("class", cls); l.setAttribute("opacity", op); return l;
    };
    const text = (x: number, y: number, s: string, cls: string) => {
      const t = document.createElementNS(NS, "text");
      t.setAttribute("x", String(x)); t.setAttribute("y", String(y)); t.setAttribute("class", cls); t.textContent = s; return t;
    };

    // measurement ruler along the bottom + PROD crosshair — instrument feel
    for (let x = 96; x <= 600; x += 28) {
      const major = (x - 96) % 112 === 0;
      ruler.appendChild(line(x, 432, x, major ? 424 : 428, "rl", "0.5"));
      if (major) ruler.appendChild(text(x - 3, 444, String((x - 96) / 28), "rlab"));
    }
    ruler.appendChild(line(96, 432, 600, 432, "rl", "0.35"));
    ruler.appendChild(line(78, 230, 114, 230, "rl", "0.4"));
    ruler.appendChild(line(96, 212, 96, 248, "rl", "0.4"));

    // branches fork up and down off the replica rail, each with a latency readout
    const branches = [
      { bx: 150, ex: 214, ey: 150, lat: "0.21s" },
      { bx: 212, ex: 292, ey: 316, lat: "0.34s" },
      { bx: 286, ex: 360, ey: 130, lat: "0.19s" },
      { bx: 352, ex: 440, ey: 330, lat: "0.23s" },
      { bx: 420, ex: 500, ey: 166, lat: "0.28s" },
    ];
    const Pd = (bx: number, ex: number, ey: number) => `M${bx} 230 C ${(bx + ex) / 2} 230, ${bx + 18} ${ey}, ${ex} ${ey}`;
    const pathLen = (d: string) => { const t = document.createElementNS(NS, "path"); t.setAttribute("d", d); return t.getTotalLength(); };

    branches.forEach((b, i) => {
      const { bx, ex, ey } = b;
      const up = ey < 230;
      const delay = 1.2 + i * 0.32;
      const tick = document.createElementNS(NS, "circle");
      tick.setAttribute("cx", String(bx)); tick.setAttribute("cy", "230"); tick.setAttribute("r", "2.6");
      tick.setAttribute("fill", "#0b0a09"); tick.setAttribute("stroke", "#efe9df"); tick.setAttribute("stroke-width", "1.6");
      tick.setAttribute("class", "node"); (tick as any).style.animationDelay = delay - 0.15 + "s";
      branchLayer.appendChild(tick);
      const d = Pd(bx, ex, ey); const L = Math.ceil(pathLen(d));
      const p = document.createElementNS(NS, "path");
      p.setAttribute("d", d); p.setAttribute("fill", "none"); p.setAttribute("stroke", "#efe9df");
      p.setAttribute("stroke-width", "1.5"); p.setAttribute("stroke-linecap", "round"); p.setAttribute("opacity", (0.9 - i * 0.05).toFixed(2));
      p.style.strokeDasharray = String(L); p.style.strokeDashoffset = String(L);
      p.style.animation = `draw 1.1s cubic-bezier(.6,0,.35,1) ${delay}s forwards`;
      branchLayer.appendChild(p);
      const g = document.createElementNS(NS, "g"); g.setAttribute("class", "node drift"); (g as any).style.animationDelay = delay + 1 + "s";
      const dot = document.createElementNS(NS, "circle");
      dot.setAttribute("cx", String(ex)); dot.setAttribute("cy", String(ey)); dot.setAttribute("r", "6.5");
      dot.setAttribute("fill", "#0b0a09"); dot.setAttribute("stroke", "#efe9df"); dot.setAttribute("stroke-width", "1.7");
      const core = document.createElementNS(NS, "circle");
      core.setAttribute("cx", String(ex)); core.setAttribute("cy", String(ey)); core.setAttribute("r", "2.2"); core.setAttribute("fill", "#fbf7ef");
      core.setAttribute("class", "pulse"); (core as any).style.animationDelay = i * 0.5 + "s";
      g.appendChild(dot); g.appendChild(core); branchLayer.appendChild(g);
      const ly = up ? ey - 15 : ey + 22;
      const lab = text(ex - 8, ly, "branch/" + (i + 1) + "  ·  " + b.lat, "anno"); (lab as any).style.animationDelay = delay + 1.2 + "s";
      branchLayer.appendChild(lab);
    });

    // replication particles along the spine
    for (let i = 0; i < 4; i++) {
      const c = document.createElementNS(NS, "circle");
      c.setAttribute("r", "2"); c.setAttribute("fill", "#fbf7ef"); c.setAttribute("opacity", "0.9");
      const am = document.createElementNS(NS, "animateMotion");
      am.setAttribute("dur", "3.6s"); am.setAttribute("repeatCount", "indefinite"); am.setAttribute("begin", i * 0.9 + "s");
      const mp = document.createElementNS(NS, "mpath"); mp.setAttributeNS(XL, "href", "#spine");
      am.appendChild(mp); c.appendChild(am); particles.appendChild(c);
    }

    // live telemetry readout (top-right of the panel)
    const readout = document.getElementById("readout");
    const setReadout = (s: string) => { if (readout && readout.childNodes[1]) readout.childNodes[1].nodeValue = s; };
    const states = ["streaming", "0.2 ms behind", "in sync", "streaming", "0.1 ms behind"];
    let ri = 0;
    const roTimer = window.setInterval(() => { ri = (ri + 1) % states.length; setReadout(states[ri]); }, 1900);

    // a branch sprouts, then fades; figr counter tracks it
    const figr = document.getElementById("figr");
    let sproutN = 5, stopped = false;
    function sprout() {
      if (stopped) return;
      const bx = 190 + Math.random() * 300, dir = Math.random() < 0.5 ? -1 : 1;
      const ex = bx + 70 + Math.random() * 90, ey = 230 + dir * (80 + Math.random() * 80);
      const d = Pd(bx, ex, ey);
      const g = document.createElementNS(NS, "g");
      const p = document.createElementNS(NS, "path");
      p.setAttribute("d", d); p.setAttribute("fill", "none"); p.setAttribute("stroke", "#efe9df"); p.setAttribute("stroke-width", "1.3"); p.setAttribute("stroke-linecap", "round");
      p.style.cssText = "stroke-dasharray:260;stroke-dashoffset:260;animation:sd 2.6s cubic-bezier(.6,0,.35,1) forwards;opacity:.7";
      const t = document.createElementNS(NS, "circle");
      t.setAttribute("cx", String(ex)); t.setAttribute("cy", String(ey)); t.setAttribute("r", "5"); t.setAttribute("fill", "#0b0a09"); t.setAttribute("stroke", "#fbf7ef"); t.setAttribute("stroke-width", "1.7");
      t.style.cssText = "opacity:0;transform-box:fill-box;transform-origin:center;animation:st 2.6s ease forwards";
      g.appendChild(p); g.appendChild(t); sprouts.appendChild(g);
      sproutN++; if (figr) figr.textContent = "nodes: 1 · branches: " + sproutN;
      window.setTimeout(() => {
        g.style.transition = "opacity 1s"; g.style.opacity = "0";
        window.setTimeout(() => { g.remove(); sproutN--; if (figr) figr.textContent = "nodes: 1 · branches: " + sproutN; }, 1000);
      }, 2800);
    }
    const style = document.createElement("style");
    style.textContent = "@keyframes sd{to{stroke-dashoffset:0}}@keyframes st{0%,55%{opacity:0;transform:scale(0)}78%{opacity:1;transform:scale(1.2)}100%{opacity:.9;transform:scale(1)}}";
    document.head.appendChild(style);
    const t0 = window.setTimeout(() => sprout(), 5400);
    const sproutTimer = window.setInterval(sprout, 4600);

    // parallax on the schematic panel
    const right = svg.closest(".right") as HTMLElement | null;
    (svg as unknown as HTMLElement).style.transition = "transform .6s cubic-bezier(.2,.7,.2,1)";
    const onMove = (e: MouseEvent) => {
      const r = right!.getBoundingClientRect();
      const dx = (e.clientX - r.left) / r.width - 0.5, dy = (e.clientY - r.top) / r.height - 0.5;
      (svg as unknown as HTMLElement).style.transform = `translate(${dx * 16}px,${dy * 11}px)`;
    };
    const onLeave = () => ((svg as unknown as HTMLElement).style.transform = "translate(0,0)");
    right?.addEventListener("mousemove", onMove);
    right?.addEventListener("mouseleave", onLeave);

    // phase ticker
    const words = ["opens", "diverges", "isolates", "collects", "resets"];
    let pi = 0; const pe = document.getElementById("phase");
    const phaseTimer = window.setInterval(() => {
      pi = (pi + 1) % words.length; if (!pe) return;
      pe.style.opacity = "0";
      window.setTimeout(() => { pe.textContent = words[pi]; pe.style.transition = ".3s"; pe.style.opacity = "1"; }, 250);
    }, 2200);

    // scroll reveals
    const io = new IntersectionObserver(
      (es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } }),
      { threshold: 0.16 }
    );
    document.querySelectorAll(".reveal").forEach((n, i) => { (n as HTMLElement).style.transitionDelay = (i % 4) * 0.07 + "s"; io.observe(n); });

    return () => {
      stopped = true;
      window.clearTimeout(t0); window.clearInterval(sproutTimer); window.clearInterval(phaseTimer); window.clearInterval(roTimer);
      right?.removeEventListener("mousemove", onMove);
      right?.removeEventListener("mouseleave", onLeave);
      io.disconnect(); style.remove();
    };
  }, []);
  return null;
}
