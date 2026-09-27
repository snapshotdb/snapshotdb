import crypto from "node:crypto";

// Dodo Payments: hosted checkout sessions for the Pro subscription, Standard Webhooks for
// entitlement. DODO_PAYMENTS_ENVIRONMENT picks the API host; keys only work in their mode.
const HOSTS = { test_mode: "https://test.dodopayments.com", live_mode: "https://live.dodopayments.com" };
export const CHECKOUT_HOSTS = ["checkout.dodopayments.com", "test.checkout.dodopayments.com"];
export const SUBSCRIPTION_ID = /^sub_[A-Za-z0-9]+$/;
const TENANT = /^github-[1-9][0-9]{0,19}$/;
const PRO_PRICE = 15000; // USD cents per month

export async function dodo(path: string, init: { method?: string; body?: unknown } = {}) {
  const key = process.env.DODO_PAYMENTS_API_KEY;
  const base = HOSTS[process.env.DODO_PAYMENTS_ENVIRONMENT as keyof typeof HOSTS];
  if (!key || !base) throw new Error("Billing is not configured yet. Contact SnapshotDB support.");
  const r = await fetch(`${base}${path}`, {
    method: init.method || (init.body === undefined ? "GET" : "POST"),
    headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
    body: init.body === undefined ? undefined : JSON.stringify(init.body),
    cache: "no-store", redirect: "error", signal: AbortSignal.timeout(15000),
  });
  if (!r.ok) throw new Error("Payment provider request failed. Please try again or contact support.");
  return r.json();
}

/** The configured product must really be the $150/month Pro plan before anyone is sent to pay. */
export async function verifiedProduct() {
  const id = process.env.DODO_PRO_PRODUCT_ID;
  if (!id || !/^pdt_[A-Za-z0-9]+$/.test(id)) throw new Error("Pro billing plan is not configured.");
  const p = await dodo(`/products/${id}`);
  const d = p.price || {}; // GET /products/{id} nests the price here (the list calls it price_detail)
  if (!p.is_recurring || d.type !== "recurring_price" || d.price !== PRO_PRICE || d.currency !== "USD" || d.discount
    || d.payment_frequency_count !== 1 || d.payment_frequency_interval !== "Month" || d.trial_period_days) {
    throw new Error("Pro requires a $150 USD monthly Dodo product without a trial.");
  }
  return id;
}

export function checkoutUrl(raw: string) {
  const url = new URL(raw);
  if (url.protocol !== "https:" || !CHECKOUT_HOSTS.includes(url.hostname)) throw new Error("Invalid checkout URL");
  return url.toString();
}

/** Standard Webhooks: HMAC-SHA256 over `id.timestamp.body` with the base64 key after `whsec_`. */
export function validWebhook(body: string, headers: Headers, secret: string, now = Date.now() / 1000): boolean {
  const id = headers.get("webhook-id") || "";
  const timestamp = headers.get("webhook-timestamp") || "";
  const signatures = headers.get("webhook-signature") || "";
  if (!secret.startsWith("whsec_") || !id || !/^\d{1,12}$/.test(timestamp) || Math.abs(now - Number(timestamp)) > 300) return false;
  const expected = crypto.createHmac("sha256", Buffer.from(secret.slice(6), "base64")).update(`${id}.${timestamp}.${body}`).digest();
  return signatures.split(" ").some((s) => {
    const [version, value] = s.split(",", 2);
    const given = Buffer.from(value || "", "base64");
    return version === "v1" && given.length === expected.length && crypto.timingSafeEqual(given, expected);
  });
}

const seconds = (iso: unknown) => typeof iso === "string" ? Math.floor(Date.parse(iso) / 1000) : NaN;

/**
 * Map a verified subscription event to the hosted backend's entitlement record, or null when
 * the event does not change access. Paid events carry the billing period Dodo just charged;
 * an immediate cancellation, failure or expiry ends access. A cancellation scheduled for the
 * next billing date changes nothing: the paid period already ends then.
 */
export function subscriptionEntitlement(event: Record<string, any>, eventId: string, productId: string) { // eslint-disable-line @typescript-eslint/no-explicit-any
  const s = event.data;
  if (!s || s.payload_type !== "Subscription" || s.product_id !== productId) return null;
  const paid = ["subscription.active", "subscription.renewed"].includes(event.type);
  const ended = ["subscription.failed", "subscription.expired"].includes(event.type)
    || (event.type === "subscription.cancelled" && !s.cancel_at_next_billing_date);
  if (!paid && !ended) return null;
  if (paid && (s.status !== "active" || s.quantity !== 1 || s.currency !== "USD" || !Number.isSafeInteger(s.recurring_pre_tax_amount) || s.recurring_pre_tax_amount < PRO_PRICE)) return null;
  if (!TENANT.test(s.metadata?.tenant || "") || !SUBSCRIPTION_ID.test(s.subscription_id || "")) throw new Error("Invalid billing identity");
  const at = seconds(event.timestamp);
  if (!Number.isInteger(at)) throw new Error("Invalid event time");
  const nonce = s.metadata?.checkout_nonce;
  const base = { event_id: eventId, event_time: at, event_time_ms: Date.parse(event.timestamp), customer: s.customer?.customer_id || "", subscription: s.subscription_id,
    ...(typeof nonce === "string" && /^[a-f0-9]{32}$/.test(nonce) ? { checkout_nonce: nonce } : {}) };
  // Ending access needs no period: a failed first payment never had one.
  if (!paid) return { tenant: s.metadata.tenant as string, body: { ...base, paid: false } };
  const start = seconds(s.previous_billing_date), end = seconds(s.next_billing_date);
  if (!Number.isInteger(start) || !Number.isInteger(end) || end <= start) throw new Error("Invalid billing period");
  return { tenant: s.metadata.tenant as string, body: { ...base, period_start: start, period_end: end, paid: true } };
}
