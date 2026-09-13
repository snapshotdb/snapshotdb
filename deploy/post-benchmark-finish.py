#!/usr/bin/env python3
"""Make interrupted/failed post-test services explicit in the durable status."""
import datetime
import json
import os
import sys
from pathlib import Path

base = Path(sys.argv[1]) if len(sys.argv) > 1 else Path('/var/lib/anybranch-benchmark')
path = base / 'post-test-report.json'
report = json.loads(path.read_text()) if path.exists() else {'checks': [], 'failures': []}
result = os.environ.get('SERVICE_RESULT', 'unknown')
if report.get('state') not in ('passed', 'failed') or result != 'success':
    report.update(state='failed', passed=False)
    report.setdefault('failures', []).append('post-test service ended: ' + result)
report['service_result'] = result
report['finished_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
path.write_text(json.dumps(report, indent=2) + '\n')
progress_path = base / 'progress.json'
progress = json.loads(progress_path.read_text())
progress['post_tests'] = report['state']
progress['all_tests_passed'] = progress.get('passed') is True and report.get('passed') is True
progress_path.write_text(json.dumps(progress, indent=2) + '\n')
