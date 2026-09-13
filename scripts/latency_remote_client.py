#!/usr/bin/env python3
"""Measure a remote client's prepared claims through preconfigured SSH port forwards.

Database copies and engines run only on the deployed server. Native driver operations
traverse the SSH tunnel. Establishes branch-port forwards during preparation, equivalent
to routing those ports over a private network; tunnel setup is outside the timed claim.
"""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import secrets
import shlex
import subprocess
import time
from urllib.parse import urlsplit
from latency import connect, sql, close, seed, marker, summarize


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--binary", required=True)
    p.add_argument("--ssh-config", required=True)
    p.add_argument("--ssh-socket", required=True)
    p.add_argument("--host", required=True)
    p.add_argument("--server-home", required=True)
    p.add_argument("--report", type=Path, required=True)
    args = p.parse_args()
    env = dict(os.environ, ANYBRANCH_HOME=str(args.report.parent/"remote-client-must-not-exist"))
    env.pop("ANYBRANCH_INTERNAL", None)
    ssh = ["ssh","-F",args.ssh_config,"-S",args.ssh_socket]
    forwarded = set()
    report = {"state":"running","measurement":"Laptop CLI -> SSH -> Mumbai API -> returned URL -> native driver connection -> read -> committed write -> read-back", "network_preparation":"SSH forwarding established before timed claims", "engines":{}, "checks":[]}
    def cli(*command):
        result = subprocess.run([args.binary,*command],env=env,capture_output=True,text=True,timeout=300)
        if result.returncode:
            raise RuntimeError(command[0]+" failed; inspect private server job")
        return result.stdout.strip()
    def forward(ports):
        ports = set(ports)-forwarded
        if not ports:
            return
        command = [*ssh,"-O","forward","-o","ExitOnForwardFailure=yes"]
        for port in ports:
            command.extend(["-L",f"127.0.0.1:{port}:127.0.0.1:{port}"])
        subprocess.run([*command,args.host],check=True,capture_output=True)
        forwarded.update(ports)
    prefix = "wan"+secrets.token_hex(3)
    report["prefix"] = prefix
    try:
        for engine in ["postgres","mysql","mongodb","sqlite"]:
            base = prefix+"-"+engine
            source, snapshot = base+"-src", base+"-snap"
            url = cli("import",engine,source,"--new","--print-url")
            forward([urlsplit(url).port])
            if engine == "mysql":
                db = connect(engine,url)
                sql(db,engine,"CREATE DATABASE app")
                close(db,engine)
            if engine in ("mysql","mongodb"):
                cli("settings",source,"set","default_db","app")
                url = cli("url",source)
            seed(engine,url)
            cli("prepare",snapshot,"--from",source,"--count","4")
            code = "from pathlib import Path; import json; root=Path("+repr(args.server_home)+"); print(json.dumps([int((p/'run/port').read_text()) for p in root.glob('abwarm-*') if (p/'pool-ready').exists() and (p/'parent').read_text()=="+repr(snapshot)+"]))"
            result = subprocess.run([*ssh,args.host,"sudo python3 -c "+shlex.quote(code)],capture_output=True,text=True,check=True)
            forward(json.loads(result.stdout))
            def allocate(i):
                start = time.perf_counter()
                branch_url = cli("create",base+f"-a{i}","--from",snapshot,"--print-url")
                db = connect(engine,branch_url)
                try:
                    if engine == "mongodb":
                        from pymongo.write_concern import WriteConcern
                        coll = db.app.get_collection("latency_probe",write_concern=WriteConcern(w=1,j=True))
                        assert coll.find_one({"_id":1})["value"] == 0
                        coll.update_one({"_id":1},{"$set":{"value":i}})
                        assert coll.find_one({"_id":1})["value"] == i
                    else:
                        assert sql(db,engine,"SELECT value FROM latency_probe WHERE id=1")[0][0] == 0
                        placeholder = "?" if engine == "sqlite" else "%s"
                        sql(db,engine,f"UPDATE latency_probe SET value={placeholder} WHERE id=1",[i])
                        assert sql(db,engine,"SELECT value FROM latency_probe WHERE id=1")[0][0] == i
                    elapsed = (time.perf_counter()-start)*1000
                finally:
                    close(db,engine)
                return elapsed,branch_url
            with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
                results = list(executor.map(allocate,range(1,5)))
            report["engines"][engine] = summarize([t for t,_ in results])
            for i,(_,branch_url) in enumerate(results,1):
                assert marker(engine,branch_url) == i
                cli("rm",base+f"-a{i}")
            assert marker(engine,url) == 0
            cli("rm",snapshot)
            cli("rm",source)
            report["checks"].append(engine+" committed writes and source/sibling isolation through remote URLs")
            args.report.write_text(json.dumps(report,indent=2)+"\n")
            print(engine+": "+json.dumps(report["engines"][engine]),flush=True)
        assert not Path(env["ANYBRANCH_HOME"]).exists()
        report["checks"].append("no database files or engines on the laptop")
        report["state"] = "passed"
        report["latency_target_passed"] = all(v["all_under_1s"] for v in report["engines"].values())
    except Exception as exc:
        report.update(state="failed",error=str(exc))
        raise
    finally:
        args.report.write_text(json.dumps(report,indent=2)+"\n")


if __name__ == "__main__":
    main()
