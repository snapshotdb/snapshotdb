import type { MetadataRoute } from "next";

const paths = [
  "", "/docs", "/docs/quickstart", "/docs/install", "/docs/server",
  "/docs/connectors", "/docs/connectors/postgres", "/docs/connectors/rds", "/docs/connectors/supabase",
  "/docs/connectors/mysql", "/docs/connectors/mongodb",
  "/docs/architecture", "/docs/security", "/docs/cli", "/docs/configuration", "/docs/api", "/docs/faq",
  "/docs/workflows/agents", "/docs/workflows/ci-cd", "/docs/workflows/local-dev",
];

export default function sitemap(): MetadataRoute.Sitemap {
  const base = "https://anybranch.dev";
  return paths.map((p) => ({ url: base + p, changeFrequency: "weekly", priority: p === "" ? 1 : 0.6 }));
}
