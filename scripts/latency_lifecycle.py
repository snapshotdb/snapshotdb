#!/usr/bin/env python3
"""Two-phase prepared branch restart/credential test. State contains private URLs.

Run setup; restart the disposable SnapshotDB server; run verify.
"""
import argparse
import json
import os
from pathlib import Path
import secrets
import subprocess
import time
from urllib.parse import urlsplit, urlunsplit
from latency import connect, sql, close, seed, marker


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("phase", choices=["setup", "verify"])
    p.add_argument("--binary", required=True)
    p.add_argument("--state", type=Path, required=True)
    p.add_argument("--report", type=Path, required=True)
    p.add_argument("--idle-seconds", type=int, default=0)
    args = p.parse_args()
    env = dict(os.environ, SNAPSHOTDB_HOME=str(args.state.parent / "lifecycle-client-must-not-exist"))
    env.pop("SNAPSHOTDB_INTERNAL", None)
    def cli(*command):
        result = subprocess.run([args.binary, *command], env=env, capture_output=True, text=True, timeout=300)
        if result.returncode:
            raise RuntimeError(f"{command[0]} failed: {result.stderr[-2000:]}")
        return result.stdout.strip()
    if args.phase == "setup":
        state = {}
        prefix = "life" + secrets.token_hex(3)
        args.state.touch(mode=0o600, exist_ok=True)
        for engine in ["postgres", "mysql", "mongodb", "sqlite"]:
            base = prefix + "-" + engine
            source, snapshot, agent = base+"-src", base+"-snap", base+"-agent"
            url = cli("import", engine, source, "--new", "--print-url")
            if engine == "mysql":
                db = connect(engine, url)
                sql(db, engine, "CREATE DATABASE app")
                close(db, engine)
            if engine in ("mysql", "mongodb"):
                cli("settings", source, "set", "default_db", "app")
                url = cli("url", source)
            seed(engine, url)
            cli("prepare", snapshot, "--from", source, "--count", "2")
            branch_url = cli("create", agent, "--from", snapshot, "--print-url")
            assert marker(engine, branch_url, 42) == 42
            assert marker(engine, url, 777) == 777
            state[engine] = dict(source=source, snapshot=snapshot, agent=agent, url=branch_url, source_url=url)
            args.state.write_text(json.dumps(state))
            print(engine+": restart fixture ready", flush=True)
        return
    if args.idle_seconds:
        time.sleep(args.idle_seconds)
    state = json.loads(args.state.read_text())
    report = {"state":"running", "checks":[], "warm_after_restart_ms":{}, "idle_seconds":args.idle_seconds}
    def check(value, label):
        assert value, label
        report["checks"].append(label)
    try:
        for engine, item in state.items():
            check(cli("url", item["agent"]) == item["url"], engine+" stable URL after restart")
            check(marker(engine, item["url"]) == 42, engine+" committed agent data after restart")
            before = time.perf_counter()
            name = item["agent"]+"2"
            url = cli("create", name, "--from", item["snapshot"], "--print-url")
            check(marker(engine, url) == 0, engine+" immutable snapshot survived restart")
            check(marker(engine, url, 84) == 84, engine+" new agent writes after restart")
            elapsed = (time.perf_counter()-before)*1000
            report["warm_after_restart_ms"][engine] = round(elapsed, 2)
            check(elapsed < 1000, engine+" ready claim and read/write under 1s after restart")
            check(marker(engine, item["url"]) == 42, engine+" sibling isolation after restart")
            check(marker(engine, item["source_url"]) == 777, engine+" source isolation after restart")
            # Borrowing another branch's password must not grant access.
            if engine == "sqlite":
                wrong = url.split("token=")[0]+"token="+item["url"].split("token=")[1]
            else:
                target, donor = urlsplit(url), urlsplit(item["url"])
                wrong = urlunsplit(target._replace(netloc=f"{target.username}:{donor.password}@{target.hostname}:{target.port}"))
            denied = False
            try:
                marker(engine, wrong)
            except Exception:
                denied = True
            check(denied, engine+" sibling credentials rejected")
            cli("rm", name)
            cli("rm", item["agent"])
            cli("rm", item["snapshot"])
            cli("rm", item["source"])
        check(not Path(env["SNAPSHOTDB_HOME"]).exists(), "no local client database files")
        report["state"] = "passed"
    except Exception as exc:
        report["state"] = "failed"
        report["error"] = str(exc)
        raise
    finally:
        args.report.write_text(json.dumps(report, indent=2)+"\n")
    args.state.unlink()


if __name__ == "__main__":
    main()
