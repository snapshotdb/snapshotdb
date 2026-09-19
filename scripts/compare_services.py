#!/usr/bin/env python3
"""Alternating API benchmark with measured readiness, cleanup and fairness gates.

Run on one Linux client with direct access to BOTH returned database endpoints.
Credentials come only from environment variables. Reports never include URLs,
tokens, SQL exception messages or raw API responses. Requires psycopg 3.
"""
import argparse
import datetime
import json
import math
import os
from pathlib import Path
import random
import statistics
import time
import urllib.request
import uuid


def fairness_gaps(manifest):
    """Fail closed: unknown/missing resource evidence cannot establish parity."""
    gaps = []
    fields = ("region", "availability_zone", "instance_type", "instance_count",
              "cpu_limit", "memory_limit_bytes", "storage_type", "storage_iops",
              "storage_mib_per_second", "postgres_version", "durability",
              "background_load", "prepared_capacity", "transport")
    for field in fields:
        values = [manifest.get(p, {}).get(field) for p in ("snapshotdb", "ardent")]
        if any(v is None or v == "unknown" or v == "" for v in values):
            gaps.append(field + ": unknown")
        elif values[0] != values[1]:
            gaps.append(field + ": differs")
    for p in ("snapshotdb", "ardent"):
        if not manifest.get(p, {}).get("evidence"):
            gaps.append(p + ": configuration evidence missing")
    return gaps


def summary(rows, key):
    values = sorted(r[key] for r in rows if r.get("success") and key in r)
    return {"attempts": len(rows), "successes": sum(bool(r.get("success")) for r in rows),
            "failures": sum(not r.get("success") for r in rows),
            "timed_samples": len(values),
            **({"min_ms": values[0], "p50_ms": statistics.median(values),
                "p95_ms": values[math.ceil(.95 * len(values)) - 1],
                "p99_ms": values[math.ceil(.99 * len(values)) - 1],
                "max_ms": values[-1]} if values else {})}


class API:
    def __init__(self, base, token, poll, timeout):
        self.base, self.token, self.poll, self.timeout = base.rstrip('/'), token, poll, timeout

    def request(self, path, body=None, method=None):
        req = urllib.request.Request(self.base + path,
            data=json.dumps(body).encode() if body is not None else None, method=method,
            headers={"Authorization": "Bearer " + self.token,
                     "Content-Type": "application/json", "X-Idempotency-Key": str(uuid.uuid4())})
        with urllib.request.urlopen(req, timeout=min(30, self.timeout)) as response:
            return json.load(response)

    def wait(self, provider, submitted):
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            if provider == 'snapshotdb':
                result = self.request('/v1/jobs/' + submitted['id'])
                if result['state'] == 'done':
                    if result['exit_code'] != 0:
                        raise RuntimeError('job_failed')
                    return result
            else:
                result = self.request('/v1/operations/' + submitted['operation_id'])
                if result['status'] in ('failed', 'cancelled'):
                    raise RuntimeError('operation_failed')
                if result['status'] == 'completed':
                    return result.get('result') or {}
            time.sleep(self.poll)
        raise TimeoutError('operation_timeout')


