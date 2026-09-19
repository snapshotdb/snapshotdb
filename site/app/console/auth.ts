import crypto from "crypto";
import { cookies } from "next/headers";

export const SESSION_COOKIE = "ab_session";
export const OAUTH_STATE_COOKIE = "ab_oauth_state";

export type SessionUser = {
  login: string;
  name?: string;
  avatar?: string;
  provider: "github";
};

function secret(): string {
  return process.env.AUTH_SECRET || "snapshotdb-dev-insecure-secret-change-in-production";
}

function hmac(payload: string): string {
  return crypto.createHmac("sha256", secret()).update(payload).digest("base64url");
}

export function encodeSession(user: SessionUser): string {
  const payload = Buffer.from(JSON.stringify(user)).toString("base64url");
  return `${payload}.${hmac(payload)}`;
}

export function verifySession(token: string | undefined | null): SessionUser | null {
  if (!token) return null;
  const dot = token.lastIndexOf(".");
  if (dot < 0) return null;
  const payload = token.slice(0, dot);
  const mac = token.slice(dot + 1);
  const good = hmac(payload);
  if (mac.length !== good.length) return null;
  if (!crypto.timingSafeEqual(Buffer.from(mac), Buffer.from(good))) return null;
  try {
    return JSON.parse(Buffer.from(payload, "base64url").toString());
  } catch {
    return null;
  }
}

export async function getSession(): Promise<SessionUser | null> {
  const jar = await cookies();
  return verifySession(jar.get(SESSION_COOKIE)?.value);
}
