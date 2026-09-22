import { NextRequest, NextResponse } from "next/server";
import { sameOrigin } from "@/app/console/hosted";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

// Explicit consent step between GitHub OAuth and handing a session to the CLI's loopback.
export async function GET(req: NextRequest) {
  const port = req.cookies.get("ab_cli_port")?.value || "";
  const user = req.nextUrl.searchParams.get("user") || "";
  if (!/^\d{1,5}$/.test(port)) return NextResponse.redirect(new URL("/console?error=cli_port", req.url));
  const html = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Authorize CLI</title>
<body style="font:16px system-ui;max-width:28rem;margin:15vh auto;padding:0 16px">
<h1 style="font-size:1.3rem">Authorize snapshotdb CLI?</h1>
<p>Signing in as <b>@${esc(user)}</b> will send a session to the program listening on
<code>127.0.0.1:${esc(port)}</code>. Only continue if you just ran <code>snapshotdb login</code>.</p>
<form method="post"><button style="font:inherit;padding:.5rem 1rem">Authorize CLI</button>
<a href="/console" style="margin-left:1rem">Cancel</a></form></body>`;
  return new Response(html, { headers: { "content-type": "text/html; charset=utf-8", "x-frame-options": "DENY" } });
}

export async function POST(req: NextRequest) {
  if (!sameOrigin(req)) return new Response("Invalid origin", { status: 403 });
  const port = req.cookies.get("ab_cli_port")?.value || "";
  const state = req.cookies.get("ab_cli_state")?.value || "";
  const session = req.cookies.get("ab_cli_session")?.value || "";
  if (!/^\d{1,5}$/.test(port) || Number(port) < 1 || Number(port) > 65535 || !/^[a-f0-9]{32}$/.test(state) || !session) {
    return NextResponse.redirect(new URL("/console?error=cli_port", req.url), 303);
  }
  const to = new URL(`http://127.0.0.1:${port}/callback`);
  to.searchParams.set("token", session);
  to.searchParams.set("user", req.cookies.get("ab_cli_user")?.value || "");
  to.searchParams.set("state", state);
  const res = NextResponse.redirect(to.toString(), 303);
  res.cookies.delete("ab_cli_port");
  res.cookies.delete("ab_cli_state");
  for (const name of ["ab_cli_session", "ab_cli_user"]) res.cookies.set(name, "", { path: "/api/auth/cli", maxAge: 0 });
  return res;
}
