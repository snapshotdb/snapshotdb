#!/usr/bin/env python3
"""After retained 1 TB regression checks, measure prepared PostgreSQL agent claims.

Runs as root to manage a separate service, whose engine processes run as anybranch.
Original benchmark databases and binary remain in place. All extra test DBs are retained.
"""
import argparse
import concurrent.futures
import datetime
import json
import os
from pathlib import Path
import pwd
import secrets
import shlex
import subprocess
import time
import urllib.request
import psycopg


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--source-env", default="/etc/anybranch-tb.env")
    p.add_argument("--source-api", default="http://127.0.0.1:7433")
    p.add_argument("--parent", default="replica")
    p.add_argument("--source-binary", default="/usr/local/bin/anybranch")
    p.add_argument("--binary", default="/usr/local/bin/anybranch-latency")
    p.add_argument("--home", type=Path, default=Path("/srv/anybranch-tb/agent-latency"))
    p.add_argument("--port", type=int, default=7441)
    p.add_argument("--service", default="anybranch-tb-agent-server")
    p.add_argument("--minimum-bytes", type=int, default=10**12)
    p.add_argument("--prerequisite", type=Path, default=Path("/var/lib/anybranch-benchmark/progress.json"))
    p.add_argument("--report", type=Path, default=Path("/var/lib/anybranch-benchmark/agent-latency-report.json"))
    args = p.parse_args()
    os.umask(0o077)
    report = {"state":"running", "passed":False, "checks":[], "minimum_bytes":args.minimum_bytes,
              "started_at":datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "measurement":"CLI launch, URL, new authenticated PostgreSQL connection, read, committed write, read-back; same EC2 host"}
    started = time.monotonic()
    def save():
        report["elapsed_seconds"] = round(time.monotonic()-started, 2)
        args.report.write_text(json.dumps(report, indent=2)+"\n")
    def check(value, label):
        if not value:
            raise RuntimeError(label)
        report["checks"].append(label)
        save()
    def read_env(path):
        result = os.environ.copy()
        result.pop("ANYBRANCH_INTERNAL", None)
        for line in path.read_text().splitlines():
            if line and not line.startswith("#"):
                k,v = line.split("=",1)
                result[k] = shlex.split(v)[0]
        return result
    def cli(env, binary, *command):
        result = subprocess.run([binary,*command], env=env, capture_output=True, text=True, timeout=1800)
        if result.returncode:
            raise RuntimeError(command[0]+" failed; inspect the corresponding private server job")
        return result.stdout.strip()
    def query(url, sql, params=None):
        with psycopg.connect(url, autocommit=True, connect_timeout=10) as db:
            return db.execute(sql, params).fetchall()
    save()
    try:
        check(json.loads(args.prerequisite.read_text()).get("all_tests_passed") is True,
              "initial benchmark and retained regression tests passed")
        original_env = read_env(Path(args.source_env))
        source_home = Path(original_env["ANYBRANCH_HOME"])
        original_env.update(ANYBRANCH_SERVER=args.source_api, ANYBRANCH_HOME=str(args.home/"source-client-must-not-exist"))
        seed = "agent-latency-seed-"+secrets.token_hex(4)
        report["retained_seed"] = seed
        cli(original_env, args.source_binary, "create", seed, "--from", args.parent, "--print-url")
        cli(original_env, args.source_binary, "stop", seed)
        # This dedicated stopped seed has never been handed to an agent. Its only
        # reader during import is the new server's copy-on-write import command.
        account = pwd.getpwnam("anybranch")
        args.home.mkdir(parents=True, exist_ok=True)
        os.chown(args.home, account.pw_uid, account.pw_gid)
        env_file = args.home/"server.env"
        if env_file.exists():
            raise RuntimeError("test home already configured; inspect retained state before rerunning")
        token = secrets.token_hex(32)
        env_file.write_text(f"ANYBRANCH_HOME={args.home}/server\nANYBRANCH_TOKEN={token}\nUSER=anybranch\nPATH=/usr/lib/postgresql/16/bin:/usr/local/bin:/usr/sbin:/usr/bin:/bin\n")
        os.chown(env_file, account.pw_uid, account.pw_gid)
        unit = Path("/etc/systemd/system")/(args.service+".service")
        if unit.exists():
            raise RuntimeError("refusing to overwrite an existing service")
        unit.write_text(f"[Unit]\nDescription=Retained Anybranch prepared agent benchmark server\nAfter=network.target\nRequiresMountsFor={args.home}\n\n[Service]\nUser=anybranch\nEnvironmentFile={env_file}\nExecStart={args.binary} serve --bind 127.0.0.1:{args.port} --public-host 127.0.0.1 --db-bind 127.0.0.1\nKillMode=control-group\nTimeoutStopSec=120\nRestart=on-failure\n\n[Install]\nWantedBy=multi-user.target\n")
        subprocess.run(["systemctl","daemon-reload"],check=True)
        subprocess.run(["systemctl","enable","--now",args.service],check=True,stdout=subprocess.DEVNULL)
        env = read_env(env_file)
        env.update(ANYBRANCH_SERVER=f"http://127.0.0.1:{args.port}",ANYBRANCH_HOME=str(args.home/"agent-client-must-not-exist"))
        for _ in range(300):
            try:
                request = urllib.request.Request(env["ANYBRANCH_SERVER"]+"/v1/health",headers={"Authorization":"Bearer "+token})
                with urllib.request.urlopen(request,timeout=1):
                    break
            except OSError:
                time.sleep(.1)
        else:
            raise RuntimeError("candidate server did not start")
        cli(env,args.binary,"import","postgres","tb-root",str(source_home/seed/"data"),"--print-url")
        password = args.home/"server/tb-root/password"
        password.write_text((source_home/seed/"password").read_text())
        os.chown(password,account.pw_uid,account.pw_gid)
        root_url = cli(env,args.binary,"url","tb-root")
        table_bytes = query(root_url,"SELECT coalesce(sum(pg_total_relation_size(c.oid)),0)::bigint FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind='r'")[0][0]
        report["measured_table_bytes"] = table_bytes
        check(table_bytes >= args.minimum_bytes,"candidate baseline contains the required physical table/index bytes")
        with psycopg.connect(root_url, autocommit=True, connect_timeout=10) as db:
            db.execute("CREATE TABLE agent_latency_probe(id integer PRIMARY KEY,value integer)")
            db.execute("INSERT INTO agent_latency_probe VALUES(1,0)")
        preparation = time.perf_counter()
        cli(env,args.binary,"prepare","tb-snapshot","--from","tb-root","--count","4")
        report["preparation_seconds"] = round(time.perf_counter()-preparation,3)
        def claim(i):
            before = time.perf_counter()
            url = cli(env,args.binary,"create",f"tb-agent-{i}","--from","tb-snapshot","--print-url")
            with psycopg.connect(url,autocommit=True,connect_timeout=5) as db:
                assert db.execute("SELECT value FROM agent_latency_probe WHERE id=1").fetchone()[0] == 0
                db.execute("UPDATE agent_latency_probe SET value=%s WHERE id=1",[i])
                assert db.execute("SELECT value FROM agent_latency_probe WHERE id=1").fetchone()[0] == i
                elapsed = (time.perf_counter()-before)*1000
            return url,elapsed
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            results = list(executor.map(claim,range(1,5)))
        report["claim_read_write_ms"] = [round(t,2) for _,t in results]
        check(len({u for u,_ in results}) == 4,"four simultaneous agents received distinct URLs")
        check(all(t < 1000 for _,t in results),"all four allocations and first committed reads/writes beat 1 second")
        for i,(url,_) in enumerate(results,1):
            check(query(url,"SELECT value FROM agent_latency_probe WHERE id=1")[0][0] == i,f"agent {i} retained its isolated committed write")
        check(query(root_url,"SELECT value FROM agent_latency_probe WHERE id=1")[0][0] == 0,"baseline unchanged by agents")
        check(not Path(env["ANYBRANCH_HOME"]).exists() and not Path(original_env["ANYBRANCH_HOME"]).exists(),"clients created no local database storage")
        report.update(state="passed",passed=True,retained_service=args.service,retained_home=str(args.home))
    except Exception as exc:
        report.update(state="failed",error=str(exc))
        raise
    finally:
        save()


if __name__ == "__main__":
    main()
