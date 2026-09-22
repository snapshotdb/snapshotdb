import crypto from "node:crypto";
import Razorpay from "razorpay";

export function paymentClient() {
  const key_id = process.env.RAZORPAY_KEY_ID;
  const key_secret = process.env.RAZORPAY_KEY_SECRET;
  if (!key_id || !key_secret) throw new Error("Razorpay is not configured");
  return new Razorpay({ key_id, key_secret });
}

export function checkoutPrice() {
  const amount = Number(process.env.RAZORPAY_CHECKOUT_AMOUNT || 100);
  const currency = process.env.RAZORPAY_CHECKOUT_CURRENCY || "INR";
  if (!Number.isSafeInteger(amount) || amount < 100 || !/^[A-Z]{3}$/.test(currency)) {
    throw new Error("Invalid checkout price configuration");
  }
  // This flow is a test payment, not a subscription purchase.
  if (!process.env.RAZORPAY_KEY_ID?.startsWith("rzp_test_")) throw new Error("Standard Checkout is currently test-mode only");
  return { amount, currency };
}

export function validPaymentSignature(orderId: string, paymentId: string, signature: string, secret: string): boolean {
  if (!secret || !/^[a-f0-9]{64}$/.test(signature)) return false;
  const expected = crypto.createHmac("sha256", secret).update(`${orderId}|${paymentId}`).digest();
  return crypto.timingSafeEqual(expected, Buffer.from(signature, "hex"));
}

export function providerError(error: unknown) {
  const status = (error as { statusCode?: number })?.statusCode === 401 ? 401 : 500;
  return Response.json({ error: status === 401 ? "Razorpay rejected the API credentials" : "Payment provider request failed. Please try again." }, { status });
}
