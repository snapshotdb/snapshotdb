"use client";

import { useEffect } from "react";
import { initConsole } from "./engine";
import type { SessionUser } from "./auth";
import "./console.css";

export default function Console({ user }: { user: SessionUser }) {
  useEffect(() => {
    const cleanup = initConsole();
    return cleanup;
  }, []);

  const display = user.name || user.login;
  const initials =
    display
      .trim()
      .split(/\s+/)
      .map((s) => s[0])
      .join("")
      .slice(0, 2)
      .toUpperCase() || user.login.slice(0, 1).toUpperCase();

  return (
    <div className="abc">
      <div className="app">
        <aside className="side">
          <div className="side-brand">
            <svg className="glyph" viewBox="0 0 32 32" fill="none" stroke="var(--ink)" strokeWidth="2.4" strokeLinecap="round">
              <circle cx="9" cy="7" r="3" />
              <circle cx="9" cy="25" r="3" />
              <circle cx="23" cy="16" r="3" />
              <path d="M9 10v12M9 16h4a6 6 0 0 0 6-6" />
            </svg>
            any<em>branch</em> <small>CONSOLE</small>
          </div>

          <div className="side-org">
            <span className="dot" id="healthDot"></span>
            <span id="healthText">not connected</span>
          </div>

          <nav className="nav">
            <div className="nav-h">Server</div>
            <button className="nav-i active" data-view="overview">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
                <rect x="1.5" y="1.5" width="5" height="5" />
                <rect x="9.5" y="1.5" width="5" height="5" />
                <rect x="1.5" y="9.5" width="5" height="5" />
                <rect x="9.5" y="9.5" width="5" height="5" />
              </svg>
              Overview
            </button>
            <button className="nav-i" data-view="sources">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
                <ellipse cx="8" cy="3.5" rx="5.5" ry="2" />
                <path d="M2.5 3.5v9c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2v-9" />
                <path d="M2.5 8c0 1.1 2.5 2 5.5 2s5.5-.9 5.5-2" />
              </svg>
              Sources
            </button>
            <button className="nav-i" data-view="branches">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <circle cx="4" cy="3.5" r="1.9" />
                <circle cx="4" cy="12.5" r="1.9" />
                <circle cx="12" cy="6.5" r="1.9" />
                <path d="M4 5.4v5.2M4 9h3.5a4 4 0 0 0 4-4" />
              </svg>
              Branches
            </button>
            <button className="nav-i" id="navSettings">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <path d="M2 4.5h7M2 11.5h4" />
                <circle cx="12" cy="4.5" r="2" />
                <circle cx="9" cy="11.5" r="2" />
              </svg>
              Settings
            </button>
            <button className="nav-i" id="connBtn">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <path d="M6.4 9.6l3.2-3.2M5.4 8 4 9.4a2.1 2.1 0 0 0 3 3L8.4 11M10.6 8 12 6.6a2.1 2.1 0 0 0-3-3L7.6 5" />
              </svg>
              Connection
            </button>
          </nav>

          <div className="side-foot">
            <a href="/docs">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round">
                <path d="M2 3h4a2 2 0 0 1 2 2v8a1.6 1.6 0 0 0-1.6-1.6H2zM14 3h-4a2 2 0 0 0-2 2v8a1.6 1.6 0 0 1 1.6-1.6H14z" />
              </svg>
              Documentation
            </a>
            <a href="https://github.com/GitHoobar/anybranch">
              <svg viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 .2a8 8 0 0 0-2.5 15.6c.4.1.5-.2.5-.4v-1.4c-2 .4-2.5-.5-2.7-1 0-.1-.5-1-.8-1.2-.3-.1-.7-.5 0-.5.6 0 1 .6 1.2.8.7 1.2 1.9.9 2.3.7.1-.5.3-.9.5-1.1-1.8-.2-3.6-.9-3.6-4 0-.9.3-1.6.8-2.1 0-.2-.3-1 .1-2.1 0 0 .7-.2 2.2.8a7.5 7.5 0 0 1 4 0c1.5-1 2.2-.8 2.2-.8.4 1.1.1 1.9.1 2.1.5.5.8 1.2.8 2.1 0 3.1-1.9 3.8-3.6 4 .3.2.5.7.5 1.4v2.1c0 .2.1.5.5.4A8 8 0 0 0 8 .2Z" />
              </svg>
              GitHub
            </a>
            <div className="side-ver">anybranch console · <span id="ver">—</span></div>
          </div>

          <div className="side-user">
            {user.avatar ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img src={user.avatar} alt={display} />
            ) : (
              <span className="avatar-init">{initials}</span>
            )}
            <div className="u-t">
              <div className="nm">{display}</div>
              <div className="hd">{user.provider === "demo" ? "demo session" : "@" + user.login}</div>
            </div>
            <a className="out" href="/api/auth/logout" title="Sign out" aria-label="Sign out">
              <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M6 14H3a1 1 0 0 1-1-1V3a1 1 0 0 1 1-1h3M10.5 11 14 8l-3.5-3M14 8H6" />
              </svg>
            </a>
          </div>
        </aside>

        <div className="main">
          <div className="topbar">
            <h1 id="pageTitle">Overview</h1>
            <div className="spacer"></div>
            <button className="iconbtn" id="refreshBtn" title="Refresh (r)">↻ Refresh</button>
            <button className="btn primary" id="newSourceBtn">+ New source</button>
          </div>
          <div className="content">
            <div className="statcards" id="statcards">
              <div className="statcard"><div className="lbl"><span className="d"></span>Sources</div><div className="num" id="stSources">—</div></div>
              <div className="statcard"><div className="lbl"><span className="d"></span>Branches</div><div className="num" id="stBranches">—</div></div>
              <div className="statcard"><div className="lbl"><span className="d"></span>Engines</div><div className="num" id="stEngines">—</div></div>
            </div>
            <div id="stage">
              <div className="empty"><h3>Connecting…</h3><p>Reading the server inventory.</p></div>
            </div>
          </div>
        </div>
      </div>

      <div id="toasts"></div>
      <div id="modalHost"></div>
    </div>
  );
}
