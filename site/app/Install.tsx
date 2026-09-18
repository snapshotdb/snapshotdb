"use client";

import { useState } from "react";

const CMD = "curl -fsSL https://www.snapshotdb.io/install.sh | sh";

export default function Install() {
  const [copied, setCopied] = useState(false);
  return (
    <div className="inst">
      <code><span className="d">$ </span>{CMD}</code>
      <button
        type="button"
        onClick={() => {
          navigator.clipboard?.writeText(CMD).catch(() => {});
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1400);
        }}
      >
        {copied ? "copied" : "copy"}
      </button>
    </div>
  );
}
