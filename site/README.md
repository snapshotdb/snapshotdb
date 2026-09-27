# SnapshotDB website and console

The Next.js app serves the landing page, product docs, GitHub-authenticated console,
hosted API gateway, and Dodo checkout/webhooks. Database files and engines live on
the separate SnapshotDB server; deploying this app does not deploy that server.

## Local development

Use Node.js 24 (matching CI). From this directory:

```sh
npm ci
cp .env.local.example .env.local
npm run dev
```

Open <http://localhost:3000>. Public pages work without hosted credentials. Console
sign-in requires a GitHub OAuth app and a random `AUTH_SECRET`; use the callback
`http://localhost:3000/api/auth/github/callback` for a separate development OAuth app.
There is no unauthenticated demo-console fallback.

For a working hosted console, configure `SNAPSHOTDB_HOSTED_API` and the server-only
`SNAPSHOTDB_HOSTED_TOKEN` shared with an isolated development backend. Billing needs
the Dodo **test-mode** product, API key, and webhook secret. Never use the live service
or production cards as a development fixture. See [hosted configuration](../docs/hosting/deployment.md).

## Checks

```sh
npm run lint
npm test
npm run build
```

`npm run start` serves the production build. CI runs these checks plus the repository's
Linux database tests. See [AGENTS.md](AGENTS.md) for the installed Next.js documentation
to consult before changing framework behavior.

## Where to edit

| Area | Location |
|---|---|
| Landing page and motion | `app/page.tsx`, `app/Boot.tsx`, visual components |
| Split logo and app icon | `app/Logo.tsx`, `app/icon.svg` |
| Documentation | `app/docs/` |
| Console, session and billing helpers | `app/console/` |
| Authentication, hosted gateway and payment routes | `app/api/` |
| CLI installer and generated download bundle | `public/install.sh`, `public/dl/` |

## Deployment

Deployment is manual. Configure GitHub's production callback for the deployed origin,
use a long random session secret, and keep every API/payment credential server-side
(never `NEXT_PUBLIC_*`). The hosted token must differ from the BYOC administrative token.

Deploy compatible backend changes before this app. Building the website does not rebuild
the CLI downloads: use [the CLI packaging workflow](../docs/hosting/release-cli.md) and
verify the served `BUILD.json`, checksums, installed CLI revision, and backend health.

Before public paid signup, complete [launch acceptance](../docs/hosting/launch-checklist.md),
including real test-mode payments, infrastructure isolation, restore testing, and
[customer policies](../docs/hosting/customer-policies.md). A successful build is not
evidence that the live server or billing configuration works end to end.
