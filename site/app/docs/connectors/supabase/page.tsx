export const metadata = { title: "Supabase — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Connect a database</span>
      <h1>Supabase</h1>
      <p className="lead">Supabase runs Postgres with logical replication already enabled. Connect it like any Postgres source.</p>

      <h2>Connection</h2>
      <ul>
        <li>Use the <b>direct</b> connection string, not the pooler — replication needs a direct, long-lived connection.</li>
        <li>Point at the writable primary (<code>pg_is_in_recovery()</code> must be false); preflight checks this as <code>is writer</code>.</li>
        <li><code>wal_level</code> is already <code>logical</code> on Supabase.</li>
      </ul>

      <h2>Schema tracking without superuser</h2>
      <p>The <code>postgres</code> role on Supabase is not a raw superuser, but Supabase’s <code>supautils</code> grants it privileged-role powers, including creating event triggers. anybranch detects this: preflight reports <code>can create event trigger</code> as a privileged role, so schema-change replay works as the <code>postgres</code> role. Verified against the <code>supabase/postgres</code> image.</p>

      <h2>Extensions</h2>
      <p>The schema copy uses <code>pg_dump --schema-only</code>. Extensions the source uses that are not installed on the anybranch server surface as schema-load warnings (saved to <code>run/schema.log</code>); rows still replicate. Install the extensions you depend on on the server, or accept that extension-owned objects are absent on branches.</p>

      <div className="np"><a href="/docs/connectors/rds">← AWS RDS</a><a className="n" href="/docs/connectors/mysql">MySQL →</a></div>
    </>
  );
}
