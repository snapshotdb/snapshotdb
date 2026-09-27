# Hosted pricing and AWS unit economics

Decision (2026-09-21): Free = 1 source database and 2 lifetime trial compute-hours.
Pro = USD 150/month, unlimited source count, 300 compute-hours per paid period,
50 GiB logical storage, 50 GiB database payload egress, 4 concurrent databases.
Free has 1 GiB storage/egress and 2 concurrent databases (source + branch).
Each running database, including a replicating source, is capped at 1 vCPU / 2 GiB.
All active database runtimes add together. SQLite counts while its HTTP endpoint
is running. Native idle engines stop after five minutes; source replication does
not idle. Suspended data still consumes storage. No automatic overage charges.

## Historical cost model — 21 September 2026

The plan limits above are implemented in `src/hosted.rs`. The figures below are dated
planning assumptions, not current provider quotes or measured production costs. Checkout
now uses Dodo; reprice collection, tax, FX, and infrastructure costs from the actual
merchant configuration before using this model for a launch decision.

Use Linux on-demand Graviton M7g, US East (N. Virginia), without credits, Spot,
reserved commitments, or assumed copy-on-write storage savings. AWS's published
example quotes m7g.xlarge (4 vCPU / 16 GiB) at $0.1632/hour:
https://aws.amazon.com/lambda/pricing/ (EC2 price in the Managed Instances example;
we use plain EC2 and do NOT add Lambda's 15% management premium).

At 60% average sold CPU capacity, cost per 1-vCPU database-hour =
0.1632 / (4 * 0.60) = $0.068. At 300 fully-used hours, compute = $20.40.
This requires a shared fleet, actual resource limits and scheduling; a single
customer on a dedicated always-on m7g.xlarge instead costs $119.14/month compute.

| Direct expense at full allowance | Monthly USD | Basis |
| --- | ---: | --- |
| Compute | 20.40 | 300 * $0.068 |
| Storage | 4.29 | 50 GiB = 53.69 decimal GB * $0.08, conservatively rounded |
| Database egress | 6.00 | Budget allowance, region/network dependent; excludes free-tier credit |
| Backups, API, logs, IP, load-balancer allocation | 8.00 | Planning reserve, not an AWS quote |
| Payment collection | 7.50 | Historical 5% planning reserve; not a Dodo fee quote |
| Total modeled direct cost | 46.19 | Excludes sales taxes on revenue |
| Contribution before salaries, acquisition and general overhead | 103.81 | 69.2% of $150 |

AWS gp3 storage source: https://aws.amazon.com/ebs/pricing/ ($0.08/GB-month
example). Provisioned EBS bills even when databases are idle. Extra IOPS and
throughput, snapshots, cross-AZ traffic, NAT and unused disks can raise costs.
AWS egress: https://aws.amazon.com/ec2/pricing/on-demand/ . The 100 GB free
allowance is shared across the AWS account; do not allocate it to every customer.
The original collection reserve predated the switch to Dodo. International fees,
subscription fees, taxes, refunds and FX depend on the merchant agreement; $7.50
is only a retained modeling assumption. Configure and verify the actual USD recurring
product using [deployment.md](deployment.md).

## Sensitivity

With the same $25.79 non-compute reserve:

| Included hours | Compute at 60% utilization | Total | Contribution margin |
| --- | ---: | ---: | ---: |
| 300 | $20.40 | $46.19 | 69.2% |
| 500 | $34.00 | $59.79 | 60.1% |
| 750 | $51.00 | $76.79 | 48.8% |

At 30% fleet utilization, 300 hours cost $40.80 compute; modeled margin falls
to 55.6%. An idle fleet or one oversized always-on server is a fixed bill even
when no customers use hours. Actual profitability is not guaranteed by this table.
Use measured p95 concurrency, source replication load, disk growth, and network
bills before increasing the allowance or buying a larger instance. Mumbai and
other AWS regions need their own quote. This model does not price RDS.

A source that replicates 24/7 consumes about 730 hours/month by itself. Pro 300 is
therefore an intermittent development plan. Always-on replication needs separately
priced compute or BYOC; do not market this plan as unlimited always-on databases.

Storage is currently measured as summed logical file sizes across copies,
conservatively charging shared reflink extents more than once. The watchdog samples
usage every five seconds; this is not an exact filesystem hard quota. Database
payload egress is metered, not packet overhead, replication traffic or control API
traffic. Use infrastructure disk/network caps in addition to application quotas.
