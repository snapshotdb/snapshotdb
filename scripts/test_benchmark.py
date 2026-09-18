import contextlib
import io
import json
from pathlib import Path
import shlex
import tempfile
import unittest
from unittest.mock import patch

import benchmark
from scale_postgres import batch_rows, required_free_bytes, resume_directory


class RemoteBenchmarkTests(unittest.TestCase):
    def test_resume_requires_retention_and_rejects_other_stages(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            work = base / "scale-test"
            (work / "source/data").mkdir(parents=True)
            (work / "source/data/PG_VERSION").write_text("16")
            (work / "report.json").write_text(json.dumps({"target_bytes": 1000, "run_directory": str(work)}))
            self.assertEqual(resume_directory(base, str(work), 1000, True), work)
            for target, keep in [(2000, True), (1000, False)]:
                with self.assertRaises(RuntimeError):
                    resume_directory(base, str(work), target, keep)
            with self.assertRaises(RuntimeError):
                resume_directory(base / "elsewhere", str(work), 1000, True)
            (work / "replica").mkdir()
            with self.assertRaises(RuntimeError):
                resume_directory(base, str(work), 1000, True)

    def test_resume_cli_preserves_existing_data(self):
        with patch("benchmark.subprocess.run") as run, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                benchmark.main(["--local", "--home", "/srv/test", "--resume", "/srv/test/scale-run"])
            run.assert_not_called()
            run.return_value.returncode = 0
            benchmark.main(["--local", "--home", "/srv/test", "--resume", "/srv/test/scale-run", "--keep"])
            config = json.loads(run.call_args.args[0][-1])
            self.assertTrue(config["keep"])
            self.assertEqual(config["resume"], "/srv/test/scale-run")

    def test_local_execution_requires_explicit_selection(self):
        with patch("benchmark.subprocess.run") as run, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                benchmark.main(["--home", "/tmp/test"])
            run.assert_not_called()
            run.return_value.returncode = 0
            self.assertEqual(benchmark.main(["--local", "--home", "/tmp/test", "--target-mb", "100"]), 0)
            command = run.call_args.args[0]
            self.assertNotEqual(command[0], "ssh")
            self.assertIn('"target_bytes": 100000000', command[-1])

    def test_batches_bound_small_test_storage(self):
        self.assertEqual(batch_rows(12_500_000, 0), 10000)
        self.assertEqual(batch_rows(125_000_000_000, 0), 50000)
    def test_ssh_arguments_are_not_remote_shell_code(self):
        args = ["env", "SNAPSHOTDB_HOME=/srv/a b; touch /tmp/no", "python3", "-", '{"x":"$(id)"}']
        command = benchmark.ssh_command("test-host", args)
        self.assertEqual(shlex.split(command[-1]), args)
        self.assertEqual(command[0], "ssh")
        for host in ["-oProxyCommand=sh", "host other", ""]:
            with self.assertRaises(ValueError):
                benchmark.ssh_command(host, args)

    def test_ssh_failure_never_runs_worker_locally(self):
        with patch("benchmark.subprocess.run") as run:
            run.return_value.returncode = 255
            result = benchmark.main(["--host", "missing-host", "--home", "/srv/test", "--target-gb", "1000"])
            self.assertEqual(result, 255)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.args[0][0], "ssh")
            self.assertIn('"target_bytes": 1000000000000', run.call_args.args[0][-1])

    def test_invalid_scale_fails_before_ssh(self):
        with patch("benchmark.subprocess.run") as run, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                benchmark.main(["--host", "test-host", "--home", "/srv/test", "--target-gb", "0"])
            run.assert_not_called()
        self.assertEqual(required_free_bytes(10**12), 3_002_000_000_000)


if __name__ == "__main__":
    unittest.main()
