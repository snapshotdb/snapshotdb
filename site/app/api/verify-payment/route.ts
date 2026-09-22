import { getSession } from "@/app/console/auth";
import { sameOrigin, tenantFor } from "@/app/console/hosted";
import { paymentClient, providerError, validPaymentSignature } from "@/app/console/standard-checkout";

export const runtime = "nodejs";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  let body;
  try { body = await request.json(); } catch { return Response.json({ error: "Invalid JSON" }, { status: 400 }); }
  const { razorpay_order_id: orderId, razorpay_payment_id: paymentId, razorpay_signature: signature } = body || {};
  if (typeof orderId !== "string" || !/^order_[A-Za-z0-9]+$/.test(orderId) || typeof paymentId !== "string" || !/^pay_[A-Za-z0-9]+$/.test(paymentId) || typeof signature !== "string") {
    return Response.json({ error: "Missing or invalid payment fields" }, { status: 400 });
  }
  const secret = process.env.RAZORPAY_KEY_SECRET;
  if (!secret) return Response.json({ error: "Billing is not configured" }, { status: 500 });
  if (!validPaymentSignature(orderId, paymentId, signature, secret)) return Response.json({ error: "Payment signature mismatch" }, { status: 400 });
  try {
    const client = paymentClient();
    const order = await client.orders.fetch(orderId);
    if (order.notes?.tenant !== tenantFor(user) || order.notes?.purpose !== "standard-checkout-test") return Response.json({ error: "Order does not belong to this account" }, { status: 403 });
    const payment = await client.payments.fetch(paymentId);
    if (payment.order_id !== order.id || Number(payment.amount) !== Number(order.amount) || payment.currency !== order.currency) return Response.json({ error: "Payment does not match order" }, { status: 400 });
    if (payment.status !== "captured") return Response.json({ success: false, status: payment.status, message: "Payment verified but capture is pending. No paid access has been granted." }, { status: 202 });
    return Response.json({ success: true, payment_id: payment.id, message: "Test payment verified. Your subscription is unchanged." });
  } catch (error) { return providerError(error); }
}
