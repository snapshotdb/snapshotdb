import type { Metadata } from "next";
import { Bricolage_Grotesque, Space_Mono } from "next/font/google";
import "./globals.css";

const grotesk = Bricolage_Grotesque({ subsets: ["latin"], variable: "--f-grotesk", display: "swap" });
const mono = Space_Mono({ subsets: ["latin"], weight: ["400", "700"], variable: "--f-mono", display: "swap" });

const title = "anybranch — branch your database like code";
const description =
  "An isolated, writable copy of production in seconds — any size, any engine, on infrastructure you own. Open-source database branching for Postgres, MySQL, MongoDB, and SQLite.";

// TODO: set to the deployed domain; used to resolve OG/Twitter image URLs.
export const metadata: Metadata = {
  metadataBase: new URL("https://anybranch.dev"),
  title: { default: title, template: "%s · anybranch" },
  description,
  keywords: ["database branching", "copy-on-write", "postgres", "mysql", "mongodb", "sqlite", "replica", "open source", "self-hosted", "coding agents"],
  authors: [{ name: "anybranch" }],
  openGraph: { title, description, type: "website", siteName: "anybranch", url: "/" },
  twitter: { card: "summary_large_image", title, description },
  robots: { index: true, follow: true },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${grotesk.variable} ${mono.variable}`}>
      <body>{children}</body>
    </html>
  );
}
