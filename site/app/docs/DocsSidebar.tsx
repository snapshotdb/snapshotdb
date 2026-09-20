"use client";
import { usePathname } from "next/navigation";
import Link from "next/link";
import Logo from "../Logo";

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
      <Link className="home" href="/">
        <Logo />
        snapshot<em>db</em>
      </Link>
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
