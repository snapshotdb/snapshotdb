import { NextRequest, NextResponse } from "next/server";
import { SESSION_COOKIE } from "@/app/console/auth";

export async function GET(req: NextRequest) {
  const res = NextResponse.redirect(new URL("/console", req.url));
  res.cookies.delete(SESSION_COOKIE);
  return res;
}
