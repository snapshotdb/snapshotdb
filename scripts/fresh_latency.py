#!/usr/bin/env python3
"""Fresh branches from a live synced 1 TB replica, without pre-created capacity.

Every trial changes a source marker and waits for replication BEFORE requesting
a brand-new branch. Timed work includes CLI submission, cold engine startup,
URL retrieval, new authenticated connection, large-table read and committed write.
"""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import secrets
import statistics
import subprocess
import time
import psycopg


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',required=True)
    p.add_argument('--source',default='source')
    p.add_argument('--replica',default='replica')
    p.add_argument('--trials',type=int,default=20)
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--server-home',type=Path,required=True)
    p.add_argument('--minimum-bytes',type=int,default=10**12)
    p.add_argument('--skip-padding-check',action='store_true',help='Use for fixtures without the original 1 TB padding table')
    args=p.parse_args()
    env=dict(os.environ,SNAPSHOTDB_HOME=str(args.report.parent/'fresh-client-must-not-exist'))
    env.pop('SNAPSHOTDB_INTERNAL',None)
    prefix='fresh'+secrets.token_hex(4)
    report={'state':'running','passed':False,'prefix':prefix,'trials':[], 'checks':[],
            'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'measurement':'fresh CLI create from live replica -> URL -> new authenticated connection -> fresh marker + large-table read -> committed update -> read-back',
            'precreated_capacity':False,'client_location':'same EC2 host'}
    def save():
        temp=args.report.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(args.report)
    def check(ok,label):
        if not ok:raise RuntimeError(label)
        report['checks'].append(label)
    def cli(*command):
        r=subprocess.run([args.binary,*command],env=env,capture_output=True,text=True,timeout=300)
        if r.returncode:raise RuntimeError(command[0]+' failed: '+r.stderr[-1200:])
        return r.stdout.strip()
    def connect(url):return psycopg.connect(url,autocommit=True,connect_timeout=10)
    save()
    try:
        source_url=cli('url',args.source);replica_url=cli('url',args.replica)
        # Discover newly added source tables and finish their initial copy before
        # measuring a fresh branch. This wait is part of connector setup.
        deadline=time.monotonic()+120
        while True:
            cli('status',args.replica)
            with connect(replica_url) as replica:
                ready=replica.execute("SELECT count(*) FROM pg_subscription_rel WHERE srsubstate <> 'r'").fetchone()[0]==0
                if ready and args.skip_padding_check:break
                exists=replica.execute("SELECT to_regclass('public.scale_data_padding') IS NOT NULL").fetchone()[0]
                if ready and exists:
                    count=replica.execute('SELECT count(*) FROM public.scale_data_padding').fetchone()[0]
                    if count==32768:break
            if time.monotonic()>deadline:raise RuntimeError('new source table did not finish syncing before benchmark')
            time.sleep(.2)
        with connect(source_url) as source,connect(replica_url) as replica:
            sizes="SELECT coalesce(sum(pg_total_relation_size(c.oid)),0)::bigint FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname LIKE 'scale_data_%' AND c.relkind='r'"
            report['source_table_bytes']=source.execute(sizes).fetchone()[0]
            report['replica_table_bytes']=replica.execute(sizes).fetchone()[0]
            check(report['source_table_bytes']>=args.minimum_bytes,'source is at least the requested size')
            check(report['replica_table_bytes']>=args.minimum_bytes,'synced replica is at least the requested size')
            table=prefix+'_probe'
            source.execute(f'CREATE TABLE public.{table}(id integer PRIMARY KEY,value text NOT NULL)')
            source.execute(f'INSERT INTO public.{table} VALUES(1,%s)',['initial'])
            expected=source.execute('SELECT md5(payload) FROM public.scale_data_0 WHERE id=1').fetchone()[0]
        for i in range(args.trials):
            nonce=secrets.token_hex(12);name=f'{prefix}-{i}'
            with connect(source_url) as source:
                source.execute(f'UPDATE public.{table} SET value=%s WHERE id=1',[nonce])
            deadline=time.monotonic()+120
            while True:
                cli('status',args.replica)
                try:
                    with connect(replica_url) as replica:
                        row=replica.execute(f'SELECT value FROM public.{table} WHERE id=1').fetchone()
                        if row and row[0]==nonce:break
                except psycopg.Error:pass
                if time.monotonic()>deadline:raise RuntimeError('replication did not catch up before trial')
                time.sleep(.1)
            check(not (args.server_home/name).exists(),f'trial {i}: branch did not exist before request')
            started=time.perf_counter()
            url=cli('create',name,'--from',args.replica,'--print-url')
            url_ms=(time.perf_counter()-started)*1000
            with connect(url) as branch:
                check(branch.execute(f'SELECT value FROM public.{table} WHERE id=1').fetchone()[0]==nonce,f'trial {i}: branch has latest pre-request source marker')
                check(branch.execute('SELECT md5(payload) FROM public.scale_data_0 WHERE id=1').fetchone()[0]==expected,f'trial {i}: actual large table readable')
                branch.execute(f'UPDATE public.{table} SET value=%s WHERE id=1',['agent-'+nonce])
                check(branch.execute(f'SELECT value FROM public.{table} WHERE id=1').fetchone()[0]=='agent-'+nonce,f'trial {i}: committed write readable')
                end_ms=(time.perf_counter()-started)*1000
            report['trials'].append({'index':i,'url_ms':round(url_ms,3),'read_write_ms':round(end_ms,3)})
            with connect(source_url) as source,connect(replica_url) as replica:
                for label,db in [('source',source),('replica',replica)]:
                    check(db.execute(f'SELECT value FROM public.{table} WHERE id=1').fetchone()[0]==nonce,f'trial {i}: {label} isolated')
            # Keep the final two fresh branches for user inspection; scratch trials
            # are deleted only after timing and all isolation checks.
            if i<args.trials-2:cli('rm',name)
            save();print(json.dumps(report['trials'][-1]),flush=True)
        values=sorted(t['read_write_ms'] for t in report['trials'])
        import math
        report['summary']={'n':len(values),'p50_ms':statistics.median(values),'p95_ms':values[math.ceil(len(values)*.95)-1],'max_ms':max(values),'all_under_6s':all(v<6000 for v in values),'all_under_1s':all(v<1000 for v in values)}
        check(not Path(env['SNAPSHOTDB_HOME']).exists(),'client created no database storage')
        report.update(state='passed',passed=True)
    except Exception as exc:
        report.update(state='failed',error=str(exc));raise
    finally:save()


if __name__=='__main__':main()
