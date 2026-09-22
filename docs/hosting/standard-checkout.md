# Razorpay Standard Checkout (test mode)

The Next.js console has a one-time Standard Checkout flow alongside recurring
Pro subscriptions. It does not activate or renew Pro. The button is in Console →
Plan & usage → Test payment · ₹1. BYOC does not show hosted payment controls.

Run `npm install` and `npm run dev` from `site`, configure the existing GitHub
login, sign in, and open the console. `site/.env` and `site/.env.local` are ignored
by Git; both currently contain the supplied test credentials. Next.js gives
`.env.local` precedence. Never commit either file.

Required server variables:

- RAZORPAY_KEY_ID (test key)
- RAZORPAY_KEY_SECRET
- RAZORPAY_CHECKOUT_AMOUNT=100 (smallest currency units)
- RAZORPAY_CHECKOUT_CURRENCY=INR

The key ID is returned by the order endpoint for Checkout. The key secret is
server-only; no NEXT_PUBLIC secret is needed. Configure these variables on the
hosting platform separately before deployment.

POST /api/create-order requires a session and matching Origin. It creates a
server-priced order with a generated receipt, tenant owner and purpose in
Razorpay notes. Client-supplied amount, currency and receipt are ignored to prevent
price manipulation. Invalid configured amounts below 100 fail closed. The API
returns order_id, amount, currency, and key_id.

The browser loads https://checkout.razorpay.com/v1/checkout.js and handles modal
dismissal, SDK load failures and payment.failed. Success sends the three Razorpay
fields to POST /api/verify-payment. Verification requires the same account,
constant-time HMAC-SHA256 comparison, a matching provider order and payment,
matching amount/currency, and captured status. Authorized-but-not-captured returns
202 with success=false. Repeated verification has no entitlement side effects.
Razorpay stores the order/payment records; no new database tables are created.

Use the supplied Razorpay test card or test UPI in the modal. Confirm successful
verification, then test cancellation and failed payment. Configure automatic
capture in Razorpay test settings, or capture an authorized test payment through
the dashboard and retry verification. No live payments are enabled by this flow.

Validation performed: real test order creation with Razorpay succeeded; automated
signature/tampering, amount/configuration and error-response tests pass. A full
interactive card/UPI payment has not yet been completed. Existing recurring Pro
checkout still needs its own USD plan and signed webhook configuration.
