import { hostedFetch } from "@/app/console/hosted";
import { subscriptionEntitlement, validWebhook } from "@/app/console/dodo";

export const runtime = "nodejs";
export async function POST(request: Request) {
  const body = await request.text();
  if (Buffer.byteLength(body) > 262144) return new Response("Payload too large", { status: 413 });
  if (!validWebhook(body, request.headers, process.env.DODO_WEBHOOK_SECRET || "")) return new Response("Invalid signature", { status: 401 });
  const product = process.env.DODO_PRO_PRODUCT_ID;
  if (!product) return new Response("Billing not configured", { status: 503 });
  // webhook-id is signed and stable across retries; the backend ignores ids it has applied.
  const id = request.headers.get("webhook-id")!;
  let entitlement;
  try { entitlement = subscriptionEntitlement(JSON.parse(body), id, product); }
  catch (e) {
    // Retrying cannot fix a malformed payload; a 5xx loop gets the endpoint disabled for everyone.
    console.error("dodo webhook ignored", id, e instanceof Error ? e.message : e);
    return Response.json({ received: true, ignored: true });
  }
  try {
    if (entitlement) {
      const r = await hostedFetch(entitlement.tenant, "/v1/billing/entitlement", { method: "POST", body: JSON.stringify(entitlement.body) });
      if (!r.ok) return new Response("Entitlement update failed; retry", { status: 503 });
    }
    return Response.json({ received: true });
  } catch { return new Response("Webhook processing failed; retry", { status: 503 }); }
}
