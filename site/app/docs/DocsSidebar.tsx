"use client";
import { usePathname } from "next/navigation";

const groups: { title: string; links: [string, string][] }[] = [
  { title: "Getting started", links: [["/docs", "Overview"], ["/docs/quickstart", "Quickstart"], ["/docs/install", "Install"], ["/docs/server", "Server deployment"]] },
  { title: "Connect a database", links: [["/docs/connectors", "Overview"], ["/docs/connectors/postgres", "Self-hosted Postgres"], ["/docs/connectors/rds", "AWS RDS"], ["/docs/connectors/supabase", "Supabase"], ["/docs/connectors/mysql", "MySQL"], ["/docs/connectors/mongodb", "MongoDB"]] },
  { title: "Concepts", links: [["/docs/architecture", "Architecture"], ["/docs/security", "Security"]] },
  { title: "Reference", links: [["/docs/cli", "CLI"], ["/docs/configuration", "Configuration"], ["/docs/api", "REST API"], ["/docs/faq", "FAQ"]] },
  { title: "Workflows", links: [["/docs/workflows/agents", "AI agents"], ["/docs/workflows/ci-cd", "CI / CD"], ["/docs/workflows/local-dev", "Local development"]] },
];

export default function DocsSidebar() {
  const path = usePathname();
  return (
    <aside className="docs-side">
      <a className="home" href="/">
        <svg viewBox="0 0 32 32" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round"><circle cx="9" cy="7" r="3" /><circle cx="9" cy="25" r="3" /><circle cx="23" cy="16" r="3" /><path d="M9 10v12M9 16h4a6 6 0 0 0 6-6" /></svg>
        snapshot<em>db</em>
      </a>
      {groups.map((g) => (
        <div key={g.title}>
          <div className="grp">{g.title}</div>
          {g.links.map(([href, label]) => (
            <a key={href} href={href} className={path === href ? "active" : ""}>{label}</a>
          ))}
        </div>
      ))}
      <div className="grp">More</div>
      <a href="/console">Console</a>
      <a href="https://github.com/GitHoobar/anybranch">GitHub</a>
    </aside>
  );
}