def main():
    import psycopg
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--snapshotdb-api', default='http://127.0.0.1:7432')
    p.add_argument('--root', required=True)
    p.add_argument('--ardent-connector', required=True)
    p.add_argument('--manifest', type=Path, required=True)
    p.add_argument('--report', type=Path, required=True)
    p.add_argument('--trials', type=int, default=30)
    p.add_argument('--seed', type=int, default=20260913)
    p.add_argument('--poll', type=float, default=.1)
    p.add_argument('--timeout', type=float, default=180)
    p.add_argument('--minimum-bytes', type=int, required=True)
    p.add_argument('--require-infrastructure-parity', action='store_true')
    a = p.parse_args()
    if a.trials < 1 or a.poll <= 0 or a.timeout <= 0 or a.minimum_bytes < 1:
        p.error('trials, poll, timeout and minimum-bytes must be positive')
    manifest = json.loads(a.manifest.read_text())
    gaps = fairness_gaps(manifest)
    if a.require_infrastructure_parity and gaps:
        p.error('Infrastructure parity not established: ' + '; '.join(gaps))
    apis = {'snapshotdb': API(a.snapshotdb_api, os.environ['SNAPSHOTDB_TOKEN'], a.poll, a.timeout),
            'ardent': API('https://api.tryardent.com', os.environ['ARDENT_TOKEN'], a.poll, a.timeout)}
    report = dict(started_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
        state='running', mode='fresh', manifest=manifest, infrastructure_gaps=gaps,
        infrastructure_parity=not gaps, seed=a.seed, poll_seconds=a.poll,
        measurement='API submission -> returned URL -> new connection -> sampled data -> committed write/read-back',
        limitations=['Does not time initial setup, reset, suspend or concurrent load.',
                     'Source must be quiescent for this frozen-content run; latest-write propagation is a separate test.',
                     'Infrastructure parity uses supplied evidence; it is not inferred from latency.'],
        trials=[], cleanup=[])
    a.report.parent.mkdir(parents=True, exist_ok=True)

    def save():
        tmp = a.report.with_suffix('.tmp')
        tmp.write_text(json.dumps(report, indent=2) + '\n')
        tmp.replace(a.report)

    def connect(url):
        return psycopg.connect(url, autocommit=True, connect_timeout=10)

    def inventory(conn):
        return conn.execute("SELECT c.relname, pg_total_relation_size(c.oid) "
            "FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace "
            "WHERE n.nspname='public' AND c.relname ~ '^scale_data_[0-7]$' "
            "AND c.relkind='r' ORDER BY c.relname").fetchall()

    def content(conn):
        # Fixed beginning/middle/end samples in every table; indexes keep this cheap.
        rows = []
        for i in range(8):
            table = psycopg.sql.Identifier('public', 'scale_data_' + str(i))
            lo, hi = conn.execute(psycopg.sql.SQL('SELECT min(id), max(id) FROM {}').format(table)).fetchone()
            values = conn.execute(psycopg.sql.SQL('SELECT id, md5(payload) FROM {} WHERE id = ANY(%s) ORDER BY id').format(table),
                                  [[lo, (lo + hi) // 2, hi]]).fetchall()
            rows.append([i, lo, hi, values])
        return rows

    source_url = os.environ['BENCHMARK_SOURCE_URL']
    with connect(source_url) as source:
        inv = inventory(source)
        if len(inv) != 8 or sum(row[1] for row in inv) < a.minimum_bytes:
            raise RuntimeError('source_dataset_does_not_meet_size_requirement')
        expected = content(source)
        report['source_inventory'] = inv
        report['source_content_samples'] = expected
    save()
    rng = random.Random(a.seed)
    for index in range(a.trials):
        order = ['snapshotdb', 'ardent']
        rng.shuffle(order)
        for provider in order:
            api = apis[provider]
            name = 'strict-' + uuid.uuid4().hex[:16]
            row = dict(provider=provider, index=index, name=name, success=False)
            report['trials'].append(row)
            branch_id = None
            started = time.perf_counter()
            try:
                if provider == 'snapshotdb':
                    submitted = api.request('/v1/commands', ['create', name, '--from', a.root, '--print-url'])
                else:
                    submitted = api.request('/v1/branch/create', dict(connector_id=a.ardent_connector, service_type='postgres', name=name))
                row['operation_id'] = submitted.get('operation_id') or submitted.get('id')
                result = api.wait(provider, submitted)
                if provider == 'snapshotdb':
                    url = result['stdout'].strip()
                else:
                    branch = result.get('branch') or result
                    branch_id = branch.get('id') or result.get('branch_id')
                    url = branch.get('branch_url') or result['branch_url']
                row['url_ms'] = (time.perf_counter() - started) * 1000
                with connect(url) as branch:
                    row['connect_ms'] = (time.perf_counter() - started) * 1000
                    row['database_tls'] = branch.execute('SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()').fetchone()[0]
                    assert branch.execute('SELECT md5(payload) FROM public.scale_data_0 WHERE id=%s', [expected[0][1]]).fetchone()[0] == expected[0][3][0][1], 'payload_mismatch'
                    branch.execute('CREATE TABLE public.strict_agent_probe (id int PRIMARY KEY, value text)')
                    with branch.transaction():
                        branch.execute('INSERT INTO public.strict_agent_probe VALUES (1,%s)', [name])
                    assert branch.execute('SELECT value FROM public.strict_agent_probe WHERE id=1').fetchone()[0] == name, 'write_mismatch'
                    row['read_write_ms'] = (time.perf_counter() - started) * 1000
                    assert content(branch) == expected, 'content_mismatch'
                    assert len(inventory(branch)) == 8, 'table_inventory_mismatch'
                    row['branch_inventory'] = inventory(branch)
                    row['superuser'] = branch.execute('SELECT rolsuper FROM pg_roles WHERE rolname=current_user').fetchone()[0]
                    assert not row['superuser'], 'agent_is_superuser'
                with connect(url) as fresh:
                    assert fresh.execute('SELECT value FROM public.strict_agent_probe WHERE id=1').fetchone()[0] == name, 'commit_not_visible'
                with connect(source_url) as source:
                    assert source.execute("SELECT to_regclass('public.strict_agent_probe')").fetchone()[0] is None, 'source_isolation_failed'
                row['success'] = True
            except Exception as exc:
                # SQL/network errors often contain credentials or server paths.
                row['error_type'] = type(exc).__name__
                if isinstance(exc, AssertionError):
                    row['failed_check'] = str(exc)
                row['elapsed_ms'] = (time.perf_counter() - started) * 1000
            finally:
                save()
                cleanup = dict(provider=provider, name=name, success=False)
                report['cleanup'].append(cleanup)
                try:
                    deletion_start = time.perf_counter()
                    if provider == 'snapshotdb':
                        submitted = api.request('/v1/commands', ['rm', name])
                    elif branch_id:
                        submitted = api.request('/v1/cli/branches/' + branch_id, method='DELETE')
                    else:
                        raise RuntimeError('no_branch_id_cleanup_requires_reconciliation')
                    cleanup['operation_id'] = submitted.get('operation_id') or submitted.get('id')
                    save()
                    api.wait(provider, submitted)
                    cleanup.update(success=True, operation_complete_ms=(time.perf_counter() - deletion_start) * 1000)
                except Exception as exc:
                    cleanup['error_type'] = type(exc).__name__
                save()
            print(json.dumps({k: row[k] for k in ('provider', 'index', 'success', 'url_ms', 'read_write_ms', 'error_type') if k in row}), flush=True)
    report['summary'] = {provider: summary([r for r in report['trials'] if r['provider'] == provider], 'read_write_ms') for provider in apis}
    report['state'] = 'passed' if all(r['success'] for r in report['trials'] + report['cleanup']) else 'failed'
    report['completed_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    save()
    return 0 if report['state'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
