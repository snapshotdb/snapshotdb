import { NextRequest, NextResponse } from "next/server";
import { encodeSession, SESSION_COOKIE, githubConfigured } from "@/app/console/auth";

// Demo sign-in for local use when real GitHub OAuth is not configured.
// If GitHub IS configured, defer to the real flow instead.
export async function GET(req: NextRequest) {
  if (githubConfigured()) {
    return NextResponse.redirect(new URL("/api/auth/github", req.url));
  }
  const session = encodeSession({
    login: "demo",
    name: "Demo user",
    provider: "demo",
  });
  const res = NextResponse.redirect(new URL("/console", req.url));
  res.cookies.set(SESSION_COOKIE, session, {
    httpOnly: true,
    sameSite: "lax",
    secure: req.nextUrl.protocol === "https:",
    path: "/",
    maxAge: 60 * 60 * 24 * 7,
  });
  return res;
}
