import type { SessionUser } from "./session";

export function tenantFor(user: SessionUser): string {
  if (!user.id) throw new Error("Sign out and sign in again to create your hosted workspace.");
  return `github-${user.id}`;
}

export function sameOrigin(request: Request): boolean {
  return request.headers.get("origin") === new URL(request.url).origin;
}

export async function hostedFetch(tenant: string, path: string, init: RequestInit = {}) {
  const token = process.env.SNAPSHOTDB_HOSTED_TOKEN;
  if (!token || token.length < 32) throw new Error("Hosted service is not configured. Contact SnapshotDB support.");
  const base = process.env.SNAPSHOTDB_HOSTED_API || "https://api.snapshotdb.io";
  const url = new URL(base);
  if (url.protocol !== "https:" && !(process.env.NODE_ENV !== "production" && ["localhost", "127.0.0.1"].includes(url.hostname))) {
    throw new Error("Hosted API must use HTTPS");
  }
  return fetch(new URL(path, url), {
    ...init,
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}`, "X-SnapshotDB-Tenant": tenant },
    cache: "no-store", redirect: "error", signal: AbortSignal.timeout(20000),
  });
}

export async function account(tenant: string) {
  const r = await hostedFetch(tenant, "/v1/account");
  if (!r.ok) throw new Error("Could not load plan usage");
  return r.json();
}
