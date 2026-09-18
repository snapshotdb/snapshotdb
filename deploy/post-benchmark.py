#!/usr/bin/env python3
"""Verify retained benchmark databases, then run the repository regression suites."""
import argparse
import datetime
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--env-file', default='/etc/snapshotdb-tb.env')
    parser.add_argument('--service', default='snapshotdb-tb')
    parser.add_argument('--api', default='http://127.0.0.1:7433')
    parser.add_argument('--benchmark-report', default='/var/lib/snapshotdb-benchmark/report.json')
    parser.add_argument('--output', default='/var/lib/snapshotdb-benchmark/post-test-report.json')
    parser.add_argument('--repo', default='/opt/snapshotdb-src')
    parser.add_argument('--binary', default='/usr/local/bin/snapshotdb')
    parser.add_argument('--minimum-bytes', type=int, default=10**12)
    parser.add_argument('--skip-engine-suite', action='store_true')
    args = parser.parse_args()
    os.umask(0o077)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    report = {'state': 'running', 'passed': False, 'checks': [], 'failures': [], 'groups': {},
              'started_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    started = time.monotonic()
    env = os.environ.copy()
    env.pop('SNAPSHOTDB_INTERNAL', None)
    env['PATH'] = '/home/snapshotdb/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin'
    for line in Path(args.env_file).read_text().splitlines():
        if line and not line.startswith('#'):
            key, value = line.split('=', 1)
            env[key] = shlex.split(value)[0]
    env['PATH'] = '/home/snapshotdb/.cargo/bin:/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin'
    env['SNAPSHOTDB_SERVER'] = args.api
    client_home = Path('/srv/snapshotdb-data/tmp') / ('post-client-' + str(os.getpid()))
    env['SNAPSHOTDB_HOME'] = str(client_home)
    env['PGCONNECT_TIMEOUT'] = '15'
    env['TMPDIR'] = '/srv/snapshotdb-data/tmp'
    log = output.with_suffix('.log').open('w')
    created = []
    urls = {}
    subscription_paused = False

    def save():
        report['elapsed_seconds'] = round(time.monotonic() - started, 2)
        report['updated_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        temporary = output.with_suffix('.tmp')
        temporary.write_text(json.dumps(report, indent=2) + '\n')
        temporary.replace(output)

    def check(label, condition):
        if not condition:
            raise RuntimeError(label)
        report['checks'].append(label)
        print('PASS ' + label, flush=True)
        save()

    def run(command, sql=None, user=True, timeout=600, require=True, custom_env=None):
        argv = ['runuser', '-u', 'snapshotdb', '--', *command] if user else command
        try:
            result = subprocess.run(argv, input=sql, text=True, capture_output=True,
                                    env=custom_env or env, timeout=timeout, cwd=args.repo)
        except subprocess.TimeoutExpired:
            raise RuntimeError(Path(command[0]).name + ' timed out') from None
        log.write(result.stdout + '\n' + result.stderr + '\n')
        log.flush()
        if require and result.returncode:
            raise RuntimeError(Path(command[0]).name + ' failed; see private post-test log')
        return result

    def ab(*command, require=True):
        return run([args.binary, *command], require=require)

    def sql(name, statement):
        return run(['psql', '-XqAt', '-v', 'ON_ERROR_STOP=1', urls[name]], sql=statement, timeout=120).stdout.strip()

    def wait(label, predicate):
        for _ in range(120):
            if predicate():
                check(label, True)
                return
            time.sleep(1)
        raise RuntimeError('timed out: ' + label)

    def ready():
        request = urllib.request.Request(args.api + '/v1/health', headers={'Authorization': 'Bearer ' + env['SNAPSHOTDB_TOKEN']})
        try:
            with urllib.request.urlopen(request, timeout=3) as response:
                return response.status == 200
        except (OSError, urllib.error.URLError):
            return False

    save()
    try:
        benchmark = json.loads(Path(args.benchmark_report).read_text())
        check('scale benchmark passed at the requested physical size', benchmark.get('passed') is True
              and benchmark.get('source_table_bytes', 0) >= args.minimum_bytes)
        wait('retained API is ready', ready)
        try:
            urllib.request.urlopen(args.api + '/v1/health', timeout=5)
            unauthorized = False
        except urllib.error.HTTPError as error:
            unauthorized = error.code == 401
        check('retained API rejects missing authentication', unauthorized)
        for name in ('source', 'replica', 'branch-a', 'branch-b'):
            urls[name] = ab('url', name).stdout.strip()
        check('original branch-only writes survived handoff', sql('branch-a', 'SELECT value FROM public.scale_marker WHERE id=1;') == 'branch-only')
        check('original second branch stayed detached', sql('branch-b', 'SELECT value FROM public.scale_marker WHERE id=1;') == 'live')
        for name in ('source', 'replica'):
            check(name + ' retained latest benchmark write', sql(name, 'SELECT value FROM public.scale_marker WHERE id=1;') == 'later')
        # This dedicated fixture avoids overwriting any retained benchmark rows.
        table = 'public.post_check_' + str(time.time_ns())
        report['fixture_table'] = table
        sql('source', f'CREATE TABLE {table}(id integer PRIMARY KEY, value text);')
        sql('source', f"INSERT INTO {table} VALUES (1,'original'),(2,'original');")
        wait('new test table is published', lambda: ab('status', 'replica').returncode == 0
             and sql('replica', f"SELECT to_regclass('{table}') IS NOT NULL;") == 't')
        wait('new test table finishes initial copy', lambda: sql('replica', f'SELECT count(*) FROM {table};') == '2')
        wait('all retained tables are ready for branching', lambda: sql('replica',
             "SELECT count(*) > 0 AND bool_and(srsubstate='r') FROM pg_subscription_rel;") == 't')
        prefix = 'post-' + str(int(time.time()))
        a, b = prefix + '-a', prefix + '-b'
        for branch in (a, b):
            created.append(branch)
            urls[branch] = ab('create', branch, '--from', 'replica', '--print-url').stdout.strip()
            check(branch[-1] + ' is detached', sql(branch, 'SELECT count(*) FROM pg_subscription;') == '0')
            for table_name, count in benchmark['rows_per_table'].items():
                check(branch[-1] + ' retains large-table endpoint ' + table_name,
                      sql(branch, f'SELECT count(*) FROM {table_name} WHERE id IN (1,{count});') == '2')
        sql(a, f"UPDATE {table} SET value='branch-only' WHERE id=1; DELETE FROM {table} WHERE id=2; INSERT INTO {table} VALUES(3,'new');")
        for name in ('source', 'replica', b):
            check(name + ' isolated from scratch branch changes', sql(name, f"SELECT count(*)=2 AND bool_and(value='original') FROM {table};") == 't')
        ab('stop', a)
        check('scratch branch resumes on connection', sql(a, f'SELECT value FROM {table} WHERE id=1;') == 'branch-only')
        ab('lock', b)
        check('locked scratch branch refuses deletion', ab('rm', b, require=False).returncode != 0)
        ab('unlock', b)
        sql('replica', 'ALTER SUBSCRIPTION snapshotdb_replica DISABLE;')
        subscription_paused = True
        check('reset refuses a paused parent', ab('reset', a, require=False).returncode != 0)
        check('refused reset preserves branch changes', sql(a, f'SELECT value FROM {table} WHERE id=1;') == 'branch-only')
        sql('replica', 'ALTER SUBSCRIPTION snapshotdb_replica ENABLE;')
        subscription_paused = False
        sql('source', f"UPDATE {table} SET value='before-restart' WHERE id=1;")
        wait('source changes reach retained replica', lambda: sql('replica', f'SELECT value FROM {table} WHERE id=1;') == 'before-restart')
        ab('reset', a)
        urls[a] = ab('url', a).stdout.strip()
        check('reset adopts current replica data', sql(a, f'SELECT value FROM {table} WHERE id=1;') == 'before-restart')
        check('reset removes branch inserts and restores deletes', sql(a, f'SELECT string_agg(id::text,\',\' ORDER BY id) FROM {table};') == '1,2')
        run(['systemctl', 'restart', args.service], user=False)
        wait('retained API returns after service restart', ready)
        check('original branch survives service restart', sql('branch-a', 'SELECT value FROM public.scale_marker WHERE id=1;') == 'branch-only')
        sql('source', f"UPDATE {table} SET value='after-restart' WHERE id=1;")
        wait('replication continues after service restart', lambda: sql('replica', f'SELECT value FROM {table} WHERE id=1;') == 'after-restart')
        check('scratch branch stays isolated after restart', sql(a, f'SELECT value FROM {table} WHERE id=1;') == 'before-restart')
        check('client commands created no database directory', not client_home.exists())
    except Exception as error:
        report['failures'].append(str(error))
        save()
    finally:
        if subscription_paused:
            try:
                sql('replica', 'ALTER SUBSCRIPTION snapshotdb_replica ENABLE;')
            except Exception:
                report['failures'].append('could not re-enable retained replication')
        for name in reversed(created):
            try:
                ab('unlock', name, require=False)
                ab('rm', name)
            except Exception:
                report['failures'].append('scratch cleanup failed: ' + name)
        if created and not report['failures']:
            check('scratch branches cleaned up', all(name not in ab('list').stdout for name in created))
        if urls and not report['failures']:
            check('four original benchmark databases retained', all(name in ab('list').stdout for name in ('source', 'replica', 'branch-a', 'branch-b')))
        report['groups']['retained_database_checks'] = {'passed': not report['failures'], 'checks': len(report['checks'])}
        save()

    if not args.skip_engine_suite:
        regression_env = env.copy()
        for key in ('SNAPSHOTDB_HOME', 'SNAPSHOTDB_SERVER', 'SNAPSHOTDB_TOKEN'):
            regression_env.pop(key, None)
        commands = [
            ('rust', ['cargo', 'test', '--locked', '-j', '2']),
            ('python', ['python3', '-m', 'unittest', 'discover', '-s', 'scripts', '-p', 'test_*.py']),
            ('engines', ['bash', '-c', 'set -e; for p in pg_ctl psql mysqld mysql mongod mongosh mongodump mongorestore sqlite3; do command -v "$p" >/dev/null; done; export TMPDIR=/srv/snapshotdb-data/tmp; SNAPSHOTDB_HOME=$(mktemp -d "$TMPDIR/post-e2e.XXXXXX") ./e2e.sh'])]
        for group, command in commands:
            report['current_group'] = group
            save()
            try:
                result = run(command, custom_env=regression_env, timeout=7200)
                passed = result.returncode == 0
                count = None
                if group == 'engines':
                    count = result.stdout.count('  ok   ')
                    passed = passed and count >= 94 and 'all passed' in result.stdout and 'FAIL' not in result.stdout
                elif group == 'rust':
                    count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', result.stdout)))
                    passed = passed and count >= 8
                else:
                    match = re.search(r'Ran (\d+) tests', result.stderr)
                    count = int(match[1]) if match else 0
                    passed = passed and count >= 7
                report['groups'][group] = {'passed': passed, 'checks': count}
                if not passed:
                    report['failures'].append(group + ' suite failed or did not run all expected checks')
            except Exception as error:
                report['groups'][group] = {'passed': False}
                report['failures'].append(group + ': ' + str(error))
            save()
    report['passed'] = not report['failures']
    report['state'] = 'passed' if report['passed'] else 'failed'
    report.pop('current_group', None)
    save()
    log.close()
    print(json.dumps({'state': report['state'], 'checks': len(report['checks']), 'groups': report['groups']}), flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
