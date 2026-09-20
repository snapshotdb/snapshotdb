const GH = "https://github.com/snapshotdb/snapshotdb";

function formatStars(n: number) {
  if (n < 1000) return String(n);
  return (n / 1000).toFixed(n < 10000 ? 1 : 0).replace(/\.0$/, "") + "k";
}

export default async function GithubStars() {
  let count: number | null = null;
  try {
    const res = await fetch("https://api.github.com/repos/snapshotdb/snapshotdb", {
      headers: { Accept: "application/vnd.github+json" },
      next: { revalidate: 3600 },
    });
    if (res.ok) {
      const data = await res.json();
      if (typeof data.stargazers_count === "number") count = data.stargazers_count;
    }
  } catch {}

  return (
    <a className="btn gh-stars" href={GH}>
      <svg viewBox="0 0 16 16" width="15" height="15" fill="currentColor" aria-hidden="true">
        <path d="M8 .2a8 8 0 0 0-2.5 15.6c.4.1.5-.2.5-.4v-1.4c-2 .4-2.5-.5-2.7-1 0-.1-.5-1-.8-1.2-.3-.1-.7-.5 0-.5.6 0 1 .6 1.2.8.7 1.2 1.9.9 2.3.7.1-.5.3-.9.5-1.1-1.8-.2-3.6-.9-3.6-4 0-.9.3-1.6.8-2.1 0-.2-.3-1 .1-2.1 0 0 .7-.2 2.2.8a7.5 7.5 0 0 1 4 0c1.5-1 2.2-.8 2.2-.8.4 1.1.1 1.9.1 2.1.5.5.8 1.2.8 2.1 0 3.1-1.9 3.8-3.6 4 .3.2.5.7.5 1.4v2.1c0 .2.1.5.5.4A8 8 0 0 0 8 .2Z" />
      </svg>
      GitHub
      {count != null && <span className="gh-count">★ {formatStars(count)}</span>}
    </a>
  );
}
