import crypto from "node:crypto";

export function validWebhook(body: string, signature: string | null, secret: string): boolean {
  if (!secret || !signature || !/^[a-f0-9]{64}$/.test(signature)) return false;
  const expected = crypto.createHmac("sha256", secret).update(body).digest("hex");
  return crypto.timingSafeEqual(Buffer.from(signature), Buffer.from(expected));
}

export async function razorpay(path: string, body?: unknown) {
  const id = process.env.RAZORPAY_KEY_ID;
  const secret = process.env.RAZORPAY_KEY_SECRET;
  if (!id || !secret) throw new Error("Billing is not configured yet. Contact SnapshotDB support.");
  const r = await fetch(`https://api.razorpay.com/v1/${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: { Authorization: `Basic ${Buffer.from(`${id}:${secret}`).toString("base64")}`, "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body), cache: "no-store", redirect: "error", signal: AbortSignal.timeout(15000),
  });
  if (!r.ok) throw new Error("Payment provider request failed. Please try again or contact support.");
  return r.json();
}

export async function verifiedPlan() {
  const id = process.env.RAZORPAY_PRO_PLAN_ID;
  if (!id || !/^plan_[A-Za-z0-9]+$/.test(id)) throw new Error("Pro billing plan is not configured.");
  const plan = await razorpay(`plans/${id}`);
  if (plan.period !== "monthly" || plan.interval !== 1 || plan.item?.currency !== "USD" || plan.item?.amount !== 15000) {
    throw new Error("Pro requires a $150 USD monthly Razorpay plan.");
  }
  return id;
}

export function paymentEntitlement(event: Record<string, any>, eventId: string, planId: string) { // eslint-disable-line @typescript-eslint/no-explicit-any
  // Authorization/activation alone can be a small mandate payment, not a paid month.
  if (event.event !== "subscription.charged") return null;
  const sub = event.payload?.subscription?.entity;
  const payment = event.payload?.payment?.entity;
  if (!sub || sub.plan_id !== planId || sub.quantity !== 1 || payment?.status !== "captured" || payment.currency !== "USD" || payment.amount < 15000) return null;
  if (!/^github-[1-9][0-9]{0,19}$/.test(sub.notes?.tenant || "") || !/^sub_[A-Za-z0-9]+$/.test(sub.id || "")) throw new Error("Invalid billing identity");
  if (!Number.isInteger(sub.current_start) || !Number.isInteger(sub.current_end) || sub.current_end <= sub.current_start || !Number.isInteger(event.created_at)) throw new Error("Invalid billing period");
  return { tenant: sub.notes.tenant as string, body: {
    event_id: eventId, event_time: event.created_at,
    customer: sub.customer_id || "", subscription: sub.id,
    period_start: sub.current_start, period_end: sub.current_end, paid: true,
  } };
}
