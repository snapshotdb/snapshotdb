import { getSession } from "@/app/console/auth";
import { account, sameOrigin, tenantFor } from "@/app/console/hosted";
import { createBillingPortal } from "@/app/console/billing-portal";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  try {
    const current = await account(tenantFor(user));
    const url = await createBillingPortal(current.customer, new URL("/console?billing=return", request.url).toString());
    return Response.json({ url }, { headers: { "Cache-Control": "no-store" } });
  } catch (e) {
    return Response.json({ error: e instanceof Error ? e.message : "Billing portal unavailable" }, { status: 503 });
  }
}
