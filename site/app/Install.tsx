"use client";

import { useState } from "react";

const CMD = "curl -fsSL https://www.snapshotdb.io/install.sh | sh";

export default function Install() {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  return (
    <div className="inst">
      <code><span className="d">$ </span>{CMD}</code>
      <button
        type="button"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(CMD);
            setFailed(false);
            setCopied(true);
            window.setTimeout(() => setCopied(false), 1400);
          } catch { setFailed(true); }
        }}
      >
        {copied ? "copied" : failed ? "copy failed — select command" : "copy"}
      </button>
    </div>
  );
}
