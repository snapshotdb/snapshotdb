import DocsSidebar from "./DocsSidebar";

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="docs">
      <DocsSidebar />
      <main className="doc">{children}</main>
    </div>
  );
}
