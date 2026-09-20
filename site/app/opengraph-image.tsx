import { ImageResponse } from "next/og";
import Logo from "./Logo";

export const size = { width: 1200, height: 630 };
export const contentType = "image/png";
export const alt = "SnapshotDB — branch your database like code";

export default function OG() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%", height: "100%", display: "flex", flexDirection: "column", justifyContent: "space-between",
          background: "#0b0a09", color: "#efe9df", padding: 72, fontFamily: "sans-serif",
          backgroundImage:
            "linear-gradient(#221f1b 1px, transparent 1px), linear-gradient(90deg, #221f1b 1px, transparent 1px)",
          backgroundSize: "48px 48px",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 16, fontSize: 26, letterSpacing: -1 }}>
          <Logo size={36} fill="#efe9df" />
          <div style={{ display: "flex" }}>
            <span style={{ fontWeight: 700 }}>snapshot</span>
            <span style={{ color: "#8f887c" }}>db</span>
          </div>
          <div style={{ marginLeft: 8, fontSize: 15, color: "#8f887c", border: "1px solid #302b25", padding: "3px 9px", letterSpacing: 3, fontFamily: "monospace" }}>OSS</div>
        </div>
        <div style={{ display: "flex", flexDirection: "column" }}>
          <div style={{ fontSize: 92, fontWeight: 700, letterSpacing: -3, lineHeight: 1 }}>Branch your database</div>
          <div style={{ fontSize: 92, fontWeight: 700, letterSpacing: -3, lineHeight: 1, color: "#8f887c" }}>like code.</div>
          <div style={{ marginTop: 30, fontSize: 28, color: "#bdb6a8", maxWidth: 900 }}>
            An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own.
          </div>
        </div>
        <div style={{ display: "flex", gap: 28, fontSize: 20, color: "#8f887c", fontFamily: "monospace" }}>
          <span>~1s · 1&nbsp;TB branch</span>
          <span>postgres · mysql · mongo · sqlite</span>
          <span>apache-2.0</span>
        </div>
      </div>
    ),
    size
  );
}
