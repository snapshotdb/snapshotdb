"""Exercise the actual CloudFormation disk guard on disposable files on Linux."""
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipUnless(shutil.which('wipefs') and shutil.which('mkfs.ext4'), 'Linux filesystem tools required')
class VolumeGuardTests(unittest.TestCase):
    def test_blank_occupied_and_failed_inspection(self):
        template = (Path(__file__).resolve().parents[1] / 'deploy/byoc/aws.yaml').read_text()
        guard = re.search(r'^          signatures=.*?\n          \[ -z "\$signatures" \]', template, re.MULTILINE | re.DOTALL)
        self.assertIsNotNone(guard, 'Template must contain a fail-closed signature guard')
        command = 'set -euo pipefail\ndevice=$1\n' + guard.group(0)
        with tempfile.TemporaryDirectory(prefix='anybranch-disk-test-') as directory:
            disk = Path(directory) / 'disposable.img'
            with disk.open('wb') as stream:
                stream.truncate(32 * 1024 * 1024)

            def allowed(path):
                return subprocess.run(['bash', '-c', command, 'guard', str(path)], capture_output=True).returncode == 0

            self.assertTrue(allowed(disk), 'Blank test volume should be accepted')
            subprocess.run(['mkfs.ext4', '-q', '-F', str(disk)], check=True, capture_output=True)
            self.assertFalse(allowed(disk), 'Existing filesystem must be refused')
            self.assertFalse(allowed(Path(directory) / 'missing'), 'Inspection failure must be refused')


if __name__ == '__main__':
    unittest.main()
