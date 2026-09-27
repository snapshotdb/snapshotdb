import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from package_site_cli import ASSETS, package


class PackageTest(unittest.TestCase):
    def test_complete_bundle_checksums_and_no_mixed_release(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in ASSETS:
                (root / name).write_bytes(name.encode())
            out = root / "bundle"
            package(root, out, "a" * 40)
            manifest = json.loads((out / "BUILD.json").read_text())
            self.assertEqual(manifest["revision"], "a" * 40)
            for name in ASSETS:
                self.assertEqual(manifest["sha256"][name], hashlib.sha256((out / name).read_bytes()).hexdigest())
                self.assertIn(f'{manifest["sha256"][name]}  {name}\n', (out / "SHA256SUMS.txt").read_text())
            with self.assertRaises(ValueError):
                package(root, out, "b" * 40)

    def test_incomplete_bundle_writes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ASSETS[0]).write_bytes(b"one")
            with self.assertRaises(ValueError):
                package(root, root / "bundle", "a" * 40)
            self.assertFalse((root / "bundle").exists())
