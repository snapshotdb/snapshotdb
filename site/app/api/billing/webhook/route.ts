import crypto from "node:crypto";
import { hostedFetch } from "@/app/console/hosted";
import { paymentEntitlement, validWebhook } from "@/app/console/razorpay";

export const runtime = "nodejs";
export async function POST(request: Request) {
  const body = await request.text();
  if (Buffer.byteLength(body) > 262144) return new Response("Payload too large", { status: 413 });
  if (!validWebhook(body, request.headers.get("x-razorpay-signature"), process.env.RAZORPAY_WEBHOOK_SECRET || "")) return new Response("Invalid signature", { status: 400 });
  // Digest the signed body, not an unsigned event header, for replay protection.
  const id = crypto.createHash("sha256").update(body).digest("hex");
  const plan = process.env.RAZORPAY_PRO_PLAN_ID;
  if (!plan) return new Response("Billing not configured", { status: 503 });
  let entitlement;
  try { entitlement = paymentEntitlement(JSON.parse(body), id, plan); }
  catch (e) {
    // Retrying cannot fix a malformed payload; a 5xx loop gets the webhook disabled for everyone.
    console.error("razorpay webhook ignored", id, e instanceof Error ? e.message : e);
    return Response.json({ received: true, ignored: true });
  }
  try {
    if (entitlement) {
      const r = await hostedFetch(entitlement.tenant, "/v1/billing/entitlement", { method: "POST", body: JSON.stringify(entitlement.body) });
      if (!r.ok) return new Response("Entitlement update failed; retry", { status: 503 });
    }
    // Failed renewals/cancellations cannot extend the paid-through timestamp.
    return Response.json({ received: true });
  } catch { return new Response("Webhook processing failed; retry", { status: 503 }); }
}
