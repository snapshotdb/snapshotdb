#!/usr/bin/env python3
"""Same-client fresh 1 TB CLI benchmark. Never compares setup failure with latency.

Runs on the developer laptop. Anybranch database traffic uses an encrypted SSH
tunnel; Ardent uses its returned TLS URL. Region and routing differ and are recorded.
No ready pool, existing branch reuse, or retries inside a timed successful sample.
"""
import argparse
import datetime
import json
import math
import os
from pathlib import Path
import secrets
import statistics
import socket
import subprocess
import time
from urllib.parse import urlsplit, urlunsplit
import psycopg


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--anybranch',required=True)
    p.add_argument('--ardent',required=True)
    p.add_argument('--provider',choices=['anybranch','ardent','both'],default='both')
    p.add_argument('--trials',type=int,default=20)
    p.add_argument('--ssh-config',required=True)
    p.add_argument('--ssh-socket',required=True)
    p.add_argument('--host',default='anybranch-mumbai')
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--minimum-bytes',type=int,default=10**12)
    p.add_argument('--probe-table',default='fresh994209c5_probe')
    p.add_argument('--ardent-connector-id',help='Refuse to benchmark a different active Ardent connector')
    args=p.parse_args()
    if not args.probe_table.replace('_','').isalnum():raise ValueError('invalid probe table')
    env=dict(os.environ,ANYBRANCH_HOME=str(args.report.parent/'comparison-client-must-not-exist'))
    env.pop('ANYBRANCH_INTERNAL',None)
    ssh=['ssh','-F',args.ssh_config,'-S',args.ssh_socket]
    forwards={}
    report={'state':'running','providers':{},'checks':[],
            'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'client':'same developer laptop','anybranch_region':'ap-south-1','ardent_region':'us-east-1',
            'routing':'Anybranch SSH versus Ardent native TLS; routing setup for new Anybranch branch included in elapsed time',
            'precreated_capacity':False,'minimum_source_bytes':args.minimum_bytes,
            'measurement':'CLI process launch -> branch URL -> new authenticated connection -> fresh marker and large-table read -> committed write and read-back'}
    def save():
        temporary=args.report.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(args.report)
    def check(value,label):
        if not value:raise RuntimeError(label)
        report['checks'].append(label)
    def command(binary,*command,timeout=300):
        out=subprocess.run([binary,*command],env=env,capture_output=True,text=True,timeout=timeout)
        if out.returncode:
            # Write exact output privately. It may contain an endpoint credential.
            log=args.report.with_suffix('.private-error.log');log.touch(mode=0o600,exist_ok=True)
            log.write_text(out.stdout+'\n'+out.stderr)
            raise RuntimeError(command[0]+' failed; see private CLI log')
        return out.stdout.strip()
    def ab(*c):return command(args.anybranch,*c)
    def route(url):
        parts=urlsplit(url)
        port=parts.port
        if port not in forwards:
            with socket.socket() as reservation:
                reservation.bind(('127.0.0.1',0))
                local_port=reservation.getsockname()[1]
            subprocess.run([*ssh,'-O','forward','-o','ExitOnForwardFailure=yes','-L',f'127.0.0.1:{local_port}:127.0.0.1:{port}',args.host],capture_output=True,check=True)
            forwards[port]=local_port
        credentials=parts.netloc.rsplit('@',1)[0]+'@' if '@' in parts.netloc else ''
        return urlunsplit(parts._replace(netloc=credentials+'127.0.0.1:'+str(forwards[port])))
    def db(url):return psycopg.connect(url,autocommit=True,connect_timeout=10)
    table='public.'+args.probe_table
    prefix='cmp'+secrets.token_hex(4)
    report['prefix']=prefix
    save()
    try:
        source_url=route(ab('url','source')); replica_url=route(ab('url','replica'))
        with db(source_url) as source:
            report['source_table_bytes']=source.execute("SELECT sum(pg_total_relation_size(c.oid))::bigint FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname LIKE 'scale_data_%' AND c.relkind='r'").fetchone()[0]
            check(report['source_table_bytes']>=args.minimum_bytes,'source measured at least 1 TB')
            expected=source.execute('SELECT md5(payload) FROM public.scale_data_0 WHERE id=1').fetchone()[0]
        for provider in (['anybranch','ardent'] if args.provider=='both' else [args.provider]):
            data=report['providers'][provider]={'trials':[],'state':'running'}
            for i in range(args.trials):
                if provider=='ardent' and args.ardent_connector_id:
                    config=json.loads((Path.home()/'.ardent/config.json').read_text())
                    check(config.get('currentConnectorId')==args.ardent_connector_id,'Ardent active connector matches benchmark source')
                nonce=secrets.token_hex(12)
                with db(source_url) as source:
                    source.execute(f'INSERT INTO {table}(id,value) VALUES(1,%s) ON CONFLICT(id) DO UPDATE SET value=EXCLUDED.value',[nonce])
                deadline=time.monotonic()+120
                while True:
                    with db(replica_url) as replica:
                        row=replica.execute(f'SELECT value FROM {table} WHERE id=1').fetchone()
                    if row and row[0]==nonce:break
                    if time.monotonic()>deadline:raise RuntimeError('Anybranch replication not caught up')
                    time.sleep(.1)
                # Ardent does not expose direct replica SQL. Record a fixed source
                # catch-up grace for both systems and verify the exact marker in
                # every returned branch. Stale branches fail, never get omitted.
                time.sleep(2)
                name=f'{prefix}-{provider}-{i}'
                started=time.perf_counter()
                try:
                    if provider=='anybranch':url=ab('create',name,'--from','replica','--print-url')
                    else:url=command(args.ardent,'branch','create',name,'--print-url')
                    url_ms=(time.perf_counter()-started)*1000
                    if provider=='anybranch':url=route(url)
                    with db(url) as branch:
                        check(branch.execute(f'SELECT value FROM {table} WHERE id=1').fetchone()[0]==nonce,f'{provider} {i}: latest source marker')
                        check(branch.execute('SELECT md5(payload) FROM public.scale_data_0 WHERE id=1').fetchone()[0]==expected,f'{provider} {i}: large-table data')
                        branch.execute(f'UPDATE {table} SET value=%s WHERE id=1',['agent-'+nonce])
                        check(branch.execute(f'SELECT value FROM {table} WHERE id=1').fetchone()[0]=='agent-'+nonce,f'{provider} {i}: committed write')
                        elapsed=(time.perf_counter()-started)*1000
                        # Endpoint timing excludes the potentially expensive size
                        # inventory, which is verified separately on every branch.
                        check(branch.execute("SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname ~ '^scale_data_[0-7]$' AND c.relkind='r'").fetchone()[0]==8,f'{provider} {i}: all eight large tables present')
                        branch_bytes=branch.execute("SELECT sum(pg_total_relation_size(c.oid))::bigint FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relname LIKE 'scale_data_%' AND c.relkind='r'").fetchone()[0]
                        check(branch_bytes>=args.minimum_bytes,f'{provider} {i}: branch measured at least 1 TB')
                    with db(source_url) as source,db(replica_url) as replica:
                        check(source.execute(f'SELECT value FROM {table} WHERE id=1').fetchone()[0]==nonce,f'{provider} {i}: source isolation')
                        check(replica.execute(f'SELECT value FROM {table} WHERE id=1').fetchone()[0]==nonce,f'{provider} {i}: replica isolation')
                    data['trials'].append({'index':i,'success':True,'url_ms':round(url_ms,3),'read_write_ms':round(elapsed,3),'branch_table_bytes':branch_bytes})
                except Exception as exc:
                    data['trials'].append({'index':i,'success':False,'elapsed_ms':round((time.perf_counter()-started)*1000,3),'error':str(exc)})
                    data['state']='failed';raise
                save();print(provider,json.dumps(data['trials'][-1]),flush=True)
                if provider=='anybranch':ab('rm',name)
                else:command(args.ardent,'branch','delete',name)
            samples=sorted(t['read_write_ms'] for t in data['trials'])
            data['summary']={'n':len(samples),'p50_ms':statistics.median(samples),'p95_ms':samples[math.ceil(.95*len(samples))-1],'max_ms':max(samples),'all_under_6s':all(t<6000 for t in samples)}
            data['state']='passed'
        check(not Path(env['ANYBRANCH_HOME']).exists(),'no local database copy')
        report['state']='passed'
    except Exception as exc:
        report.update(state='failed',error=str(exc));raise
    finally:save()


if __name__=='__main__':main()
