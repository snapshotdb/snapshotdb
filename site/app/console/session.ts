import crypto from "node:crypto";

export type SessionUser = {
  login: string;
  name?: string;
  avatar?: string;
  provider: "github";
};

function secret(): string {
  const value = process.env.AUTH_SECRET;
  if (!value || value.length < 32) throw new Error("AUTH_SECRET must contain at least 32 characters");
  return value;
}

function hmac(payload: string): string {
  return crypto.createHmac("sha256", secret()).update(payload).digest("base64url");
}

export function encodeSession(user: SessionUser): string {
  const payload = Buffer.from(JSON.stringify({ ...user, exp: Math.floor(Date.now() / 1000) + 60 * 60 * 24 * 7 })).toString("base64url");
  return `${payload}.${hmac(payload)}`;
}

export function verifySession(token: string | undefined | null): SessionUser | null {
  if (!token || token.length > 8192) return null;
  const dot = token.lastIndexOf(".");
  if (dot < 0) return null;
  const payload = token.slice(0, dot);
  const mac = token.slice(dot + 1);
  if (!/^[A-Za-z0-9_-]{43}$/.test(mac) || !/^[A-Za-z0-9_-]+$/.test(payload)) return null;
  try {
    const good = hmac(payload);
    if (!crypto.timingSafeEqual(Buffer.from(mac), Buffer.from(good))) return null;
    const value = JSON.parse(Buffer.from(payload, "base64url").toString());
    if (!value || value.provider !== "github" || typeof value.login !== "string" || !value.login ||
        !Number.isInteger(value.exp) || value.exp <= Math.floor(Date.now() / 1000) ||
        (value.name !== undefined && typeof value.name !== "string") ||
        (value.avatar !== undefined && typeof value.avatar !== "string")) return null;
    return { login: value.login, name: value.name, avatar: value.avatar, provider: "github" };
  } catch {
    return null;
  }
}

