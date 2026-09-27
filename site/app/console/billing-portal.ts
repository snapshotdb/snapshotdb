import { dodo } from "./dodo.ts";

export function portalUrl(raw: string) {
  const url = new URL(raw);
  if (url.protocol !== "https:" || !url.hostname.endsWith(".dodopayments.com") || url.username || url.password || url.port) {
    throw new Error("Invalid billing portal URL");
  }
  return url.toString();
}

/** Customer identity comes only from the authenticated tenant's account. */
export async function createBillingPortal(customer: string, returnUrl: string) {
  if (!/^cus_[A-Za-z0-9]+$/.test(customer)) throw new Error("No billing customer found. Complete checkout first.");
  const query = new URLSearchParams({ send_email: "false", return_url: returnUrl });
  const session = await dodo(`/customers/${customer}/customer-portal/session?${query}`, { method: "POST" });
  return portalUrl(session.link);
}
