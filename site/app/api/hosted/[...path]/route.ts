import { getSession } from "@/app/console/auth";
import { hostedFetch, sameOrigin, tenantFor } from "@/app/console/hosted";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

async function handle(request: Request, context: { params: Promise<{ path: string[] }> }) {
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in to use SnapshotDB" }, { status: 401 });
  if (request.method === "POST" && !sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const path = "/" + (await context.params).path.join("/");
  const allowed = request.method === "GET"
    ? ["/v1/health", "/v1/account"].includes(path) || /^\/v1\/jobs\/[A-Za-z0-9]{1,64}$/.test(path)
    : path === "/v1/commands";
  if (!allowed) return Response.json({ error: "Unknown endpoint" }, { status: 404 });
  try {
    const body = request.method === "POST" ? await request.text() : undefined;
    if (body && Buffer.byteLength(body) > 65536) return Response.json({ error: "Request too large" }, { status: 413 });
    const upstream = await hostedFetch(tenantFor(user), path, { method: request.method, body });
    return new Response(upstream.body, { status: upstream.status, headers: { "Content-Type": "application/json", "Cache-Control": "no-store" } });
  } catch (error) {
    return Response.json({ error: error instanceof Error ? error.message : "Hosted service unavailable" }, { status: 503 });
  }
}
export const GET = handle;
export const POST = handle;
