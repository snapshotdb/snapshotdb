#!/usr/bin/env python3
"""Run a disposable Anybranch server and PostgreSQL scale test on a chosen host."""
import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys


def absolute_path(value):
    if not value.startswith("/"):
        raise argparse.ArgumentTypeError("use an absolute path on the server host")
    return value


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    location = p.add_mutually_exclusive_group(required=True)
    location.add_argument("--host", help="SSH host or configured alias")
    location.add_argument("--local", action="store_true", help="explicitly run the test server on this machine")
    p.add_argument("--home", required=True, type=absolute_path, help="data directory on the chosen host")
    p.add_argument("--binary", default="anybranch", help="server-host binary, preferably an absolute path")
    p.add_argument("--engine-bin", type=absolute_path, help="PostgreSQL bin directory on the chosen host")
    size = p.add_mutually_exclusive_group()
    size.add_argument("--target-gb", type=int, help="decimal GB of table/index storage; default 1; 1000 = 1 TB")
    size.add_argument("--target-mb", type=int, help="decimal MB for a small smoke test")
    p.add_argument("--timeout-hours", type=int, default=48, help="maximum initial replication wait")
    p.add_argument("--keep", action="store_true", help="retain test databases for inspection")
    p.add_argument("--resume", type=absolute_path, help="resume an interrupted generation directory; requires --keep")
    return p


def ssh_command(host, argv):
    if not host or host.startswith("-") or any(c.isspace() for c in host):
        raise ValueError("host must be an SSH hostname or alias, not SSH options")
    return ["ssh", "-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", host, shlex.join(argv)]


def main(argv=None):
    p = parser()
    a = p.parse_args(argv)
    if a.resume and not a.keep:
        p.error("--resume requires --keep so existing data is preserved on failure")
    prefix = ["env", "ANYBRANCH_HOME=" + a.home]
    if a.engine_bin:
        # A fixed executable search path avoids expanding user input in a remote shell.
        prefix.append("PATH=" + a.engine_bin + ":/usr/local/bin:/usr/bin:/bin")
    size = a.target_mb if a.target_mb is not None else (a.target_gb if a.target_gb is not None else 1)
    if size < 1 or a.timeout_hours < 1:
        p.error("target size and timeout-hours must be positive integers")
    target = size * (10**6 if a.target_mb is not None else 10**9)
    config = dict(home=a.home, binary=a.binary, target_bytes=target,
                  timeout_seconds=a.timeout_hours * 3600, keep=a.keep)
    if a.resume:
        config["resume"] = a.resume
    if a.local:
        environment = os.environ.copy()
        if a.engine_bin:
            environment["PATH"] = a.engine_bin + os.pathsep + environment.get("PATH", "")
        worker = Path(__file__).with_name("scale_postgres.py")
        return subprocess.run([sys.executable, str(worker), json.dumps(config)], env=environment).returncode
    source = Path(__file__).with_name("scale_postgres.py").read_text()
    remote = prefix + ["python3", "-", json.dumps(config)]
    try:
        command = ssh_command(a.host, remote)
    except ValueError as e:
        p.error(str(e))
    # SSH mode never falls back to local execution. --local must be explicit.
    return subprocess.run(command, input=source, text=True).returncode


if __name__ == "__main__":
    raise SystemExit(main())
