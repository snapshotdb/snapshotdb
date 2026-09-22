# Hosted console and Razorpay rollout

The console defaults to a same-origin authenticated gateway at /api/hosted/*.
The gateway forwards to https://api.snapshotdb.io using a server-only secret.
A GitHub numeric user id selects .tenants/github-ID under SNAPSHOTDB_HOME.
Old sessions lacking the numeric id must sign out and back in. Existing BYOC/admin
inventory is not automatically assigned to a hosted customer.

## Configuration

On the Next.js server set AUTH_SECRET, GitHub OAuth settings, and:

- SNAPSHOTDB_HOSTED_API=https://api.snapshotdb.io
- SNAPSHOTDB_HOSTED_TOKEN: new random secret of at least 32 characters
- RAZORPAY_KEY_ID and RAZORPAY_KEY_SECRET
- RAZORPAY_PRO_PLAN_ID: a USD 15000-subunit, monthly, interval-1 plan
- RAZORPAY_WEBHOOK_SECRET

On the Oracle/AWS API server set the same SNAPSHOTDB_HOSTED_TOKEN, distinct from
SNAPSHOTDB_TOKEN (the operator/BYOC admin credential). Never send either secret
to hosted browsers or put them in NEXT_PUBLIC variables. Keep the admin credential
private. Selecting BYOC does not grant access to hosted data: users must supply
credentials to their own server.

Hosted compute requires Linux, bubblewrap, all database executables, and a
writable delegated cgroup-v2 subtree in SNAPSHOTDB_CGROUP_ROOT with cpu, memory
and pids controllers enabled. Each tenant/database child cgroup gets cpu.max
100000/100000, memory.max 2 GiB, swap 0 and pids.max 512. Merely setting an ordinary
folder is insufficient. Use a properly delegated systemd subtree with the control
process in a separate leaf (the cgroup no-internal-process constraint applies).
BYOC without hosted identity retains existing behavior. Test real engine launches
on Linux before rollout; macOS tests cannot validate bubblewrap/cgroup execution.

Install mongosh, mongod, mongodump and mongorestore for MongoDB. Missing-client
preflight errors now identify a server dependency instead of blaming credentials.

Configure Razorpay's webhook URL as:
https://www.snapshotdb.io/api/billing/webhook
Subscribe to subscription.charged. Only a signed captured full-price USD payment
for the configured plan grants a paid period. Browser redirects never grant Pro.
Duplicate and older events do not reset usage. Cancellation at cycle end leaves
the paid-through date intact; failed renewals cannot extend it. Refunds and chargeback
revocation still require operator handling. Test-mode keys/plan/webhook must be
kept separate from live values. Checkout fails closed if the plan currency/amount
is wrong. No live payment credentials are included in this repository.

## Deployment validation

1. Deploy API support before the web gateway, configure cgroup delegation and
   keep a backup of existing data. Configure both secrets without exposing values.
2. Use two GitHub accounts: each should start with an empty, separate inventory;
   job ids from the other workspace must return 404.
3. Create a small source and branch with the required replication permissions.
   Confirm a second Free source is rejected, then test source deletion at quota.
4. Exercise five-minute idle suspension, reconnect, storage/transfer limits,
   two-hour exhaustion (test ledger), graceful restart, and Linux cgroup limits.
5. Run a Razorpay test payment, replay its webhook, cancel renewal, and test a
   renewal. Confirm usage resets only for a new paid period, not a replay.
6. Switch to BYOC; only then should server address/token inputs be visible.

## Production boundaries

Application workspace/job isolation is not a replacement for network isolation.
Hosted preflight and replication connect to user-supplied URLs: provision an egress
policy blocking metadata endpoints, private/control services and other tenants.
Native database URLs currently inherit the existing TCP transport; terminate TLS
or provide private-network access before exposing sensitive hosted data. Retained
Postgres replication slots may accumulate source WAL while quota-paused; configure
bounded source WAL retention and explain resync requirements. Logical storage is
sampled rather than a kernel disk limit: use per-workspace filesystem quotas before
untrusted production workloads. No existing production data is migrated by this patch.

## Deployment record — 2026-09-21

Deployed the hosted backend to the existing Oracle anybranch.service, with
cgroup-v2 delegation and an HTTPS-only API listener behind Caddy. Previous binary
is /usr/local/bin/snapshotdb.pre-hosted; previous service file is
/etc/systemd/system/anybranch.service.pre-hosted. Hosted service configuration is
/etc/systemd/system/anybranch.service.d/hosted.conf; secrets are in the root-only
/etc/snapshotdb-hosted.env. Database volume is Btrfs. Tests must set TMPDIR to a
Btrfs directory; ordinary /tmp does not support the required reflinks.

All 14 Rust tests passed on Oracle. Live smoke checks passed for hosted SQLite
startup with cgroup limits, source quota, tenant inventory isolation and deletion.
The site deployment site-mfb17cuqt-alphawaves-projects.vercel.app was promoted to
production. Its authenticated gateway returned the backend Free-plan account.
Ten site tests and the production build passed. AUTH_SECRET was rotated; existing
sessions must sign in again. Local environment files are excluded from Vercel uploads.

Limitations: Oracle currently has about 1 GiB RAM, below the advertised paid-plan
capacity. No live end-to-end PostgreSQL/MySQL/MongoDB replication smoke tests or
complete Razorpay card/UPI payment were performed in this rollout. Latest supplied
Razorpay test credentials initially created an order but subsequently returned
401 from both the local machine and deployed route. USD subscription plan creation
also failed; a valid plan and dashboard webhook setup remain outstanding. Do not
represent payment activation as working until these are resolved. The production
boundaries above still apply.
