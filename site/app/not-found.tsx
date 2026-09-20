import Link from "next/link";
export const metadata = { title: "Not found" };

export default function NotFound() {
  return (
    <div className="frame" style={{ minHeight: "100vh", display: "flex", alignItems: "center", justifyContent: "center" }}>
      <span className="tick tl" /><span className="tick tr" /><span className="tick bl" /><span className="tick br" />
      <div style={{ textAlign: "center", padding: 40 }}>
        <div className="lbl" style={{ marginBottom: 14 }}>ERROR · 404</div>
        <h1 style={{ fontSize: 52, fontWeight: 600, letterSpacing: "-.03em" }}>Branch not found.</h1>
        <p style={{ color: "var(--muted)", margin: "14px 0 28px" }}>That page was reset, dropped, or never opened.</p>
        <div style={{ display: "flex", gap: 12, justifyContent: "center" }}>
          <Link className="btn solid" href="/">← Home</Link>
          <Link className="btn" href="/docs">Docs</Link>
        </div>
      </div>
    </div>
  );
}
