import { NextRequest, NextResponse } from "next/server";

// CLI login entry. `snapshotdb login` opens the browser here with the loopback
// port + state; we stash them, run the normal GitHub OAuth flow, and the
// callback then redirects the browser back to the CLI's loopback with a session.
export async function GET(req: NextRequest) {
  const port = req.nextUrl.searchParams.get("port") || "";
  const state = req.nextUrl.searchParams.get("state") || "";
  if (!/^\d{1,5}$/.test(port) || Number(port) < 1 || Number(port) > 65535 || !/^[a-f0-9]{32}$/.test(state)) {
    return NextResponse.redirect(new URL("/console?error=cli_port", req.url));
  }
  const res = NextResponse.redirect(new URL("/api/auth/github", req.url));
  const opts = {
    httpOnly: true,
    sameSite: "lax" as const,
    secure: req.nextUrl.protocol === "https:",
    path: "/",
    maxAge: 600,
  };
  res.cookies.set("ab_cli_port", port, opts);
  res.cookies.set("ab_cli_state", state, opts);
  return res;
}
