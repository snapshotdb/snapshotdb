#!/usr/bin/env python3
"""Native-driver E2E latency on disposable server-side fixtures (no local DB copies).

Run on the deployed Linux server with psycopg[binary], PyMySQL[rsa], pymongo.
Timer: CLI process launch -> returned URL -> new authenticated connection ->
read seeded data -> committed write -> read-back. Preparation is timed separately.
"""
import argparse
import concurrent.futures
import json
import math
import os
from pathlib import Path
import secrets
import subprocess
import time
import urllib.request
from urllib.parse import urlsplit, urlunsplit, unquote


def connect(engine, url):
    if engine == "postgres":
        import psycopg
        return psycopg.connect(url, autocommit=True, connect_timeout=5)
    if engine == "mysql":
        import pymysql
        p = urlsplit(url)
        return pymysql.connect(host=p.hostname, port=p.port, user=unquote(p.username),
                               password=unquote(p.password), database=p.path.strip("/") or None,
                               autocommit=True, connect_timeout=5)
    if engine == "mongodb":
        from pymongo import MongoClient
        return MongoClient(url, connectTimeoutMS=5000)
    return url


def sql(db, engine, statement, params=None):
    if engine == "sqlite":
        body = {"statements": [{"sql": statement, "params": params or []}]}
        request = urllib.request.Request(db, json.dumps(body).encode(), {"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=10) as response:
            return json.load(response)["results"][0]["rows"]
    with db.cursor() as cursor:
        cursor.execute(statement, params)
        return cursor.fetchall() if cursor.description else []


def close(db, engine):
    if engine != "sqlite":
        db.close()


def marker(engine, url, value=None):
    db = connect(engine, url)
    try:
        if engine == "mongodb":
            from pymongo.write_concern import WriteConcern
            collection = db.app.get_collection("latency_probe", write_concern=WriteConcern(w=1, j=True))
            if value is not None:
                collection.update_one({"_id": 1}, {"$set": {"value": value}})
            return collection.find_one({"_id": 1})["value"]
        if value is not None:
            placeholder = "?" if engine == "sqlite" else "%s"
            sql(db, engine, f"UPDATE latency_probe SET value={placeholder} WHERE id=1", [value])
        return sql(db, engine, "SELECT value FROM latency_probe WHERE id=1")[0][0]
    finally:
        close(db, engine)


def seed(engine, url):
    db = connect(engine, url)
    try:
        if engine == "mongodb":
            db.app.latency_probe.insert_many([{"_id": i, "value": 0} for i in range(1, 1001)])
        else:
            sql(db, engine, "CREATE TABLE latency_probe(id INTEGER PRIMARY KEY, value INTEGER NOT NULL)")
            sql(db, engine, "INSERT INTO latency_probe VALUES " + ",".join(f"({i},0)" for i in range(1, 1001)))
    finally:
        close(db, engine)


def summarize(samples):
    ordered = sorted(samples)
    return {"n": len(samples), "p50_ms": round(ordered[len(ordered)//2], 2),
            "p95_ms": round(ordered[math.ceil(len(ordered)*0.95)-1], 2), "max_ms": round(max(samples), 2),
            "all_under_1s": all(v < 1000 for v in samples), "samples_ms": [round(x, 2) for x in samples]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--engines", default="postgres,mysql,mongodb,sqlite")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--capacity", type=int, default=4)
    args = parser.parse_args()
    prefix = "lat" + secrets.token_hex(3)
    client_home = args.report.parent / (prefix + "-client-must-not-exist")
    env = dict(os.environ, SNAPSHOTDB_HOME=str(client_home))
    env.pop("SNAPSHOTDB_INTERNAL", None)
    report = {"state": "running", "prefix": prefix, "engines": {}, "checks": [],
              "measurement": "CLI launch to new authenticated connection, read, committed write, read-back; client on same EC2 host", "fixture_rows": 1000}
    def save():
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    def check(condition, label):
        assert condition, label
        report["checks"].append(label)
        save()
    def cli(*command, success=True):
        result = subprocess.run([args.binary, *command], env=env, capture_output=True, text=True, timeout=600)
        if success and result.returncode:
            # Arguments can include passwords; never write them to the report.
            raise RuntimeError(f"{command[0]} failed: {result.stderr[-2000:]}")
        if not success:
            return result.returncode != 0
        return result.stdout.strip()
    def allocate(engine, parent, name, value):
        started = time.perf_counter()
        url = cli("create", name, "--from", parent, "--print-url")
        # One new connection performs all timed DB work.
        db = connect(engine, url)
        try:
            if engine == "mongodb":
                from pymongo.write_concern import WriteConcern
                collection = db.app.get_collection("latency_probe", write_concern=WriteConcern(w=1, j=True))
                assert collection.find_one({"_id": 1})["value"] == 0
                collection.update_one({"_id": 1}, {"$set": {"value": value}})
                assert collection.find_one({"_id": 1})["value"] == value
            else:
                assert sql(db, engine, "SELECT value FROM latency_probe WHERE id=1")[0][0] == 0
                placeholder = "?" if engine == "sqlite" else "%s"
                sql(db, engine, f"UPDATE latency_probe SET value={placeholder} WHERE id=1", [value])
                assert sql(db, engine, "SELECT value FROM latency_probe WHERE id=1")[0][0] == value
            elapsed = (time.perf_counter() - started) * 1000
        finally:
            close(db, engine)
        return elapsed, url
    save()
    try:
        for engine in args.engines.split(","):
            print(f"{engine}: setting up disposable fixture", flush=True)
            base = prefix + "-" + engine
            source, replica, snapshot = base + "-src", base + "-rep", base + "-snap"
            url = cli("import", engine, source, "--new", "--print-url")
            if engine == "mysql":
                db = connect(engine, url)
                sql(db, engine, "CREATE DATABASE app")
                close(db, engine)
            if engine in ("mysql", "mongodb"):
                cli("settings", source, "set", "default_db", "app")
                url = cli("url", source)
            seed(engine, url)
            if engine == "sqlite":
                replica = source
                replica_url = url
            else:
                replica_url = cli("clone", replica, url)
                deadline = time.monotonic() + 120
                while True:
                    try:
                        if marker(engine, replica_url) == 0:
                            # PostgreSQL table sync may be finishing after rows are visible.
                            if engine == "postgres" and "copying" in cli("status", replica):
                                raise RuntimeError("copy still running")
                            break
                    except Exception:
                        if time.monotonic() > deadline:
                            raise
                    time.sleep(0.2)
            result = report["engines"][engine] = {"cold_ms": [], "warm_ms": [], "preparation_seconds": []}
            for i in range(2):
                name = base + f"-cold{i}"
                elapsed, branch_url = allocate(engine, replica, name, i+1)
                result["cold_ms"].append(elapsed)
                check(marker(engine, replica_url) == 0, f"{engine} cold source isolation {i}")
                cli("rm", name)
            for batch in range(args.rounds):
                started = time.perf_counter()
                ready = json.loads(cli("prepare", snapshot, "--from", replica, "--count", str(args.capacity)))
                result["preparation_seconds"].append(round(time.perf_counter()-started, 3))
                check(ready["ready"] == args.capacity, f"{engine} ready capacity batch {batch}")
                if batch == 0:
                    check(cli("start", snapshot, success=False), f"{engine} immutable snapshot refuses start")
                    marker(engine, url, 777)
                names = [base + f"-b{batch}-{i}" for i in range(args.capacity)]
                with concurrent.futures.ThreadPoolExecutor(max_workers=args.capacity) as executor:
                    claimed = list(executor.map(lambda pair: allocate(engine, snapshot, pair[1], pair[0]+1), enumerate(names)))
                result["warm_ms"].extend(t for t, _ in claimed)
                check(len({u for _, u in claimed}) == len(names), f"{engine} distinct URLs batch {batch}")
                for i, (name, (_, branch_url)) in enumerate(zip(names, claimed)):
                    check(marker(engine, branch_url) == i+1, f"{engine} independent committed data {name}")
                    check(cli("create", name, "--from", snapshot, "--print-url") == branch_url, f"{engine} idempotent claim {name}")
                check(cli("create", base+"-overflow", "--from", snapshot, success=False), f"{engine} exhausted pool refuses cold fallback {batch}")
                check(cli("rm", snapshot, success=False), f"{engine} snapshot deletion protects agents {batch}")
                check(marker(engine, url) == 777, f"{engine} original source untouched by agents {batch}")
                for name in names:
                    cli("rm", name)
                save()
            result["cold"] = summarize(result.pop("cold_ms"))
            result["warm"] = summarize(result.pop("warm_ms"))
            check(not client_home.exists(), f"{engine} client created no local storage")
            print(f"{engine}: warm {result['warm']}", flush=True)
            cli("rm", snapshot)
            if replica != source:
                cli("rm", replica)
            cli("rm", source)
        report["state"] = "passed"
        report["latency_target_passed"] = all(v["warm"]["all_under_1s"] for v in report["engines"].values())
    except Exception as exc:
        report["state"] = "failed"
        report["error"] = str(exc)
        raise
    finally:
        save()


if __name__ == "__main__":
    main()
