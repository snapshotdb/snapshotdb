# Hosted launch acceptance

Code checks are not proof of a deployed service. Complete this checklist against
the actual release before enabling paid public signup. Record the commit, UTC
time, and redacted evidence for each item. Deployment is manual.

## Release and billing

- Merge the launch fixes, rebuild backend and all downloadable CLI binaries from
  that merged revision, and deploy the backend before the site. Do not reuse the
  older binaries currently tracked under `site/public/dl`.
- Confirm the served CLI checksums and the running backend revision match the
  intended build. Preserve the previous release for rollback.
- In **Dodo test mode**, verify checkout, signed webhook delivery, duplicate and
  out-of-order delivery, renewal, failed payment, portal recovery, scheduled
  cancellation, and expiry. Verify the correct GitHub workspace changes plan and
  that another workspace does not. Do not substitute a checkout-page screenshot
  for a completed payment-to-entitlement test.
- Configure the live $150 USD monthly product and matching live webhook secret
  separately. Never use production cards for automated tests. Follow
  [deployment.md](deployment.md) for billing environment variables.

## Server isolation and durability — required before shared hosting

- Enforce egress isolation at connection time for **all** tenant database clients,
  dump/restore tools, replication workers, and database engines. Deny cloud
  metadata, private/link-local ranges, control-plane services, and other tenants'
  database ports/sockets. Test both IPv4 and IPv6, DNS rebinding, and MongoDB
  replica discovery using disposable canary services. Filesystem sandboxing and
  initial URL validation do not establish this boundary. Keep hosted signup
  closed if these negative tests have not passed.
- Verify native database and SQLite traffic use a private encrypted route or a
  compatible TLS gateway. The control API's HTTPS certificate does not protect
  the separate database ports; native proxies do not add TLS themselves.
- Install and verify the Linux filesystem sandbox and per-tenant CPU/memory
  limits. Verify host capacity under concurrent imports and branch workloads;
  four allowed databases per Pro tenant are not a host capacity guarantee.
- Enforce filesystem hard limits per tenant and monitor total free disk. Periodic
  logical-size checks alone cannot contain a sudden write burst or shared-disk
  exhaustion. Exercise the full-disk path on disposable storage.
- Back up account/billing state and database data consistently; restore onto a
  clean host and verify queries, credentials, entitlement, and usage. Test restart
  during import and branch creation; inspect interrupted mutations before retry.
- Alert on failed jobs, webhook failures, disk pressure, process restarts, quota
  enforcement failures, and source WAL retention. Set a bounded Postgres
  `max_slot_wal_keep_size`; a quota-stopped replica still retains its source slot.

## Customer-facing launch decisions

- Publish approved privacy, service terms, refund/cancellation terms, a working
  support contact, and a specific data-retention/deletion policy. These require
  owner decisions and are not supplied by the code fixes.
- Make `snapshotdb/snapshotdb` public when ready, or remove public open-source
  calls to action until the repository is accessible without signing in.
- State that CLI access currently uses BYOC server URL/token credentials; GitHub
  CLI sign-in alone does not connect the CLI to the hosted console workspace.
- Test sign-in, first source, preflight errors, sync readiness, branch creation,
  actual SQL read/write, stop/start/reset/delete, and quota recovery across all
  supported engines. Repeat on phone and desktop. Use disposable databases, not
  customer production data.
