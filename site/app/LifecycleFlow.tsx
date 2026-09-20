"use client";

import { useState } from "react";

const STAGES = [
  {
    k: "01 / OPEN", h: "opens",
    body: "A copy-on-write clone of the live replica. Unchanged pages share storage. Prepare a pool ahead of time for sub-second branch claims.",
    icon: <><circle cx="10" cy="20" r="4" /><path d="M14 20h8" /><circle cx="28" cy="20" r="4" /></>,
  },
  {
    k: "02 / DIVERGE", h: "diverges",
    body: "Own server, own port, own generated credentials. Writes stay on the branch — never production, never a sibling.",
    icon: <><circle cx="9" cy="20" r="4" /><path d="M13 20q9 0 12-7M13 20q9 0 12 7" /><circle cx="29" cy="12" r="3" /><circle cx="29" cy="28" r="3" /></>,
  },
  {
    k: "03 / WORK", h: "does work",
    body: "Run the migration, the backfill, the risky query, the agent. Break it entirely. Real data, zero blast radius.",
    icon: <><circle cx="20" cy="20" r="4.5" /><path d="M20 8v5M20 27v5M8 20h5M27 20h5" /></>,
  },
  {
    k: "04 / COLLECT", h: "collects",
    body: "Keep what you learned, drop the branch, or reset to a fresh copy. Production never felt a thing. Idle branches suspend.",
    icon: <><circle cx="28" cy="20" r="4" /><path d="M24 20H15M20 13l-7 7 7 7" /></>,
  },
];

export default function LifecycleFlow() {
  const [active, setActive] = useState(0);
  const stage = STAGES[active];

  return (
    <div className="flow">
      <div className="flow-rail" role="tablist" aria-label="Stages of a branch">
        <div className="flow-line"><i style={{ width: `${(active / (STAGES.length - 1)) * 100}%` }} /></div>
        {STAGES.map((s, i) => (
          <button
            key={s.k}
            role="tab"
            aria-selected={i === active}
            className={"flow-stop" + (i === active ? " active" : i < active ? " done" : "")}
            onClick={() => setActive(i)}
          >
            <span className="flow-dot" />
            <span className="flow-k">{s.k}</span>
          </button>
        ))}
      </div>
      <div className="flow-panel">
        <svg className="flow-icon" viewBox="0 0 40 40" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">{stage.icon}</svg>
        <h3>It <em>{stage.h}</em></h3>
        <p>{stage.body}</p>
      </div>
    </div>
  );
}
