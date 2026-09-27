# Customer-policy requirements before hosted launch

This is an operator checklist, not published service terms or a privacy policy.
Do not expose placeholder contacts or promise deletion/backup behavior that has not
been implemented. Self-hosted operators control their own infrastructure and policies.

## Publish and verify

| Page | Facts to establish before publication |
|---|---|
| `/support` | A monitored private contact for account, billing, security, and deletion requests; a tested inbound message and response path |
| `/privacy` | Legal operator and hosting region; data collected; purposes, processors, access, and implemented retention/deletion periods |
| `/terms` | Operator identity, service scope, acceptable use, customer responsibilities, limits, and applicable terms |
| `/refund` | Actual price/interval, renewal and cancellation behavior, refund request process, and handling of mandatory rights |

Link the pages from the deployed footer and billing flow. Review the copy against the
actual merchant setup and applicable requirements before publication. Public GitHub Issues
are appropriate for reproducible bugs and feature requests, not credentials, private
database contents, account records, or security disclosures.

## Behavior supported by current source

- Hosted replication copies source data and stores source credentials on the service
  host. BYOC stores those files on the customer's server. Restrictive file permissions
  are not proof of encryption at rest.
- GitHub sign-in uses the account ID, login, optional name, and avatar in a signed
  seven-day session (`site/app/console/session.ts`). Session expiry does not delete a workspace.
- Branch deletion stops its proxy and database and removes the active branch directory
  (`src/main.rs`). Independent branches, backups, provider records, and source-side
  resources whose cleanup failed are separate. This is not secure physical erasure.
- Stopping compute or cancelling renewal does not delete stored databases.
- Completed job files older than 24 hours are pruned when a new job starts
  (`src/remote.rs`). Running jobs remain. This does not establish retention for engine,
  proxy, authentication, billing, or infrastructure logs.
- Dodo handles checkout and subscription management. The service uses customer,
  subscription, and event identifiers for entitlements; provider retention must be
  documented from the actual merchant setup.

## Retention and deletion implementation

Inventory active database files, source credentials, billing/account records, job and
engine logs, backups, and external processors. Assign each an owner, retention period,
deletion mechanism, and verification step. Implement the chosen periods before promising
them. Account deletion needs authenticated identity verification and a tracked process;
branch deletion alone does not close an account or remove billing records.

Backups need a bounded rotation policy and a restore procedure that does not silently
restore previously deleted active data. Test a restore and expiry of retained backups.
State any documented exceptions separately, rather than promising immediate erasure
from every storage layer.

Record the completed policies and their deployed URLs in the
[launch acceptance evidence](launch-checklist.md). Policy publication does not replace
the isolation, transport, payment, and recovery checks in that checklist.
