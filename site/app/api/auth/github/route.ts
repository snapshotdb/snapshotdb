import { NextRequest, NextResponse } from "next/server";
import crypto from "crypto";
import { OAUTH_STATE_COOKIE } from "@/app/console/auth";

// Begin the GitHub OAuth flow. Requires GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET.
export async function GET(req: NextRequest) {
  const clientId = process.env.GITHUB_CLIENT_ID;
  if (!clientId) {
    return NextResponse.redirect(new URL("/console?error=github_not_configured", req.url));
  }
  const state = crypto.randomBytes(16).toString("hex");
  const redirectUri = new URL("/api/auth/github/callback", req.url).toString();

  const authorize = new URL("https://github.com/login/oauth/authorize");
  authorize.searchParams.set("client_id", clientId);
  authorize.searchParams.set("redirect_uri", redirectUri);
  authorize.searchParams.set("scope", "read:user");
  authorize.searchParams.set("state", state);
  authorize.searchParams.set("allow_signup", "false");

  const res = NextResponse.redirect(authorize.toString());
  res.cookies.set(OAUTH_STATE_COOKIE, state, {
    httpOnly: true,
    sameSite: "lax",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge: 600,
  });
  // A plain web login must not be diverted to a CLI loopback left over from an earlier attempt.
  if (!req.nextUrl.searchParams.has("cli")) {
    res.cookies.delete("ab_cli_port");
    res.cookies.delete("ab_cli_state");
  }
  return res;
}
