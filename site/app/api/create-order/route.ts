import crypto from "node:crypto";
import { getSession } from "@/app/console/auth";
import { sameOrigin, tenantFor } from "@/app/console/hosted";
import { checkoutPrice, paymentClient, providerError } from "@/app/console/standard-checkout";

export const runtime = "nodejs";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  try {
    // Price and receipt are server-owned; browser-supplied amounts are never trusted.
    const price = checkoutPrice();
    const order = await paymentClient().orders.create({ ...price, receipt: crypto.randomUUID(), notes: { tenant: tenantFor(user), purpose: "standard-checkout-test" } });
    return Response.json({ order_id: order.id, amount: order.amount, currency: order.currency, key_id: process.env.RAZORPAY_KEY_ID }, { headers: { "Cache-Control": "no-store" } });
  } catch (error) { return providerError(error); }
}
