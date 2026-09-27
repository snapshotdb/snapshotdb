import { getSession } from "@/app/console/auth";
import { account, hostedFetch, sameOrigin, tenantFor } from "@/app/console/hosted";
import { checkoutUrl, dodo, SUBSCRIPTION_ID, verifiedProduct } from "@/app/console/dodo";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  try {
    const tenant = tenantFor(user);
    const current = await account(tenant);
    if (current.plan === "pro") return Response.json({ error: "You already have Pro. Your allowance renews next billing period." }, { status: 409 });
    if (current.subscription && !SUBSCRIPTION_ID.test(current.subscription)) throw new Error("Invalid subscription on file; contact support");
    if (current.subscription) {
      const existing = await dodo(`/subscriptions/${current.subscription}`);
      if (!["cancelled", "expired", "failed"].includes(existing.status)) {
        return Response.json({ error: "A subscription already exists. Choose Manage billing in Plan & usage to check its status or update your payment method." }, { status: 409 });
      }
    }
    const productId = await verifiedProduct();
    // One checkout per tenant at a time: the backend hands out a nonce, or the link already made.
    const reservation = await hostedFetch(tenant, "/v1/billing/checkout", { method: "POST", body: "{}" });
    const pending = await reservation.json();
    if (!reservation.ok) return Response.json(pending, { status: 409 });
    if (pending.url) return Response.json({ url: checkoutUrl(pending.url) });
    const session = await dodo("/checkouts", { body: {
      product_cart: [{ product_id: productId, quantity: 1 }],
      // Charge the list price everywhere. Otherwise Dodo offers the visitor's local currency
      // and discount codes, and the webhook (which requires USD >= $150) would never grant Pro.
      billing_currency: "USD",
      feature_flags: { allow_currency_selection: false, allow_discount_code: false },
      return_url: new URL("/console?billing=return", request.url).toString(),
      metadata: { tenant },
    } });
    const url = checkoutUrl(session.checkout_url);
    const saved = await hostedFetch(tenant, "/v1/billing/checkout", { method: "POST", body: JSON.stringify({ nonce: pending.nonce, url }) });
    if (!saved.ok) throw new Error("Could not save checkout. Please try again shortly.");
    return Response.json({ url });
  } catch (e) {
    return Response.json({ error: e instanceof Error ? e.message : "Checkout unavailable" }, { status: 503 });
  }
}
