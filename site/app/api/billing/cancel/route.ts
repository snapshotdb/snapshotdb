import { getSession } from "@/app/console/auth";
import { account, sameOrigin, tenantFor } from "@/app/console/hosted";
import { razorpay } from "@/app/console/razorpay";

export async function POST(request: Request) {
  if (!sameOrigin(request)) return Response.json({ error: "Invalid origin" }, { status: 403 });
  const user = await getSession();
  if (!user) return Response.json({ error: "Sign in first" }, { status: 401 });
  try {
    const current = await account(tenantFor(user));
    if (!/^sub_[A-Za-z0-9]+$/.test(current.subscription)) return Response.json({ error: "No subscription found" }, { status: 409 });
    await razorpay(`subscriptions/${current.subscription}/cancel`, { cancel_at_cycle_end: true });
    return Response.json({ message: "Renewal cancelled. Pro remains available until the end of your paid period." });
  } catch (e) { return Response.json({ error: e instanceof Error ? e.message : "Cancellation failed" }, { status: 503 }); }
}
