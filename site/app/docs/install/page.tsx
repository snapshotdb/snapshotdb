export const metadata = { title: "Install — snapshotdb docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Getting started</span>
      <h1>Install</h1>
      <p className="lead">One binary, no runtime dependencies. The client submits jobs; the server holds the databases.</p>

      <h2>Client</h2>
      <p>The client is a single Rust binary and only needs itself:</p>
      <pre className="code"><code><span className="p">$</span> curl -fsSL https://snapshotdb.io/install.sh | sh</code></pre>
      <p>Published binaries support macOS (arm64, x86_64) and Linux (x86_64). The installer verifies the download checksum. For other Linux architectures, build from <a href="https://github.com/snapshotdb/snapshotdb">source</a>.</p>
      <div className="note"><b>Client and server:</b> set <code>SNAPSHOTDB_SERVER</code> and <code>SNAPSHOTDB_TOKEN</code> before issuing database commands. GitHub sign-in does not replace your server access token.</div>

      <h2>Server engine binaries</h2>
      <p>Engines run on the <b>server</b>, so their binaries must be on the server’s <code>PATH</code>. Only install the engines you branch.</p>
      <table>
        <thead><tr><th>Engine</th><th>Binaries on the server PATH</th></tr></thead>
        <tbody>
          <tr><td>Postgres</td><td><code>pg_ctl initdb psql pg_dump pg_dumpall</code></td></tr>
          <tr><td>MySQL</td><td><code>mysqld mysql mysqldump</code></td></tr>
          <tr><td>MongoDB</td><td><code>mongod mongosh</code> + <code>mongodump mongorestore</code> for sync</td></tr>
          <tr><td>SQLite</td><td>Bundled with SnapshotDB; no separate SQLite runtime needed.</td></tr>
        </tbody>
      </table>

      <h2>Filesystem</h2>
      <p>Copy-on-write cloning is <code>cp -c</code> (clonefile) on macOS/APFS and <code>cp --reflink=always</code> on Linux (Btrfs, XFS with reflink, or bcachefs). Linux refuses clones when reflinks are unavailable. macOS needs APFS for copy-on-write; other macOS filesystems may copy bytes. Point <code>SNAPSHOTDB_HOME</code> at a directory on a reflink-capable volume.</p>

      <h2>Restore branches after reboot</h2>
      <p>Run <code>snapshotdb serve</code> under your service manager. Starting the server restores branch proxies and restarts synced roots. Linux systemd units are included under <code>deploy/</code> in the repository.</p>

      <div className="np"><a href="/docs/quickstart">← Quickstart</a><a className="n" href="/docs/server">Server deployment →</a></div>
    </>
  );
}
