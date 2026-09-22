import { NextRequest, NextResponse } from "next/server";
import { SESSION_COOKIE } from "@/app/console/auth";
import { sameOrigin } from "@/app/console/hosted";

// POST only: a GET logout lets any page sign visitors out via <img src>.
export async function POST(req: NextRequest) {
  if (!sameOrigin(req)) return new Response("Invalid origin", { status: 403 });
  const res = NextResponse.redirect(new URL("/console", req.url), 303);
  res.cookies.delete(SESSION_COOKIE);
  return res;
}
