import { NextRequest, NextResponse } from "next/server";
import { encodeSession, SESSION_COOKIE, OAUTH_STATE_COOKIE } from "@/app/console/auth";

// GitHub redirects here with ?code&state. Exchange the code, read the user, set a session.
export async function GET(req: NextRequest) {
  const url = req.nextUrl;
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const expected = req.cookies.get(OAUTH_STATE_COOKIE)?.value;

  const fail = (reason: string) =>
    NextResponse.redirect(new URL(`/console?error=${reason}`, req.url));

  if (!code || !state || !expected || state !== expected) return fail("oauth_state");

  const clientId = process.env.GITHUB_CLIENT_ID;
  const clientSecret = process.env.GITHUB_CLIENT_SECRET;
  if (!clientId || !clientSecret) return fail("github_not_configured");

  const redirectUri = new URL("/api/auth/github/callback", req.url).toString();

  let accessToken: string | undefined;
  try {
    const tokenRes = await fetch("https://github.com/login/oauth/access_token", {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify({
        client_id: clientId,
        client_secret: clientSecret,
        code,
        redirect_uri: redirectUri,
      }),
    });
    const tokenJson = await tokenRes.json();
    accessToken = tokenJson?.access_token;
  } catch {
    return fail("oauth_token");
  }
  if (!accessToken) return fail("oauth_token");

  let gh: { login?: string; name?: string; avatar_url?: string } | null = null;
  try {
    const userRes = await fetch("https://api.github.com/user", {
      headers: {
        Authorization: `Bearer ${accessToken}`,
        Accept: "application/vnd.github+json",
        "User-Agent": "snapshotdb-console",
      },
    });
    gh = await userRes.json();
  } catch {
    return fail("oauth_user");
  }
  if (!gh?.login) return fail("oauth_user");

  const session = encodeSession({
    login: gh.login,
    name: gh.name || gh.login,
    avatar: gh.avatar_url,
    provider: "github",
  });

  const res = NextResponse.redirect(new URL("/console", req.url));
  res.cookies.set(SESSION_COOKIE, session, {
    httpOnly: true,
    sameSite: "lax",
    secure: req.nextUrl.protocol === "https:",
    path: "/",
    maxAge: 60 * 60 * 24 * 7,
  });
  res.cookies.delete(OAUTH_STATE_COOKIE);
  return res;
}
