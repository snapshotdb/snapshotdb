import { getSession } from "@/app/console/auth";
import { account, hostedFetch, sameOrigin, tenantFor } from "@/app/console/hosted";
import { razorpay, verifiedPlan } from "@/app/console/razorpay";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  try {
    const tenant = tenantFor(user);
    const current = await account(tenant);
    if (current.plan === "pro") return Response.json({ error: "You already have Pro. Your allowance renews next billing period." }, { status: 409 });
    if (current.subscription && !/^sub_[A-Za-z0-9]+$/.test(current.subscription)) throw new Error("Invalid subscription on file; contact support");
    if (current.subscription) {
      const existing = await razorpay(`subscriptions/${current.subscription}`);
      if (!["cancelled", "completed", "expired"].includes(existing.status)) {
        return Response.json({ error: "A subscription already exists. Refresh usage or contact support to resolve its payment status." }, { status: 409 });
      }
    }
    const planId = await verifiedPlan();
    const reservation = await hostedFetch(tenant, "/v1/billing/checkout", { method: "POST", body: "{}" });
    const pending = await reservation.json();
    if (!reservation.ok) return Response.json(pending, { status: 409 });
    const checkoutUrl = (raw: string) => {
      const url = new URL(raw);
      if (url.protocol !== "https:" || !["rzp.io", "rzp.co", "razorpay.com"].includes(url.hostname)) throw new Error("Invalid checkout URL");
      return url;
    };
    if (pending.url) return Response.json({ url: checkoutUrl(pending.url).toString() });
    const sub = await razorpay("subscriptions", { plan_id: planId, total_count: 120, quantity: 1, customer_notify: true, expire_by: Math.floor(Date.now()/1000)+1800, notes: { tenant } });
    const url = checkoutUrl(sub.short_url);
    const saved = await hostedFetch(tenant, "/v1/billing/checkout", { method: "POST", body: JSON.stringify({ nonce: pending.nonce, url: url.toString() }) });
    if (!saved.ok) throw new Error("Could not save checkout. Please try again shortly.");
    return Response.json({ url: url.toString() });
  } catch (e) {
    return Response.json({ error: e instanceof Error ? e.message : "Checkout unavailable" }, { status: 503 });
  }
}
