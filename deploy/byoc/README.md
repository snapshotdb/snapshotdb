# SnapshotDB BYOC appliance

Deploy the CLI/server binary and database engines in the customer's account. The
client sends requests; initial copies and writable branches stay on that server.
The appliance is a Linux VM, with a separately attached Btrfs data volume for
copy-on-write branches. An ordinary Docker overlay filesystem is not a supported
replacement for this storage or the engine namespace sandbox.

The AWS Packer recipe builds an **account-private** Ubuntu 24.04 amd64 AMI. It
installs PostgreSQL 16 and SQLite (`postgres` profile), or also MySQL 8 and MongoDB 8
(`all` profile). Source providers are connection profiles, not different VM images.
The artifact records package versions and the binary SHA256 under
`/usr/share/snapshotdb`. Ubuntu package updates mean two builds can differ; retain
the AMI ID and package inventory with every benchmark/release.

## Build

Run from the repository root with Packer and AWS credentials for the target account.
The archive must contain only reviewed source, including this directory. Never
archive `.local`, `.env`, SSH keys or an existing deployment's data directory.

```sh
git archive --format=tar.gz HEAD > /tmp/snapshotdb-source.tar.gz
packer init deploy/byoc/aws.pkr.hcl
packer validate -var source_archive=/tmp/snapshotdb-source.tar.gz \
  -var revision="$(git rev-parse --short HEAD)" deploy/byoc/aws.pkr.hcl
packer build -var source_archive=/tmp/snapshotdb-source.tar.gz \
  -var revision="$(git rev-parse --short HEAD)" \
  -var engine_profile=all deploy/byoc/aws.pkr.hcl
```

Packer uses a temporary m6i.xlarge builder, encrypted 24 GiB root volume and SSH
restricted to the builder operator's public IP. It deletes the temporary builder
after image creation. The AMI and its snapshot remain in your account. No SnapshotDB
API token or source credentials are installed in the image. No access to an
SnapshotDB-hosted control plane is required.

## Launch and initialize

Launch the resulting AMI in the customer's VPC with IMDSv2 required, encrypted EBS,
and SSH reachable only from the operator/VPN. Attach and mount a new Btrfs data
volume, with enough capacity for the initial replica, WAL and branch changes.
Format only a specifically identified **new empty** volume, never an existing
production or benchmark disk. The initializer deliberately does not format disks.
Make the mount persistent in `/etc/fstab` before starting the server.

Alternatively, deploy `aws.yaml` with CloudFormation, supplying the built AMI,
VPC, subnet, its Availability Zone, SSH key and operator CIDR. That template creates
an encrypted gp3 data volume and identifies it by its exact EBS serial before
formatting. It retains that volume when the stack is deleted or replaced. It does
not expose API/database ports publicly. Check `cloud-init status --wait` and the
service health after stack creation: CloudFormation resource completion alone is
not an application readiness check. Restoring a retained volume is an operator
recovery procedure; the fresh-install initializer refuses to format/reinitialize it.

```sh
sudo snapshotdb-initialize /mnt/snapshotdb-data
sudo systemctl status snapshotdb
```

The initializer requires an empty directory, verifies reflinks, creates an isolated
server directory, generates a fresh token and waits for authenticated API health.
It refuses repeated initialization or replacement of existing storage/secrets.
The service stays disabled until initialization. Retrieve the token privately from
`/etc/snapshotdb.env`; it is not printed in deployment logs.

The initial appliance binds API and native database ports to **loopback**. Run the
agent inside the VM, or use an authenticated SSH/VPN gateway with routing for the
returned database port. A single API port forward does not forward database ports.
For a public service, HTTPS control-plane termination and native database TLS
routing are additional deployment work; this image does not provide them yet.

On the VM, after securely supplying the token to the client environment:

```sh
export SNAPSHOTDB_SERVER=http://127.0.0.1:7432
snapshotdb preflight postgres "$SOURCE_URL" --format json
snapshotdb clone production "$SOURCE_URL"
snapshotdb status production --format json
# Wait for initial replication to complete before benchmarking branches.
snapshotdb create agent-1 --from production --print-url
```

Use an application schema selection for provider-managed databases; copying an
entire hosted service's internal schemas does not reproduce that service. Branch
ownership is reassigned to an agent role, so source permission/RLS fidelity must
be checked against the application's requirements.

## Source compatibility status

These are qualification requirements, **not claims of completed live provider
certification**. The all-engine E2E suite uses self-hosted database fixtures.

| Source | Appliance | Required qualification |
| --- | --- | --- |
| Self-hosted PostgreSQL | postgres/all | Tested engine path; matching tools/extensions, logical WAL, publication and DDL permissions |
| RDS PostgreSQL | postgres/all | Enable `rds.logical_replication`; replication/admin grants, security group access and event-trigger checks |
| Supabase PostgreSQL | postgres/all | Direct database endpoint, replication and privileged event-trigger role, selected application schemas; separately test RLS/extensions |
| Neon PostgreSQL | postgres/all | Enable logical replication, direct endpoint and publisher role; current preflight does not recognize `neon_superuser` as an event-trigger-capable role, so automatic DDL requires further qualification |
| PlanetScale PostgreSQL | postgres/all | Logical CDC is documented; replication/event-trigger privileges, version and extensions require live qualification |
| PlanetScale Vitess/MySQL | all | **Unverified**: MySQL wire compatibility does not prove access to the native GTID/binlog replication that this implementation requires |
| RDS MySQL | all | Native GTID/binlog availability, ROW format, replication grants and dump compatibility require live qualification |
| MongoDB replica set | all | Change-stream permissions and dump/restore tools; managed-provider policies require separate qualification |

Provider references: [RDS logical replication](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/PostgreSQL.Concepts.General.FeatureSupport.LogicalReplication.html),
[Supabase replication](https://supabase.com/docs/guides/database/replication),
[Neon logical replication](https://neon.com/docs/guides/logical-replication-neon),
[PlanetScale PostgreSQL CDC](https://planetscale.com/docs/postgres/integrations/logical-cdc).

This is a single-administrator appliance. It does not yet provide a customer web
dashboard, per-user policies, compute quotas, HA or automatic capacity scaling.
See [security boundaries](../../docs/security-hardening.md) and the
[strict comparison protocol](../../docs/strict-comparison.md).
