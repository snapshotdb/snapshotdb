export const metadata = { title: "Install — anybranch docs" };

export default function Page() {
  return (
    <>
      <span className="eyebrow lbl">Getting started</span>
      <h1>Install</h1>
      <p className="lead">One binary, no runtime dependencies. The client submits jobs; the server holds the databases.</p>

      <h2>Client</h2>
      <p>The client is a single Rust binary and only needs itself:</p>
      <pre className="code"><code><span className="p">$</span> cargo install --git https://github.com/GitHoobar/anybranch</code></pre>
      <p>Or download a release binary for macOS (arm64, x86_64) or Linux (x86_64, aarch64) from the <a href="https://github.com/GitHoobar/anybranch/releases">Releases</a> page.</p>
      <div className="note"><b>Note:</b> the client/server split is on <code>main</code> and unreleased; the v0.3.0 release binaries still use local storage. Build from source for the server mode described here.</div>

      <h2>Server engine binaries</h2>
      <p>Engines run on the <b>server</b>, so their binaries must be on the server’s <code>PATH</code>. Only install the engines you branch.</p>
      <table>
        <thead><tr><th>Engine</th><th>Binaries on the server PATH</th></tr></thead>
        <tbody>
          <tr><td>Postgres</td><td><code>pg_ctl initdb psql pg_dump pg_dumpall</code></td></tr>
          <tr><td>MySQL</td><td><code>mysqld mysql mysqldump</code></td></tr>
          <tr><td>MongoDB</td><td><code>mongod mongosh</code> + <code>mongodump mongorestore</code> for sync</td></tr>
          <tr><td>SQLite</td><td>the stdlib <code>sqlite3</code> is enough</td></tr>
        </tbody>
      </table>

      <h2>Filesystem</h2>
      <p>Copy-on-write cloning is <code>cp -c</code> (clonefile) on macOS/APFS and <code>cp --reflink=always</code> on Linux (Btrfs, XFS with reflink, or bcachefs). On a filesystem without reflink support, branches fall back to full byte copies, so branch storage is no longer free. Point <code>ANYBRANCH_HOME</code> at a directory on a reflink-capable volume.</p>

      <h2>Optional: restore branches at login</h2>
      <pre className="code"><code><span className="p">$</span> anybranch service install</code></pre>
      <p>Installs a launchd agent (macOS) or a systemd user unit (Linux) that runs <code>anybranch up</code> after a reboot, restoring every branch’s proxy and restarting synced roots.</p>

      <div className="np"><a href="/docs/quickstart">← Quickstart</a><a className="n" href="/docs/server">Server deployment →</a></div>
    </>
  );
}
