import type { Metadata } from "next";
import { Saira, Space_Mono, Silkscreen } from "next/font/google";
import "./globals.css";

const grotesk = Saira({ subsets: ["latin"], variable: "--f-grotesk", display: "swap" });
const mono = Space_Mono({ subsets: ["latin"], weight: ["400", "700"], variable: "--f-mono", display: "swap" });
const pixel = Silkscreen({ subsets: ["latin"], weight: ["400", "700"], variable: "--f-pixel", display: "swap" });

const title = "SnapshotDB — branch your database like code";
const description =
  "An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own. Open-source database branching for Postgres, MySQL, MongoDB, and SQLite.";

export const metadata: Metadata = {
  metadataBase: new URL("https://www.snapshotdb.io"),
  title: { default: title, template: "%s · SnapshotDB" },
  description,
  keywords: ["database branching", "snapshot", "copy-on-write", "postgres", "mysql", "mongodb", "sqlite", "replica", "open source", "self-hosted", "coding agents"],
  authors: [{ name: "SnapshotDB" }],
  openGraph: { title, description, type: "website", siteName: "SnapshotDB", url: "/" },
  twitter: { card: "summary_large_image", title, description },
  robots: { index: true, follow: true },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${grotesk.variable} ${mono.variable} ${pixel.variable}`}>
      <body>{children}</body>
    </html>
  );
}
