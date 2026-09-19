"use client";

import { useEffect } from "react";

// Reliable scroll-reveal: adds `.in` to every `.reveal` element as it enters the
// viewport. Belt-and-suspenders alongside Effects' IntersectionObserver.
export default function Reveal() {
  useEffect(() => {
    const els = Array.from(document.querySelectorAll<HTMLElement>(".reveal"));
    if (!els.length) return;
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) { els.forEach((el) => el.classList.add("in")); return; }
    const check = () => {
      let remaining = false;
      for (const el of els) {
        if (el.classList.contains("in")) continue;
        const r = el.getBoundingClientRect();
        if (r.top < window.innerHeight * 0.9 && r.bottom > 0) el.classList.add("in");
        else remaining = true;
      }
      if (!remaining) {
        window.removeEventListener("scroll", check);
        window.removeEventListener("resize", check);
      }
    };
    window.addEventListener("scroll", check, { passive: true });
    window.addEventListener("resize", check);
    check();
    return () => {
      window.removeEventListener("scroll", check);
      window.removeEventListener("resize", check);
    };
  }, []);
  return null;
}
