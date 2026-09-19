"""Server-host worker for benchmark.py; creates only synthetic PostgreSQL data."""
import json
import os
import plistlib
from pathlib import Path
import shutil
import secrets
import socket
import subprocess
import sys
import tempfile
import time


def required_free_bytes(target):
    # Source + independent replica + WAL, indexes, and branch-write headroom.
    return target * 3 + 2 * 10**9


def batch_rows(target, current_size):
    # Keep small runs small, while retaining 10,000 rows for the isolation write test.
    return min(50000, max(10000, (target - current_size + 2047) // 2048))


def resume_directory(base, directory, target, keep):
    work = Path(directory)
    if not keep or work.is_symlink() or work.resolve().parent != base.resolve():
        raise RuntimeError("resume requires --keep and a direct benchmark directory under --home")
    if not work.name.startswith("scale-"):
        raise RuntimeError("not a benchmark directory")
    report = json.loads((work / "report.json").read_text())
    if report.get("target_bytes") != target or Path(report.get("run_directory", "")).resolve() != work.resolve():
        raise RuntimeError("resume target/directory does not match the saved benchmark")
    if not (work / "source/data/PG_VERSION").is_file():
        raise RuntimeError("resume source PostgreSQL database is missing")
    if any((work / name).exists() for name in ("replica", "branch-a", "branch-b")):
        raise RuntimeError("only interrupted source generation can be resumed")
    return work


def filesystem(path):
    if sys.platform == "darwin":
        device = subprocess.check_output(["df", "-P", str(path)], text=True).splitlines()[-1].split()[0]
        info = plistlib.loads(subprocess.check_output(["diskutil", "info", "-plist", device]))
        kind = info.get("FilesystemType", "")
        if kind != "apfs":
            raise RuntimeError("the macOS benchmark requires an APFS volume")
        return kind, ["cp", "-c"]
    if sys.platform != "linux":
        raise RuntimeError("the benchmark supports Linux and macOS")
    mount = subprocess.check_output(["findmnt", "-no", "FSTYPE,OPTIONS", "-T", str(path)], text=True).strip()
    if "compress" in mount and "compress=no" not in mount:
        raise RuntimeError("use a volume with compression disabled for this physical-size benchmark")
    return mount, ["cp", "--reflink=always"]


def run_benchmark(config):
    os.umask(0o077)
    base = Path(config["home"])
    if not base.is_absolute():
        raise RuntimeError("server data directory must be absolute")
    for program in (config["binary"], "psql", "pg_ctl", "initdb", "pg_dump", "pg_dumpall", "cp",
                    "diskutil" if sys.platform == "darwin" else "findmnt"):
        if not shutil.which(program):
            raise RuntimeError("missing server executable: " + program)
    if os.geteuid() == 0:
        raise RuntimeError("run as an ordinary user; PostgreSQL refuses to run as root")
    base.mkdir(parents=True, exist_ok=True)
    target = config["target_bytes"]
    resume = config.get("resume")
    work = resume_directory(base, resume, target, config["keep"]) if resume else None
    # Credit already allocated source relation blocks, but still reserve the full
    # source + replica + WAL budget. Never count sparse logical file lengths.
    existing_bytes = min(target, sum(p.stat().st_blocks * 512 for p in
                         (work / "source/data/base").rglob("*") if p.is_file())) if work else 0
    needed = required_free_bytes(target) - existing_bytes
    free = shutil.disk_usage(base).free
    if free < needed:
        raise RuntimeError(f"need at least {needed:,} free bytes; found {free:,}")
    mount, clone_command = filesystem(base)
    work = work or Path(tempfile.mkdtemp(prefix="scale-", dir=base))
    os.environ["SNAPSHOTDB_HOME"] = str(work)
    os.environ["SNAPSHOTDB_IDLE_MINUTES"] = "0"
    report = dict(target_bytes=target, filesystem=mount, run_directory=str(work), checks=[], timings_seconds={})
    if resume:
        report["resumed_generation"] = True
        report["resume_allocated_source_bytes"] = existing_bytes
        previous = work / "report.json"
        shutil.copy2(previous, work / f"report-before-resume-{time.time_ns()}.json")
    created = []
    urls = {}
    started = time.monotonic()
    server = None
    server_log = None

    def log(message):
        print(message, flush=True)

    def save():
        (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    def execute(args, sql=None, timeout=600):
        result = subprocess.run(args, input=sql, text=True, capture_output=True, timeout=timeout)
        if result.returncode:
            # Command arguments can contain generated database credentials.
            detail = result.stderr
            for url in urls.values():
                detail = detail.replace(url, "<database URL>")
            raise RuntimeError(f"{Path(args[0]).name} failed: {detail[-2000:]}")
        return result.stdout.strip()

    def ab(*args):
        return execute([config["binary"], *args])

    def query(name, sql, timeout=600):
        return execute(["psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1", urls[name]], sql, timeout)

    def check(label, condition):
        if not condition:
            raise RuntimeError("check failed: " + label)
        report["checks"].append(label)
        log("PASS " + label)
        save()

    def wait_for(label, condition, timeout=120):
        deadline = time.monotonic() + timeout
        next_log = 0
        while not condition():
            now = time.monotonic()
            if now > deadline:
                raise RuntimeError("timed out: " + label)
            if now >= next_log:
                log("Waiting: " + label)
                next_log = now + 60
            time.sleep(5)
        check(label, True)

    def create(name, *args):
        created.append(name)  # Include partially created branches in cleanup.
        before = time.monotonic()
        urls[name] = ab(*args)
        report["timings_seconds"][name] = round(time.monotonic() - before, 3)

    try:
        log(f"Server run: {work}; target table/index bytes: {target:,}")
        report["version"] = ab("--version")
        # Fail before generating data if this volume cannot clone without copying.
        probe = work / "clone-probe"
        probe.write_bytes(b"snapshotdb reflink probe\n")
        execute([*clone_command, str(probe), str(probe) + "-copy"])
        probe.unlink()
        Path(str(probe) + "-copy").unlink()
        check("filesystem supports copy-on-write cloning", True)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            api_port = listener.getsockname()[1]
        os.environ["SNAPSHOTDB_SERVER"] = f"http://127.0.0.1:{api_port}"
        os.environ["SNAPSHOTDB_TOKEN"] = secrets.token_hex(32)
        server_log = (work / "server.log").open("a" if resume else "w")
        server = subprocess.Popen([config["binary"], "serve", "--bind", f"127.0.0.1:{api_port}",
                                   "--public-host", "127.0.0.1"], stdout=server_log, stderr=server_log)
        for _ in range(100):
            if server.poll() is not None:
                raise RuntimeError("server failed to start; see remote server.log")
            try:
                with socket.create_connection(("127.0.0.1", api_port), timeout=1):
                    break
            except OSError:
                time.sleep(0.1)
        else:
            raise RuntimeError("server did not start within 10 seconds")
        if resume:
            created.append("source")
            urls["source"] = ab("url", "source")
            check("resumed source marker is unchanged", query("source",
                  "SELECT count(*) = 1 AND bool_and(id = 1 AND value = 'original') FROM public.scale_marker;") == "t")
        else:
            create("source", "import", "postgres", "source", "--new")
            query("source", "CREATE TABLE public.scale_marker(id integer PRIMARY KEY, value text);")
            query("source", "INSERT INTO public.scale_marker VALUES (1, 'original');")
        # Multiple tables exercise initial-copy workers. PLAIN storage prevents TOAST
        # compression from turning a nominal TB of repetitive payloads into a tiny DB.
        total_bytes = 0
        row_counts = {}
        fill_started = time.monotonic()
        for table in range(8):
            name = f"public.scale_data_{table}"
            exists = resume and query("source", f"SELECT to_regclass('{name}') IS NOT NULL;") == "t"
            if not exists:
                query("source", f"CREATE TABLE {name}(id bigint PRIMARY KEY, payload text NOT NULL);")
            query("source", f"ALTER TABLE {name} ALTER COLUMN payload SET STORAGE PLAIN;")
            size = rows = 0
            if exists:
                # INSERT batches commit atomically. Read the database, rather than a
                # progress log that may lag a completed transaction at interruption.
                rows, last_id, first_id = map(int, query("source", f"SELECT count(*), coalesce(max(id),0), "
                                                       f"coalesce(min(id),1) FROM {name};", timeout=7200).split("|"))
                check(f"resumed table {table} has contiguous committed rows", rows == last_id and first_id == 1)
                size = int(query("source", f"SELECT pg_total_relation_size('{name}');"))
                log(f"Generated table {table}: {size:,} bytes / {(target + 7) // 8:,}")
            while size < (target + 7) // 8:
                if shutil.disk_usage(work).free < target + 10**9:
                    raise RuntimeError("free space fell below replica reserve during generation")
                # Bounded transactions; unique row content, approximately 2 KB per row.
                batch = batch_rows((target + 7) // 8, size)
                query("source", f"INSERT INTO {name} SELECT g, repeat(md5(g::text || ':{table}'), 60) "
                                f"FROM generate_series({rows + 1}, {rows + batch}) g;", timeout=1800)
                rows += batch
                size = int(query("source", f"SELECT pg_total_relation_size('{name}');"))
                log(f"Generated table {table}: {size:,} bytes / {(target + 7) // 8:,}")
            row_counts[name] = rows
            total_bytes += size
        report["source_table_bytes"] = total_bytes
        report["rows_per_table"] = row_counts
        report["source_database_bytes"] = int(query("source", "SELECT pg_database_size(current_database());"))
        report["timings_seconds"]["generate"] = round(time.monotonic() - fill_started, 3)
        check("measured table/index storage reaches target", total_bytes >= target)
        query("source", "CHECKPOINT;")
        copy_started = time.monotonic()
        create("replica", "clone", "replica", urls["source"])
        wait_for("initial copy ready for every subscribed table", lambda: query("replica",
                 "SELECT count(*) >= 9 AND bool_and(srsubstate = 'r') FROM pg_subscription_rel;") == "t",
                 config["timeout_seconds"])
        report["timings_seconds"]["initial_copy"] = round(time.monotonic() - copy_started, 3)
        for table, rows in row_counts.items():
            check("replica row count " + table,
                  int(query("replica", f"SELECT count(*) FROM {table};", timeout=7200)) == rows)
        query("source", "UPDATE public.scale_marker SET value = 'live' WHERE id = 1;")
        wait_for("live source update reaches replica", lambda: query("replica",
                 "SELECT value FROM public.scale_marker WHERE id = 1;") == "live")
        # DDL is sent on its own, matching SnapshotDB's supported replay path.
        query("source", "ALTER TABLE public.scale_marker ADD COLUMN extra integer DEFAULT 7;")
        wait_for("schema change reaches replica", lambda: query("replica",
                 "SELECT count(*) FROM information_schema.columns WHERE table_schema='public' "
                 "AND table_name='scale_marker' AND column_name='extra';") == "1")
        free_before = shutil.disk_usage(work).free
        create("branch-a", "create", "branch-a", "--from", "replica", "--print-url")
        create("branch-b", "create", "branch-b", "--from", "replica", "--print-url")
        report["filesystem_used_delta_after_two_branches"] = free_before - shutil.disk_usage(work).free
        for branch in ("branch-a", "branch-b"):
            check(branch + " has no subscription", query(branch, "SELECT count(*) FROM pg_subscription;") == "0")
            check(branch + " has no source URL file", not (work / branch / "source").exists())
        query("branch-a", "UPDATE public.scale_marker SET value = 'branch-only' WHERE id = 1;")
        query("branch-a", "UPDATE public.scale_data_0 SET payload = repeat('changed', 200) WHERE id <= 10000;")
        for name in ("source", "replica", "branch-b"):
            check(name + " isolated from branch writes", query(name,
                  "SELECT value FROM public.scale_marker WHERE id = 1;") == "live")
            check(name + " payload unchanged", query(name,
                  "SELECT payload = repeat(md5('1:0'), 60) FROM public.scale_data_0 WHERE id = 1;") == "t")
        check("branch write persisted", query("branch-a",
              "SELECT count(*) FROM public.scale_data_0 WHERE id <= 10000 AND payload = repeat('changed', 200);") == "10000")
        query("source", "UPDATE public.scale_marker SET value = 'later' WHERE id = 1;")
        wait_for("replica continues after branching", lambda: query("replica",
                 "SELECT value FROM public.scale_marker WHERE id = 1;") == "later")
        check("branch detached from later source writes", query("branch-b",
              "SELECT value FROM public.scale_marker WHERE id = 1;") == "live")
        ab("stop", "branch-a")
        before = time.monotonic()
        check("suspended branch resumes through its URL", query("branch-a", "SELECT 1;") == "1")
        report["timings_seconds"]["resume"] = round(time.monotonic() - before, 3)
        report["passed"] = True
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        raise
    finally:
        if not config["keep"]:
            errors = []
            for name in reversed(created):
                if not (work / name).exists():
                    continue
                try:
                    ab("rm", name)
                    if name == "replica" and (work / "source").exists():
                        check("source replication slot cleaned up", query("source",
                              "SELECT count(*) FROM pg_replication_slots;") == "0")
                except Exception:
                    errors.append(name)
                    # Keep the source available when replica cleanup fails.
                    break
            report["cleanup_failed"] = errors
            if errors:
                log("Cleanup incomplete; retained databases at " + str(work))
        report["timings_seconds"]["total"] = round(time.monotonic() - started, 3)
        if server is not None:
            server.terminate()
            server.wait(timeout=30)
        if server_log is not None:
            server_log.close()
        save()
        log("Report: " + str(work / "report.json"))
    return 0 if report.get("passed") and not report.get("cleanup_failed") else 1


if __name__ == "__main__":
    try:
        raise SystemExit(run_benchmark(json.loads(sys.argv[1])))
    except Exception as error:
        print("Benchmark failed: " + str(error), file=sys.stderr)
        raise SystemExit(1)
